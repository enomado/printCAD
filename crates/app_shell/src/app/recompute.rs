//! Parametric recompute driver.
//!
//! Once per frame the app asks every registered workbench, through the
//! registry, which bodies it wants rebuilt and with what plan, and hands
//! each plan to the kernel worker. Responses are folded back into the
//! document by `drain_kernel_responses`.

use kernel_api::TessellationSettings;

use crate::PrintCadApp;
use crate::log_panel as app_log;

impl PrintCadApp {
    pub(crate) fn drive_part_recompute(&mut self) {
        self.sync_feature_preview();
        // Nothing is out on the kernel thread: a body still marked as
        // building lost its answer (it left the tab while it built), and
        // what waits for it goes now.
        if self.kernel_worker.in_flight() == 0 && !self.session.builds_in_flight.is_empty() {
            let waiting: Vec<_> = self.session.builds_in_flight.drain().collect();
            for (body, next) in waiting {
                if let Some(next) = next {
                    self.submit_build(body, next);
                }
            }
        }
        for job in self.registry.rebuild_jobs(&mut self.session.document) {
            let body_id = job.body;
            self.session.document.clear_body_feature_errors(body_id);
            match job.plan {
                Ok(plan) if plan.ops.is_empty() => {
                    // Only geometry the features produced is cleared; an
                    // imported solid outlives an empty history.
                    if !self.session.document.body_solid_is_imported(body_id) {
                        self.session.document.remove_imported_geometry(body_id);
                    }
                }
                Ok(plan) => {
                    let preview = self
                        .session
                        .preview_feature
                        .filter(|f| plan.op_features.contains(f))
                        .map(|f| f.0);
                    let build = QueuedBuild {
                        ops: plan.ops,
                        op_features: plan.op_features.iter().map(|id| id.0).collect(),
                        preview,
                        probes: plan.probes,
                    };
                    // One build per body at a time; a newer plan waits in
                    // its place, replacing any older one waiting.
                    match self.session.builds_in_flight.get_mut(&body_id.0) {
                        Some(waiting) => *waiting = Some(build),
                        None => self.submit_build(body_id.0, build),
                    }
                }
                Err(err) => {
                    let name = err
                        .feature
                        .and_then(|f| self.session.document.get_feature_meta(f))
                        .map(|n| format!("`{}`: ", n.name))
                        .unwrap_or_default();
                    if let Some(feature) = err.feature {
                        self.session
                            .document
                            .set_feature_error(feature, Some(err.message.clone()));
                    }
                    app_log::warn(format!("Recompute skipped: {name}{err}"));
                }
            }
        }
    }
}

/// A body's build plan, as it goes to the kernel thread.
pub(crate) struct QueuedBuild {
    ops: Vec<kernel_api::SolidOp>,
    op_features: Vec<uuid::Uuid>,
    preview: Option<uuid::Uuid>,
    probes: Vec<core_document::PlanProbe>,
}

impl PrintCadApp {
    /// How finely built, repaired, converted and replaced solids are meshed
    /// for the scene (Preferences › Display › Rendering, Curve smoothness).
    pub(crate) fn solid_detail(&self) -> TessellationSettings {
        TessellationSettings {
            angular_tolerance_deg: self.user_settings.rendering.curve_step_deg.clamp(2.0, 45.0),
            ..TessellationSettings::default()
        }
    }

    fn submit_build(&mut self, body: uuid::Uuid, build: QueuedBuild) {
        self.session.builds_in_flight.insert(body, None);
        self.kernel_worker.request_build_solid(
            body,
            build.ops,
            build.op_features,
            self.solid_detail(),
            build.preview,
            build.probes,
        );
    }

    /// A body's build landed: the newest plan made meanwhile goes now.
    pub(crate) fn build_landed(&mut self, body: uuid::Uuid) {
        if let Some(Some(next)) = self.session.builds_in_flight.remove(&body) {
            self.submit_build(body, next);
        }
    }

    /// Follow the feature the active bench edits: while a task edits one
    /// that builds solid, its body is built with a preview of it, and when
    /// the task closes the whole solids go back.
    fn sync_feature_preview(&mut self) {
        let document = &self.session.document;
        let editing = self
            .registry
            .workbench(&self.session.active_workbench.0)
            .ok()
            .and_then(|wb| wb.editing_feature())
            .filter(|feature| {
                document.get_feature_meta(*feature).is_some_and(|node| {
                    node.body.is_some()
                        && self
                            .registry
                            .feature_info(node)
                            .is_some_and(|info| info.builds_solid)
                })
            });
        if editing == self.session.preview_feature {
            return;
        }
        self.end_feature_previews();
        self.session.preview_feature = editing;
        if let Some(feature) = editing {
            // Built again, this time with its preview.
            self.session.document.mark_feature_stale(feature);
        }
    }

    /// Put every body showing a preview back to its whole solid.
    pub(crate) fn end_feature_previews(&mut self) {
        for (body, preview) in std::mem::take(&mut self.session.previews) {
            if self.session.document.bodies().iter().any(|b| b.id == body) {
                store_built_solid(&mut self.session.document, body, preview.full);
            }
        }
    }

    /// A build that came with the edited feature's preview: the body stands
    /// without the feature (before one that adds, after one that cuts) and
    /// the feature's tool is drawn over it, the whole solid kept aside.
    pub(crate) fn show_feature_preview(
        &mut self,
        body: core_document::BodyId,
        full: kernel_api::SolidBuildResult,
        preview: kernel_api::FeaturePreview,
    ) {
        let document = &mut self.session.document;
        match preview.shown {
            Some(shown) => store_built_solid(document, body, *shown),
            None => document.remove_imported_geometry(body),
        }
        let placement = document.body_placement(body);
        let tool = if placement.is_identity() {
            preview.tool
        } else {
            placement.mesh(&preview.tool)
        };
        let (id, revision) = match self.session.previews.get(&body) {
            Some(previous) => (previous.id, previous.revision.wrapping_add(1)),
            None => (uuid::Uuid::new_v4(), 0),
        };
        self.session.previews.insert(
            body,
            crate::app::session::BodyPreview {
                full,
                tool: std::sync::Arc::new(tool),
                id,
                revision,
            },
        );
    }

    /// Hand every body whose repair was asked for, and whose geometry is
    /// not yet the repaired shape, to the kernel worker. The request is an
    /// op, so this runs the same for a local request, a peer's, and a
    /// document reopened before its repair landed.
    pub(crate) fn drive_shape_repairs(&mut self) {
        for body in self.session.document.bodies_awaiting_repair() {
            if self.session.repairs_in_flight.contains(&body.0) {
                continue;
            }
            // A body with a base mends its base, which its features build on.
            let document = &self.session.document;
            let (blob, face_colors) = if document.has_base_solid(body) {
                (
                    document
                        .base_brep_blob(body)
                        .map(|b| std::sync::Arc::new(b.to_vec())),
                    document.base_face_colors(body).map(<[_]>::to_vec),
                )
            } else {
                (
                    document.imported_brep_blob_arc(body),
                    document.imported_brep_face_colors(body).map(<[_]>::to_vec),
                )
            };
            let Some(blob) = blob else {
                continue;
            };
            let face_colors = face_colors.unwrap_or_default();
            self.session.repairs_in_flight.insert(body.0);
            app_log::info(format!("Repairing `{}`…", self.body_name(body)));
            self.kernel_worker
                .request_repair(body.0, blob, face_colors, self.solid_detail());
        }
    }

    /// Read the new shape of every body whose shape was replaced by
    /// another file (`ReplaceBodyShape`), each once, on the kernel thread.
    pub(crate) fn drive_shape_replacements(&mut self) {
        for (body, asset) in self.session.document.bodies_awaiting_shape() {
            if self.session.shapes_in_flight.contains(&body.0)
                || self.session.shapes_failed.get(&body.0) == Some(&asset)
            {
                continue;
            }
            let Some((reference, bytes)) = self.session.document.asset_with_bytes(asset) else {
                continue;
            };
            // The kernel reads files: a copy of the asset under its format's
            // extension, removed by the worker when read.
            let name = std::path::Path::new(&reference.path)
                .file_name()
                .map(|n| n.to_owned())
                .unwrap_or_else(|| format!("{asset}.step").into());
            let dir = std::env::temp_dir().join("printcad").join("shapes");
            let path = dir.join(name);
            if let Err(e) =
                std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, &*bytes))
            {
                self.session.shapes_failed.insert(body.0, asset);
                app_log::error(format!(
                    "Could not stage the new shape of `{}`: {e}",
                    self.body_name(body)
                ));
                continue;
            }
            self.session.shapes_in_flight.insert(body.0);
            app_log::info(format!(
                "Reading the new shape of `{}`…",
                self.body_name(body)
            ));
            self.kernel_worker
                .request_read_solid(body.0, asset, path, self.solid_detail());
        }
    }

    /// Land a body's new shape: as its base, which its features then build
    /// on, or as its shape when it has no history.
    pub(crate) fn apply_shape_read(
        &mut self,
        body: core_document::BodyId,
        asset: uuid::Uuid,
        result: Result<kernel_api::MeshSolidResult, String>,
        elapsed: std::time::Duration,
    ) {
        self.session.shapes_in_flight.remove(&body.0);
        let name = self.body_name(body);
        let read = match result {
            Ok(read) => read,
            Err(error) => {
                self.session.shapes_failed.insert(body.0, asset);
                app_log::error(format!(
                    "The new shape of `{name}` could not be read: {error}"
                ));
                return;
            }
        };
        let geometry = core_document::ImportedGeometry {
            bounds_mm: read.bounds_mm.or_else(|| read.mesh.bounds()),
            mesh: std::sync::Arc::new(read.mesh),
            source_asset: Some(asset),
            revision: 0,
            brep_blob_path: None,
            face_colors_path: None,
            health: Some(read.health),
        };
        let document = &mut self.session.document;
        if document.has_base_solid(body) {
            document.set_base_solid(body, geometry, read.brep_blob, read.face_colors);
            // Its features build again on the new base.
            self.registry.invalidate_body(document, body);
        } else {
            document.set_imported_geometry(body, geometry);
            document.set_imported_brep_data(body, read.brep_blob, read.face_colors);
        }
        let notes = if read.summary.is_empty() {
            String::new()
        } else {
            format!(" ({})", read.summary.join(", "))
        };
        app_log::success(format!(
            "`{name}` takes its new shape, read in {:.1} s{notes}",
            elapsed.as_secs_f32()
        ));
    }

    /// Land a repaired shape on its body: the mended snapshot, its mesh and
    /// the checker's verdict, which clears the body from the queue.
    pub(crate) fn apply_shape_repair(
        &mut self,
        body: core_document::BodyId,
        result: kernel_api::RepairResult,
        elapsed: std::time::Duration,
    ) {
        self.session.repairs_in_flight.remove(&body.0);
        if let Some(previous) = self.session.document.base_geometry(body).cloned() {
            let broken = result.health.broken;
            self.session.document.set_base_solid(
                body,
                core_document::ImportedGeometry {
                    mesh: std::sync::Arc::new(result.mesh),
                    bounds_mm: result.bounds_mm.or(previous.bounds_mm),
                    health: Some(result.health),
                    ..previous
                },
                result.brep_blob,
                result.face_colors,
            );
            // The features build again on the mended base.
            self.registry
                .invalidate_body(&mut self.session.document, body);
            app_log::info(format!(
                "Repaired the base of `{}` in {:.1} s{}",
                self.body_name(body),
                elapsed.as_secs_f32(),
                if broken > 0 {
                    format!(": {broken} defect(s) remain that the repair does not mend")
                } else {
                    String::new()
                }
            ));
            return;
        }
        let Some(previous) = self.session.document.imported_geometry(body).cloned() else {
            // The body left the document while the repair ran.
            return;
        };
        let name = self.body_name(body);
        self.session
            .document
            .set_imported_brep_data(body, result.brep_blob, result.face_colors);
        let health = result.health;
        let verdict = if health.is_broken() {
            format!(
                "{} defect(s) remain that the repair does not mend",
                health.broken
            )
        } else {
            "the shape checks clean".to_string()
        };
        let mended = if result.mended.is_empty() {
            "nothing to mend".to_string()
        } else {
            result.mended.join(", ")
        };
        self.session.document.set_imported_geometry(
            body,
            core_document::ImportedGeometry {
                mesh: std::sync::Arc::new(result.mesh),
                bounds_mm: result.bounds_mm.or(previous.bounds_mm),
                health: Some(health),
                ..previous
            },
        );
        if self.session.face_highlight.as_ref().map(|f| f.body) == Some(body.0) {
            // The face sub-mesh belongs to the replaced shape.
            self.session.face_highlight = None;
            self.session.last_face_hit = None;
        }
        self.session.hovered_face = None;
        app_log::info(format!(
            "Repaired `{name}` in {:.0}ms: {mended}; {verdict}",
            elapsed.as_secs_f64() * 1000.0
        ));
    }

    /// A body's name, for log lines.
    pub(crate) fn body_name(&self, body: core_document::BodyId) -> String {
        self.session
            .document
            .bodies()
            .iter()
            .find(|b| b.id == body)
            .map(|b| b.name.clone())
            .unwrap_or_else(|| "body".to_string())
    }
}

impl PrintCadApp {
    /// The body the property panel is showing: a body row, or an imported
    /// part linked to one.
    fn panel_body(&self) -> Option<core_document::BodyId> {
        match self.session.tree_selection? {
            crate::ui::TreeItemId::Body(body) => Some(body),
            crate::ui::TreeItemId::ImportedObject(id) => {
                self.session.document.body_of_imported_object(id)
            }
            _ => None,
        }
    }

    /// Ask the kernel worker to measure the body the property panel shows,
    /// once per revision of its geometry.
    pub(crate) fn drive_measurement(&mut self) {
        let Some(body) = self.panel_body() else {
            return;
        };
        let Some(revision) = self
            .session
            .document
            .imported_geometry(body)
            .map(|g| g.revision)
        else {
            return;
        };
        if self
            .session
            .physical
            .get(&body.0)
            .is_some_and(|(measured, _)| *measured == revision)
        {
            return;
        }
        let Some(blob) = self.session.document.imported_brep_blob_arc(body) else {
            return;
        };
        self.session
            .physical
            .insert(body.0, (revision, crate::ui::Physical::Measuring));
        self.kernel_worker.request_measure(body.0, revision, blob);
    }

    /// The panel body's measure, when it is of the geometry on screen.
    pub(crate) fn panel_physical(&self) -> Option<crate::ui::Physical> {
        let body = self.panel_body()?;
        let revision = self.session.document.imported_geometry(body)?.revision;
        let (measured, reading) = self.session.physical.get(&body.0)?;
        if *measured != revision {
            return None;
        }
        // The kernel measures the body's own shape; the centre is shown
        // where the body sits.
        Some(match reading {
            crate::ui::Physical::Ready(props) => {
                let mut props = *props;
                let placement = self.session.document.body_placement(body);
                let c = props.centre_mm.map(|v| v as f32);
                props.centre_mm = placement.point(c).map(f64::from);
                crate::ui::Physical::Ready(props)
            }
            other => other.clone(),
        })
    }
}

impl PrintCadApp {
    /// Hand every mesh body whose conversion was asked for, and has not
    /// landed, to the kernel worker. The request is an op: a peer's and a
    /// reopened document's convert the same way.
    pub(crate) fn drive_mesh_solids(&mut self) {
        for body in self.session.document.bodies_awaiting_solid() {
            if self.session.solids_in_flight.contains(&body.0) {
                continue;
            }
            // The solid is built in the body's own frame, as every kernel
            // shape is; the scene's copy is placed.
            let Some((mesh, _)) = self.session.document.local_geometry(body) else {
                continue;
            };
            self.session.solids_in_flight.insert(body.0);
            app_log::info(format!(
                "Converting `{}` to a solid ({} triangles)…",
                self.body_name(body),
                mesh.indices.len() / 3
            ));
            self.kernel_worker
                .request_mesh_solid(body.0, mesh, self.solid_detail());
        }
    }

    /// Ask the kernel for the snapshot of every mirrored copy that has
    /// none: its source's, mirrored.
    pub(crate) fn drive_mirrored_copies(&mut self) {
        for (body, blob, plane) in self.session.document.copies_awaiting_shape() {
            let failed = self
                .session
                .mirrors_failed
                .get(&body.0)
                .is_some_and(|f| std::sync::Arc::ptr_eq(f, &blob));
            if !failed && self.session.mirrors_in_flight.insert(body.0) {
                self.kernel_worker.request_mirror(body.0, blob, plane);
            }
        }
    }

    /// Land a mesh body's solid: its snapshot, its mesh with kernel faces
    /// and edges, the checker's verdict, and a line on what it became.
    pub(crate) fn apply_mesh_solid(
        &mut self,
        body: core_document::BodyId,
        result: kernel_api::MeshSolidResult,
        elapsed: std::time::Duration,
    ) {
        self.session.solids_in_flight.remove(&body.0);
        let Some(previous) = self.session.document.imported_geometry(body).cloned() else {
            return;
        };
        let name = self.body_name(body);
        self.session
            .document
            .set_imported_brep_data(body, result.brep_blob, result.face_colors);
        self.session.document.set_imported_geometry(
            body,
            core_document::ImportedGeometry {
                mesh: std::sync::Arc::new(result.mesh),
                bounds_mm: result.bounds_mm.or(previous.bounds_mm),
                health: Some(result.health),
                ..previous
            },
        );
        if self.session.face_highlight.as_ref().map(|f| f.body) == Some(body.0) {
            self.session.face_highlight = None;
            self.session.last_face_hit = None;
        }
        self.session.hovered_face = None;
        let summary = result.summary.join(", ");
        let ms = elapsed.as_secs_f64() * 1000.0;
        if result.closed {
            app_log::info(format!(
                "Converted `{name}` to a solid in {ms:.0}ms: {summary}"
            ));
        } else {
            app_log::warn(format!(
                "`{name}` does not close, so it became an open shell, not a solid \
                 ({ms:.0}ms): {summary}"
            ));
        }
    }
}

/// Put a body's rebuilt solid in the document: its shape and the mesh drawn
/// from it.
pub(crate) fn store_built_solid(
    document: &mut core_document::Document,
    body: core_document::BodyId,
    result: kernel_api::SolidBuildResult,
) {
    let bounds_mm = result.bounds_mm;
    document.set_imported_brep_data(body, result.brep_blob, Vec::new());
    document.set_imported_geometry(
        body,
        core_document::ImportedGeometry {
            mesh: std::sync::Arc::new(result.mesh),
            source_asset: None,
            revision: 0,
            bounds_mm,
            brep_blob_path: None,
            face_colors_path: None,
            health: None,
        },
    );
}
