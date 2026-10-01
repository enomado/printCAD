//! A body's surface textures, ready to press into a mesh of it: each with
//! the faces it covers, found in the body's mesh as it stands, and a
//! picture's heights, read from its document asset.

use std::collections::HashMap;
use std::sync::Arc;

use core_document::{BodyId, Document};
use kernel_api::TriMesh;
use surface_texture::{Detail, HeightMap, Job, Pattern, Texture};
use uuid::Uuid;

/// The widest a picture's heights are read, pixels.
const PICTURE_PX: u32 = 1024;

/// Pictures read so far, by asset; `None` for one that would not read.
pub(crate) type Pictures = HashMap<Uuid, Option<Arc<HeightMap>>>;

/// One texture with its faces and heights.
#[derive(Debug, Clone)]
struct Pressed {
    texture: Texture,
    faces: Vec<u32>,
    image: Option<Arc<HeightMap>>,
}

/// Every texture of a body, ready to press into a mesh of it.
#[derive(Debug, Clone)]
pub(crate) struct Pressing {
    textures: Vec<Pressed>,
    /// What they are, for knowing a preview still stands for them.
    pub key: u64,
}

impl Pressing {
    /// `body`'s textures, its faces found in its mesh; `None` when it has
    /// none.
    pub(crate) fn of(document: &Document, body: BodyId, pictures: &mut Pictures) -> Option<Self> {
        let textures = &document.bodies().iter().find(|b| b.id == body)?.textures;
        if textures.is_empty() {
            return None;
        }
        let mesh = &document.imported_geometry(body)?.mesh;
        let textures: Vec<Pressed> = textures
            .iter()
            .map(|t| Pressed {
                texture: t.texture,
                faces: t.face_indices(mesh),
                image: match t.texture.pattern {
                    Pattern::Image { asset } => pictures
                        .entry(asset)
                        .or_insert_with(|| {
                            let bytes = document.asset_bytes(asset)?;
                            // Read once, so a picture that does not read is
                            // said once and its texture lies flat.
                            match HeightMap::from_image(bytes, PICTURE_PX) {
                                Ok(map) => Some(Arc::new(map)),
                                Err(err) => {
                                    crate::log_panel::warn(format!(
                                        "A texture's picture does not read, so it presses \
                                         nothing: {err}"
                                    ));
                                    None
                                }
                            }
                        })
                        .clone(),
                    _ => None,
                },
            })
            .collect();
        let key = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            for t in &textures {
                format!("{:?}", t.texture).hash(&mut hasher);
                t.faces.hash(&mut hasher);
                t.image
                    .as_ref()
                    .map(|i| (i.width, i.height))
                    .hash(&mut hasher);
            }
            hasher.finish()
        };
        Some(Self { textures, key })
    }

    /// `mesh` with the textures pressed in, as fine as `detail` asks.
    pub(crate) fn apply(&self, mesh: &TriMesh, detail: Detail) -> TriMesh {
        let jobs: Vec<Job<'_>> = self
            .textures
            .iter()
            .map(|t| Job {
                texture: &t.texture,
                faces: &t.faces,
                image: t.image.as_deref(),
            })
            .collect();
        surface_texture::apply(mesh, &jobs, detail)
    }
}

/// A textured body as drawn: the mesh made last, and what it was made
/// from; while a newer one is being made, the last stays on screen.
#[derive(Debug, Clone, Default)]
pub(crate) struct Preview {
    /// The key of what is wanted now.
    pub wanted: u64,
    /// The key of the one being made, if any.
    pub making: Option<u64>,
    /// The last one made, with its key.
    pub made: Option<(u64, Arc<TriMesh>)>,
}

/// A preview made on its thread.
pub(crate) struct Made {
    pub body: BodyId,
    pub key: u64,
    pub mesh: Arc<TriMesh>,
}

impl crate::PrintCadApp {
    /// Keep each visible textured body's preview up to its textures, its
    /// shape and where it sits: take in what the threads made, and start
    /// one for a body whose preview is out of date and not being made.
    pub(crate) fn drive_texture_previews(&mut self) {
        if let Some(rx) = &self.session.textured_rx {
            while let Ok(made) = rx.try_recv() {
                if let Some(preview) = self.session.textured.get_mut(&made.body) {
                    if preview.making == Some(made.key) {
                        preview.making = None;
                    }
                    preview.made = Some((made.key, made.mesh));
                }
            }
        }
        let document = &self.session.document;
        let bodies: Vec<BodyId> = document
            .bodies()
            .iter()
            .filter(|b| !b.textures.is_empty() && document.imported_body_effective_visible(b.id))
            .map(|b| b.id)
            .collect();
        self.session
            .textured
            .retain(|body, _| bodies.contains(body));
        for body in bodies {
            let document = &self.session.document;
            let Some(pressing) = Pressing::of(document, body, &mut self.session.texture_pictures)
            else {
                continue;
            };
            let Some(geometry) = document.imported_geometry(body) else {
                continue;
            };
            let placement = document.body_placement(body);
            let key = {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                (pressing.key, geometry.revision).hash(&mut hasher);
                format!("{:?}", placement.rows()).hash(&mut hasher);
                Arc::as_ptr(&geometry.mesh).hash(&mut hasher);
                hasher.finish()
            };
            let preview = self.session.textured.entry(body).or_default();
            preview.wanted = key;
            let current = preview.made.as_ref().is_some_and(|(k, _)| *k == key);
            if current || preview.making.is_some() {
                continue;
            }
            let Some((local, _)) = document.local_geometry(body) else {
                continue;
            };
            preview.making = Some(key);
            let tx = self
                .session
                .textured_tx
                .get_or_insert_with(|| {
                    let (tx, rx) = std::sync::mpsc::channel();
                    self.session.textured_rx = Some(rx);
                    tx
                })
                .clone();
            let spawned = std::thread::Builder::new()
                .name("printcad-texture".to_string())
                .spawn(move || {
                    let pressed = pressing.apply(&local, Detail::PREVIEW);
                    let placed = if placement.is_identity() {
                        pressed
                    } else {
                        placement.mesh(&pressed)
                    };
                    let _ = tx.send(Made {
                        body,
                        key,
                        mesh: Arc::new(placed),
                    });
                });
            if let Err(err) = spawned {
                crate::log_panel::warn(format!("The texture preview could not start: {err}"));
                if let Some(preview) = self.session.textured.get_mut(&body) {
                    preview.making = None;
                }
            }
        }
    }

    /// Whether a texture preview is being made: frames keep coming until
    /// it lands.
    pub(crate) fn texture_previews_pending(&self) -> bool {
        self.session.textured.values().any(|p| p.making.is_some())
    }
}
