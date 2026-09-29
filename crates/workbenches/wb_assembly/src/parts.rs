//! The parts list: every body, identical ones counted together, with the
//! size of its box in its own frame (what a print bed has to hold).
//!
//! What the list says of each part beyond that (its item number, whether
//! it is bought rather than made, the values of the columns added to the
//! list) is kept in the document as one body-less feature, the parts
//! table, by the id of a body of the part.

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use core_document::{
    BodyId, ComponentId, Document, DocumentResult, FeatureError, FeatureId, WorkbenchFeature,
    WorkbenchId,
};
use serde::{Deserialize, Serialize};

/// The feature kind the parts table is stored as.
pub const PARTS_KIND: &str = "wb.assembly.parts";

/// What the parts list keeps for its parts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartsTable {
    /// The columns added to the list, in order.
    pub columns: Vec<String>,
    /// Each part's entry, by the id of one of its bodies.
    pub entries: BTreeMap<String, PartEntry>,
    /// Listed by component: each component's parts under it, nested as
    /// the components are.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub by_component: bool,
}

/// What the list keeps for one part.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartEntry {
    /// Its item number; 0 before the list is numbered.
    pub number: u32,
    /// Bought rather than made: left out of exports and the slicer.
    pub bought: bool,
    /// Its value in each added column, by column.
    pub values: BTreeMap<String, String>,
}

impl WorkbenchFeature for PartsTable {
    fn workbench_id() -> WorkbenchId {
        WorkbenchId::from(PARTS_KIND)
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    fn from_json(value: &serde_json::Value) -> DocumentResult<Self> {
        serde_json::from_value(value.clone()).map_err(|e| {
            core_document::DocumentError::Feature(FeatureError::Deserialization(e.to_string()))
        })
    }

    fn dependencies(&self) -> Vec<FeatureId> {
        Vec::new()
    }

    fn name(&self) -> &str {
        "Parts list"
    }
}

/// The document's parts table and its feature, when it has one.
pub fn table_of(document: &Document) -> Option<(FeatureId, PartsTable)> {
    document
        .feature_tree()
        .all_nodes()
        .filter(|(_, n)| n.workbench_id.as_str() == PARTS_KIND)
        .min_by_key(|(id, n)| (n.seq, **id))
        .and_then(|(id, n)| Some((*id, PartsTable::from_json(&n.data).ok()?)))
}

/// Write the parts table, adding its feature the first time.
pub fn store_table(document: &mut Document, table: &PartsTable) -> DocumentResult<FeatureId> {
    match table_of(document) {
        Some((id, _)) => {
            document.update_feature_data(id, table.to_json())?;
            document.clear_feature_dirty(id);
            Ok(id)
        }
        None => {
            let id = document.add_feature_in_body(table.clone(), "Parts list".into(), None)?;
            document.clear_feature_dirty(id);
            Ok(id)
        }
    }
}

impl PartsTable {
    /// The entry of the part `bodies` make, any of them keyed.
    pub fn entry(&self, bodies: &[BodyId]) -> Option<&PartEntry> {
        bodies
            .iter()
            .find_map(|b| self.entries.get(&b.0.to_string()))
    }

    /// The entry of the part `bodies` make, made under its first body
    /// when it has none.
    pub fn entry_mut(&mut self, bodies: &[BodyId]) -> &mut PartEntry {
        let key = bodies
            .iter()
            .map(|b| b.0.to_string())
            .find(|k| self.entries.contains_key(k))
            .or_else(|| bodies.first().map(|b| b.0.to_string()))
            .unwrap_or_default();
        self.entries.entry(key).or_default()
    }

    /// Number every part that has no number, after the highest in use, in
    /// the order given.
    pub fn number(&mut self, parts: &[Part]) {
        let mut next = self.entries.values().map(|e| e.number).max().unwrap_or(0);
        for part in parts {
            let entry = self.entry_mut(&part.bodies);
            if entry.number == 0 {
                next += 1;
                entry.number = next;
            }
        }
    }
}

/// One part: the bodies that are the same shape, and its size.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub name: String,
    pub bodies: Vec<BodyId>,
    /// Its box along its own X, Y and Z, in millimetres; `None` for a body
    /// with no geometry yet.
    pub size_mm: Option<[f32; 3]>,
    /// A mesh body rather than a solid.
    pub mesh: bool,
    /// Its item number, once the list is numbered.
    pub number: Option<u32>,
    /// Bought rather than made.
    pub bought: bool,
    /// Its values in the list's added columns.
    pub values: BTreeMap<String, String>,
}

/// The bodies of bought parts: what an export of the model, or the
/// slicer, leaves out.
pub fn bought_bodies(document: &Document) -> Vec<BodyId> {
    parts_list(document)
        .into_iter()
        .filter(|p| p.bought)
        .flat_map(|p| p.bodies)
        .collect()
}

/// Every body, bodies of the same shape as one part, in name order.
pub fn parts_list(document: &Document) -> Vec<Part> {
    let mut parts: Vec<(u64, Part)> = Vec::new();
    for body in document.bodies() {
        let local = document.local_geometry(body.id);
        let size_mm = local
            .as_ref()
            .and_then(|(mesh, bounds)| bounds.or_else(|| mesh.bounds()))
            .map(|(lo, hi)| [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]);
        // The same shape is the same snapshot, or failing one the same
        // triangles; a body with neither is a part of its own.
        let mut hasher = DefaultHasher::new();
        match (document.imported_brep_blob(body.id), &local) {
            (Some(blob), _) => blob.hash(&mut hasher),
            (None, Some((mesh, _))) if !mesh.positions.is_empty() => {
                for p in &mesh.positions {
                    p.map(f32::to_bits).hash(&mut hasher);
                }
            }
            _ => body.id.hash(&mut hasher),
        }
        let key = hasher.finish();
        match parts.iter_mut().find(|(k, _)| *k == key) {
            Some((_, part)) => part.bodies.push(body.id),
            None => parts.push((
                key,
                Part {
                    name: body.name.clone(),
                    bodies: vec![body.id],
                    size_mm,
                    mesh: document.is_mesh_body(body.id),
                    number: None,
                    bought: false,
                    values: BTreeMap::new(),
                },
            )),
        }
    }
    let table = table_of(document).map(|(_, t)| t).unwrap_or_default();
    let mut out: Vec<Part> = parts
        .into_iter()
        .map(|(_, mut p)| {
            if let Some(entry) = table.entry(&p.bodies) {
                p.number = (entry.number > 0).then_some(entry.number);
                p.bought = entry.bought;
                p.values = entry.values.clone();
            }
            p
        })
        .collect();
    // Numbered parts by number, then the rest by name.
    out.sort_by_key(|p| (p.number.unwrap_or(u32::MAX), p.name.to_lowercase()));
    out
}

/// A row of the list by component.
#[derive(Debug, Clone, PartialEq)]
pub enum LevelRow {
    /// A component, `depth` components down.
    Component {
        depth: usize,
        id: ComponentId,
        name: String,
    },
    /// Bodies of `parts[part]` sitting directly in the component above.
    Part {
        depth: usize,
        part: usize,
        bodies: Vec<BodyId>,
    },
}

/// `parts` (the whole list) by component: the top's components, each
/// followed by what it holds, then the top's own parts; in a component,
/// its components before its parts, parts in list order.
pub fn parts_by_component(document: &Document, parts: &[Part]) -> Vec<LevelRow> {
    fn level(
        document: &Document,
        parts: &[Part],
        at: Option<ComponentId>,
        depth: usize,
        out: &mut Vec<LevelRow>,
    ) {
        for component in document.components() {
            let parent = component
                .parent
                .filter(|p| document.component(*p).is_some());
            if parent == at && depth < 64 {
                out.push(LevelRow::Component {
                    depth,
                    id: component.id,
                    name: component.name.clone(),
                });
                level(document, parts, Some(component.id), depth + 1, out);
            }
        }
        for (i, part) in parts.iter().enumerate() {
            let bodies: Vec<BodyId> = part
                .bodies
                .iter()
                .copied()
                .filter(|b| document.component_of(*b) == at)
                .collect();
            if !bodies.is_empty() {
                out.push(LevelRow::Part {
                    depth,
                    part: i,
                    bodies,
                });
            }
        }
    }
    let mut out = Vec::new();
    level(document, parts, None, 0, &mut out);
    out
}

/// A value as a CSV field: quoted when it holds a comma or a quote.
fn field(text: &str) -> String {
    if text.contains([',', '"', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

/// The list as comma-separated values, a header line first: the item
/// number, part, quantity, size, kind, whether bought, then the added
/// `columns`.
pub fn parts_csv(parts: &[Part], columns: &[String]) -> String {
    let mut out =
        String::from("Item,Part,Quantity,Size X (mm),Size Y (mm),Size Z (mm),Kind,Bought");
    for column in columns {
        out.push(',');
        out.push_str(&field(column));
    }
    out.push('\n');
    for part in parts {
        let size = part.size_mm.map_or_else(
            || ",,".to_string(),
            |s| format!("{:.2},{:.2},{:.2}", s[0], s[1], s[2]),
        );
        let kind = if part.mesh { "mesh" } else { "solid" };
        let number = part.number.map_or(String::new(), |n| n.to_string());
        let bought = if part.bought { "yes" } else { "" };
        out.push_str(&format!(
            "{number},{},{},{size},{kind},{bought}",
            field(&part.name),
            part.bodies.len()
        ));
        for column in columns {
            out.push(',');
            out.push_str(&field(part.values.get(column).map_or("", String::as_str)));
        }
        out.push('\n');
    }
    out
}

/// The list by component as comma-separated values: a level column (0
/// at the top) before `parts_csv`'s, a component a row of its own.
pub fn levels_csv(parts: &[Part], rows: &[LevelRow], columns: &[String]) -> String {
    let flat = parts_csv(&[], columns);
    let mut out = format!("Level,{flat}");
    for row in rows {
        match row {
            LevelRow::Component { depth, name, .. } => {
                out.push_str(&format!("{depth},,{},1,,,,component,", field(name)));
                out.push_str(&",".repeat(columns.len()));
                out.push('\n');
            }
            LevelRow::Part {
                depth,
                part,
                bodies,
            } => {
                let one = Part {
                    bodies: bodies.clone(),
                    ..parts[*part].clone()
                };
                let line = parts_csv(&[one], columns);
                let line = line.lines().nth(1).unwrap_or_default();
                out.push_str(&format!("{depth},{line}\n"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_by_component_nests_its_parts() {
        let mut document = Document::new("t");
        let [bolt, bolt2, lid, base] =
            ["Bolt", "Bolt 2", "Lid", "Base"].map(|n| document.create_body(Some(n.into())));
        for (body, shape) in [
            (bolt, "bolt"),
            (bolt2, "bolt"),
            (lid, "lid"),
            (base, "base"),
        ] {
            document.set_imported_brep_data(body, shape.as_bytes().to_vec(), Vec::new());
        }
        let top = document.create_component("Top".into(), None).unwrap();
        document.set_body_component(bolt, Some(top)).unwrap();
        document.set_body_component(lid, Some(top)).unwrap();
        let parts = parts_list(&document);
        let rows = parts_by_component(&document, &parts);
        let names: Vec<(usize, String, usize)> = rows
            .iter()
            .map(|r| match r {
                LevelRow::Component { depth, name, .. } => (*depth, name.clone(), 1),
                LevelRow::Part {
                    depth,
                    part,
                    bodies,
                } => (*depth, parts[*part].name.clone(), bodies.len()),
            })
            .collect();
        assert_eq!(
            names,
            [
                (0, "Top".to_string(), 1),
                (1, "Bolt".to_string(), 1),
                (1, "Lid".to_string(), 1),
                (0, "Base".to_string(), 1),
                (0, "Bolt".to_string(), 1),
            ]
        );
        let csv = levels_csv(&parts, &rows, &[]);
        let lines: Vec<&str> = csv.lines().collect();
        assert!(lines[0].starts_with("Level,Item,Part"));
        assert_eq!(lines[1], "0,,Top,1,,,,component,");
        assert_eq!(lines[2], "1,,Bolt,1,,,,solid,");
    }

    #[test]
    fn bodies_of_one_shape_are_one_part_counted() {
        let mut document = Document::new("t");
        let a = document.create_body(Some("Bolt".into()));
        let b = document.create_body(Some("Bolt 2".into()));
        let c = document.create_body(Some("Bracket, left".into()));
        document.set_imported_brep_data(a, b"bolt".to_vec(), Vec::new());
        document.set_imported_brep_data(b, b"bolt".to_vec(), Vec::new());
        document.set_imported_brep_data(c, b"bracket".to_vec(), Vec::new());
        let parts = parts_list(&document);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].name, "Bolt");
        assert_eq!(parts[0].bodies, [a, b]);
        let csv = parts_csv(&parts, &[]);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[1], ",Bolt,2,,,,solid,");
        assert_eq!(lines[2], ",\"Bracket, left\",1,,,,solid,");
    }

    /// The table numbers the parts, keeps each part's bought mark and
    /// column values in the document, and the list reads them back in
    /// number order.
    #[test]
    fn the_parts_table_is_kept_in_the_document() {
        let mut document = Document::new("t");
        let a = document.create_body(Some("Zeta".into()));
        let b = document.create_body(Some("Alpha".into()));
        document.set_imported_brep_data(a, b"z".to_vec(), Vec::new());
        document.set_imported_brep_data(b, b"a".to_vec(), Vec::new());
        let mut table = PartsTable {
            columns: vec!["Supplier".into()],
            ..PartsTable::default()
        };
        let parts = parts_list(&document);
        table.number(&parts);
        table.entry_mut(&[a]).bought = true;
        table
            .entry_mut(&[a])
            .values
            .insert("Supplier".into(), "ACME, Inc".into());
        store_table(&mut document, &table).unwrap();
        let parts = parts_list(&document);
        assert_eq!(parts[0].name, "Alpha", "numbered first by name");
        assert_eq!(parts[0].number, Some(1));
        assert_eq!(parts[1].number, Some(2));
        assert!(parts[1].bought);
        assert_eq!(bought_bodies(&document), [a]);
        let csv = parts_csv(&parts, &table.columns);
        assert!(csv.starts_with("Item,Part,Quantity"), "{csv}");
        assert!(
            csv.lines().nth(2).unwrap().ends_with(",yes,\"ACME, Inc\""),
            "{csv}"
        );
        // Stored once: a second store updates the same feature.
        let (id, _) = table_of(&document).unwrap();
        assert_eq!(store_table(&mut document, &table).unwrap(), id);
    }
}
