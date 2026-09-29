//! Ids that were renamed, and what they are called now. What was written
//! with an old id keeps working: a document's or an op log's feature kinds
//! read as the new ones, a command called by its old name (a script, a
//! recording, an agent) runs the new one, and settings keyed by old ids
//! move to the new ones when they load.

use std::borrow::Cow;

/// Workbench ids and feature kinds: old, new.
pub const WORKBENCHES: &[(&str, &str)] = &[("wb.part", "wb.design")];

/// Command, tool and action id prefixes: old, new.
pub const COMMAND_PREFIXES: &[(&str, &str)] = &[("part.", "design.")];

/// The workbench id or feature kind `id` is called now.
pub fn workbench(id: &str) -> &str {
    WORKBENCHES
        .iter()
        .find(|(old, _)| *old == id)
        .map_or(id, |(_, new)| new)
}

/// The command, tool or action id `id` is called now.
pub fn command(id: &str) -> Cow<'_, str> {
    COMMAND_PREFIXES
        .iter()
        .find_map(|(old, new)| id.strip_prefix(old).map(|rest| format!("{new}{rest}")))
        .map_or(Cow::Borrowed(id), Cow::Owned)
}

/// An id that starts with a workbench id and a `/` (a toolbar group), with
/// the workbench part called what it is now.
pub fn workbench_prefixed(id: &str) -> Cow<'_, str> {
    match id.split_once('/') {
        Some((bench, rest)) if workbench(bench) != bench => {
            Cow::Owned(format!("{}/{rest}", workbench(bench)))
        }
        _ => Cow::Borrowed(id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_ids_read_as_new_ones_and_others_stay() {
        assert_eq!(workbench("wb.part"), "wb.design");
        assert_eq!(workbench("wb.sketch"), "wb.sketch");
        assert_eq!(command("part.pad"), "design.pad");
        assert_eq!(command("design.pad"), "design.pad");
        assert_eq!(command("sketch.line"), "sketch.line");
        assert_eq!(command("partial.x"), "partial.x");
        assert_eq!(workbench_prefixed("wb.part/Make"), "wb.design/Make");
        assert_eq!(workbench_prefixed("std.file"), "std.file");
    }

    #[test]
    fn a_document_s_old_kind_reads_as_the_new_one() {
        let id: crate::WorkbenchId = serde_json::from_str("\"wb.part\"").unwrap();
        assert_eq!(id.as_str(), "wb.design");
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"wb.design\"");
    }
}
