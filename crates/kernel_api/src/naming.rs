//! Names for the faces and edges of a built solid that survive a rebuild.
//!
//! A face is named by how it came to be: the feature that made it and the
//! sketch element it was swept from, or the feature's start or end. A
//! face a later feature splits, trims or merges keeps the names of the
//! faces it came from; an edge is named by the two faces it runs between.
//! References keep these names beside their points, so a rebuild finds a
//! face again after dimensions change or features go in earlier, where a
//! point alone would miss it.
//!
//! A name is a 64-bit hash of its parts, the same on every machine and in
//! every version: it is saved in documents.

/// A face's name; zero is no name.
pub type TopoName = u64;

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0100_0000_01b3;

fn fnv(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// The name of `bytes`: never zero.
pub fn name_of(bytes: &[u8]) -> TopoName {
    match fnv(OFFSET, bytes) {
        0 => 1,
        name => name,
    }
}

/// The name of a thing with a UUID's bytes (a feature, a sketch element).
pub fn name_of_id(id: &[u8; 16]) -> TopoName {
    name_of(id)
}

/// `parent`'s child called `part`: a feature's face swept from one of its
/// sketch's elements, or its start.
pub fn child(parent: TopoName, part: &[u8]) -> TopoName {
    match fnv(fnv(OFFSET, &parent.to_le_bytes()), part) {
        0 => 1,
        name => name,
    }
}

/// Reads a name written as a number or as a string of its digits: a
/// script's numbers are doubles, which cannot hold every name, so names
/// reach scripts as strings and come back as them.
pub fn name_from_number_or_text<'de, D>(deserializer: D) -> Result<TopoName, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Written {
        Number(u64),
        Text(String),
    }
    match <Written as serde::Deserialize>::deserialize(deserializer)? {
        Written::Number(n) => Ok(n),
        Written::Text(t) => t.trim().parse().map_err(serde::de::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_fixed_and_never_zero() {
        // Saved in documents: the same bytes always make the same name.
        assert_eq!(name_of(b"pad"), 0x77ca60195676adaa);
        assert_ne!(name_of(b"pad"), name_of(b"pocket"));
        assert_ne!(
            child(name_of(b"pad"), b"start"),
            child(name_of(b"pad"), b"end")
        );
        assert_ne!(name_of(&[]), 0);
    }
}
