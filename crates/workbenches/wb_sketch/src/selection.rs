//! The sketch's selection: a set of element ids that also remembers the
//! order they were picked in, for the tools where it matters (the vertex of
//! a three-point angle, the centre of a symmetry about a point).

use std::collections::HashSet;
use std::ops::Deref;

use uuid::Uuid;

/// Reads as the set it holds; changes go through its own methods so the
/// order follows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    set: HashSet<Uuid>,
    order: Vec<Uuid>,
}

impl Deref for Selection {
    type Target = HashSet<Uuid>;

    fn deref(&self) -> &HashSet<Uuid> {
        &self.set
    }
}

impl Selection {
    /// Adds `id` after those already picked; `false` when it was there.
    pub fn insert(&mut self, id: Uuid) -> bool {
        let added = self.set.insert(id);
        if added {
            self.order.push(id);
        }
        added
    }

    /// `false` when it was not there.
    pub fn remove(&mut self, id: &Uuid) -> bool {
        let had = self.set.remove(id);
        if had {
            self.order.retain(|other| other != id);
        }
        had
    }

    pub fn clear(&mut self) {
        self.set.clear();
        self.order.clear();
    }

    pub fn extend(&mut self, ids: impl IntoIterator<Item = Uuid>) {
        for id in ids {
            self.insert(id);
        }
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&Uuid) -> bool) {
        self.order.retain(|id| keep(id));
        self.set = self.order.iter().copied().collect();
    }

    /// Empties the selection, handing back what it held in picked order.
    pub fn drain(&mut self) -> std::vec::IntoIter<Uuid> {
        self.set.clear();
        std::mem::take(&mut self.order).into_iter()
    }

    /// The ids in the order they were picked.
    pub fn in_order(&self) -> &[Uuid] {
        &self.order
    }
}

impl<'a> IntoIterator for &'a Selection {
    type Item = &'a Uuid;
    type IntoIter = std::collections::hash_set::Iter<'a, Uuid>;

    fn into_iter(self) -> Self::IntoIter {
        self.set.iter()
    }
}

impl FromIterator<Uuid> for Selection {
    fn from_iter<I: IntoIterator<Item = Uuid>>(ids: I) -> Self {
        let mut selection = Selection::default();
        selection.extend(ids);
        selection
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_order_follows_picks_removals_and_filters() {
        let ids: Vec<Uuid> = (0..4).map(|_| Uuid::new_v4()).collect();
        let mut s = Selection::default();
        for id in [ids[2], ids[0], ids[3]] {
            assert!(s.insert(id));
        }
        assert!(!s.insert(ids[0]), "already there");
        assert_eq!(s.in_order(), [ids[2], ids[0], ids[3]]);
        assert!(s.remove(&ids[0]));
        s.retain(|id| *id != ids[3]);
        assert_eq!(s.in_order(), [ids[2]]);
        assert!(s.contains(&ids[2]) && s.len() == 1);
    }
}
