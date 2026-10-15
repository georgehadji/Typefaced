//! The persistent glyph table (ADR-0017): `imbl::Vector<Option<Arc<Glyph>>>` indexed by
//! `GlyphId`, plus a name index.
//!
//! A deleted glyph leaves a `None` tombstone, so IDs stay dense and are never reused while a
//! document is open: sandboxes, undo snapshots and clients hold them. [`GlyphTable::compact`]
//! renumbers, and runs only on load. Cloning the table copies two tree roots.

use std::sync::Arc;

use imbl::{OrdMap, Vector};

use crate::{Glyph, GlyphId, ModelError, Name};

/// Glyphs by ID, with unique names. Invariant: `by_name` maps exactly the names of the live
/// slots to their IDs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GlyphTable {
    slots: Vector<Option<Arc<Glyph>>>,
    by_name: OrdMap<Name, GlyphId>,
}

impl GlyphTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of live glyphs.
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    /// The number of slots, tombstones included: the next new glyph gets this ID.
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    pub fn get(&self, id: GlyphId) -> Option<&Arc<Glyph>> {
        self.slots.get(id.0 as usize)?.as_ref()
    }

    pub fn id_of(&self, name: &str) -> Option<GlyphId> {
        self.by_name.get(name).copied()
    }

    /// Live glyphs in ID order.
    pub fn iter(&self) -> impl Iterator<Item = (GlyphId, &Arc<Glyph>)> {
        // Slot indexes fit in u32: `insert` refuses to grow past it.
        (0u32..)
            .zip(self.slots.iter())
            .filter_map(|(id, slot)| Some((GlyphId(id), slot.as_ref()?)))
    }

    /// Appends a glyph and returns its new ID. Fails if the name is taken.
    pub fn insert(&mut self, glyph: Arc<Glyph>) -> Result<GlyphId, ModelError> {
        if self.by_name.contains_key(&glyph.name) {
            return Err(ModelError::NameInUse(glyph.name.clone()));
        }
        let id = GlyphId(u32::try_from(self.slots.len()).map_err(|_| ModelError::TableFull)?);
        self.by_name.insert(glyph.name.clone(), id);
        self.slots.push_back(Some(glyph));
        Ok(id)
    }

    /// Replaces a live glyph (a new name renames it) and returns the old one.
    pub fn replace(&mut self, id: GlyphId, glyph: Arc<Glyph>) -> Result<Arc<Glyph>, ModelError> {
        let old = Arc::clone(self.live(id)?);
        if glyph.name != old.name {
            if self.by_name.contains_key(&glyph.name) {
                return Err(ModelError::NameInUse(glyph.name.clone()));
            }
            self.by_name.remove(&old.name);
            self.by_name.insert(glyph.name.clone(), id);
        }
        self.slots.set(id.0 as usize, Some(glyph));
        Ok(old)
    }

    /// Deletes a live glyph, leaving a tombstone, and returns it.
    pub fn remove(&mut self, id: GlyphId) -> Result<Arc<Glyph>, ModelError> {
        let old = Arc::clone(self.live(id)?);
        self.by_name.remove(&old.name);
        self.slots.set(id.0 as usize, None);
        Ok(old)
    }

    /// The same glyphs, in the same order, without tombstones. **Load only:** it changes IDs.
    pub fn compact(&self) -> Self {
        let mut table = Self::new();
        for (_, glyph) in self.iter() {
            // Names are unique and there are fewer glyphs than slots: cannot fail.
            let _ = table.insert(Arc::clone(glyph));
        }
        table
    }

    fn live(&self, id: GlyphId) -> Result<&Arc<Glyph>, ModelError> {
        self.get(id).ok_or(ModelError::GlyphNotFound(id))
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn glyph(name: &str) -> Arc<Glyph> {
        Arc::new(Glyph::new(Name::new(name).unwrap()))
    }

    fn names(table: &GlyphTable) -> Vec<(u32, String)> {
        table
            .iter()
            .map(|(id, g)| (id.0, g.name.to_string()))
            .collect()
    }

    #[test]
    fn insert_assigns_dense_ids_and_refuses_taken_names() {
        let mut table = GlyphTable::new();
        assert!(table.is_empty());
        assert_eq!(table.insert(glyph("A")), Ok(GlyphId(0)));
        assert_eq!(table.insert(glyph("B")), Ok(GlyphId(1)));
        assert_eq!(
            table.insert(glyph("A")),
            Err(ModelError::NameInUse(Name::new("A").unwrap()))
        );
        assert_eq!((table.len(), table.slot_count()), (2, 2));
        assert_eq!(table.id_of("B"), Some(GlyphId(1)));
        assert_eq!(table.id_of("C"), None);
        assert_eq!(table.get(GlyphId(0)).unwrap().name.as_str(), "A");
        assert_eq!(table.get(GlyphId(9)), None);
    }

    #[test]
    fn remove_leaves_a_tombstone_and_frees_the_name() {
        let mut table = GlyphTable::new();
        for name in ["A", "B", "C"] {
            table.insert(glyph(name)).unwrap();
        }
        let snapshot = table.clone();
        assert_eq!(table.remove(GlyphId(1)).unwrap().name.as_str(), "B");
        assert_eq!(
            table.remove(GlyphId(1)),
            Err(ModelError::GlyphNotFound(GlyphId(1)))
        );
        assert_eq!(
            table.remove(GlyphId(7)),
            Err(ModelError::GlyphNotFound(GlyphId(7)))
        );
        assert_eq!((table.len(), table.slot_count()), (2, 3));
        assert_eq!(table.get(GlyphId(1)), None);
        assert_eq!(table.id_of("B"), None);
        assert_eq!(names(&table), [(0, "A".into()), (2, "C".into())]);
        // A new glyph never reuses the tombstone's ID; the freed name is available again.
        assert_eq!(table.insert(glyph("B")), Ok(GlyphId(3)));
        // The snapshot taken before is untouched.
        assert_eq!(snapshot.len(), 3);
        assert_eq!(snapshot.id_of("B"), Some(GlyphId(1)));
    }

    #[test]
    fn replace_keeps_the_id_and_can_rename() {
        let mut table = GlyphTable::new();
        table.insert(glyph("A")).unwrap();
        table.insert(glyph("B")).unwrap();
        assert_eq!(
            table.replace(GlyphId(0), glyph("A")).unwrap().name.as_str(),
            "A"
        );
        assert_eq!(
            table.replace(GlyphId(0), glyph("B")),
            Err(ModelError::NameInUse(Name::new("B").unwrap()))
        );
        assert_eq!(
            table.replace(GlyphId(5), glyph("Z")),
            Err(ModelError::GlyphNotFound(GlyphId(5)))
        );
        table.replace(GlyphId(0), glyph("A.alt")).unwrap();
        assert_eq!(table.id_of("A"), None);
        assert_eq!(table.id_of("A.alt"), Some(GlyphId(0)));
        table.remove(GlyphId(1)).unwrap();
        assert_eq!(
            table.replace(GlyphId(1), glyph("B")),
            Err(ModelError::GlyphNotFound(GlyphId(1)))
        );
    }

    #[test]
    fn compact_renumbers_in_order() {
        let mut table = GlyphTable::new();
        for name in ["A", "B", "C", "D"] {
            table.insert(glyph(name)).unwrap();
        }
        table.remove(GlyphId(0)).unwrap();
        table.remove(GlyphId(2)).unwrap();
        let compact = table.compact();
        assert_eq!(names(&compact), [(0, "B".into()), (1, "D".into())]);
        assert_eq!(compact.slot_count(), 2);
        assert_eq!(compact.id_of("D"), Some(GlyphId(1)));
        assert_eq!(compact.compact(), compact);
    }

    /// One operation of the model-based property test.
    #[derive(Clone, Debug)]
    enum Op {
        Insert(u8),
        Remove(u8),
        Rename(u8, u8),
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            any::<u8>().prop_map(Op::Insert),
            any::<u8>().prop_map(Op::Remove),
            (any::<u8>(), any::<u8>()).prop_map(|(a, b)| Op::Rename(a, b)),
        ]
    }

    /// The name index agrees with the slots, and names are unique.
    fn assert_consistent(table: &GlyphTable) {
        let live: Vec<_> = table.iter().collect();
        assert_eq!(live.len(), table.len());
        for (id, glyph) in &live {
            assert_eq!(table.id_of(&glyph.name), Some(*id));
        }
        assert_eq!(table.by_name.len(), table.len());
    }

    proptest! {
        /// The table behaves like a plain `Vec<Option<String>>` under any operation sequence.
        #[test]
        fn behaves_like_a_vector_of_names(ops in prop::collection::vec(op(), 0..60)) {
            let mut table = GlyphTable::new();
            let mut model: Vec<Option<String>> = Vec::new();
            let taken = |model: &Vec<Option<String>>, n: &str| {
                model.iter().flatten().any(|m| m == n)
            };
            for op in ops {
                match op {
                    Op::Insert(n) => {
                        let name = format!("g{}", n % 16);
                        let result = table.insert(glyph(&name));
                        if taken(&model, &name) {
                            prop_assert!(result.is_err());
                        } else {
                            prop_assert_eq!(result, Ok(GlyphId(model.len() as u32)));
                            model.push(Some(name));
                        }
                    }
                    Op::Remove(i) => {
                        let id = GlyphId(u32::from(i) % (model.len() as u32 + 1));
                        let result = table.remove(id);
                        match model.get_mut(id.0 as usize) {
                            Some(slot @ Some(_)) => {
                                prop_assert!(result.is_ok());
                                *slot = None;
                            }
                            _ => prop_assert!(result.is_err()),
                        }
                    }
                    Op::Rename(i, n) => {
                        let id = GlyphId(u32::from(i) % (model.len() as u32 + 1));
                        let name = format!("g{}", n % 16);
                        let result = table.replace(id, glyph(&name));
                        let own = model.get(id.0 as usize).cloned().flatten();
                        match own {
                            Some(old) if old == name || !taken(&model, &name) => {
                                prop_assert!(result.is_ok());
                                model[id.0 as usize] = Some(name);
                            }
                            _ => prop_assert!(result.is_err()),
                        }
                    }
                }
                assert_consistent(&table);
                prop_assert_eq!(table.slot_count(), model.len());
            }
            let expected: Vec<(u32, String)> = model
                .iter()
                .enumerate()
                .filter_map(|(i, n)| n.clone().map(|n| (i as u32, n)))
                .collect();
            prop_assert_eq!(names(&table), expected);
            let compact = table.compact();
            assert_consistent(&compact);
            let compact_names: Vec<String> = names(&compact).into_iter().map(|(_, n)| n).collect();
            let model_names: Vec<String> = model.into_iter().flatten().collect();
            prop_assert_eq!(compact_names, model_names);
        }
    }
}
