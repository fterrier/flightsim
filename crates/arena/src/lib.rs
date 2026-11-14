//! Generational arenas: storage that hands out IDs.
//!
//! Every engine identifies things this way: Bevy's `Entity`, Rapier's
//! `RigidBodyHandle`, Jolt's `BodyID`, Box2D's `b2BodyId`, the `slotmap`
//! crate's keys. An ID is:
//! - **generated** by the storage when something is put in, never chosen
//!   by whoever uses it;
//! - **opaque**: it can be copied and compared, not computed with (no
//!   `id + 1`), and its parts are private;
//! - **generational**: an index into the storage, plus a *generation*
//!   counting how many times that slot has been reused. Removing a value
//!   bumps its slot's generation, so an old ID to it no longer matches
//!   anything, even after the slot is reused: a lookup says "not found"
//!   instead of silently returning the newcomer;
//! - **typed**: one ID type per kind of thing (`id_type!`), so an entity
//!   ID can't be passed where a texture ID is expected.
//!
//! [`Arena`] stores values and hands out their IDs; [`SecondaryMap`]
//! attaches more data to IDs another arena handed out (as an ECS stores a
//! component per entity).

use std::fmt;
use std::marker::PhantomData;

/// What every ID is made of: a slot index and the slot's generation. Its
/// fields are private: only an [`Arena`] makes them.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RawId {
    index: u32,
    generation: u32,
}

impl fmt::Debug for RawId {
    /// `3v1`: slot 3, generation 1 (as Bevy prints entities).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}v{}", self.index, self.generation)
    }
}

/// An ID type: a named wrapper round a [`RawId`]. Implemented by
/// [`id_type!`]; nothing else should need to.
pub trait Id: Copy + Eq + std::hash::Hash + fmt::Debug {
    fn from_raw(raw: RawId) -> Self;
    fn raw(self) -> RawId;
}

/// Declares an ID type, for the values of one kind of [`Arena`]:
///
/// ```
/// arena::id_type! {
///     /// Identifies a body of the physics world.
///     pub struct BodyId;
/// }
/// ```
///
/// A macro, as `slotmap`'s `new_key_type!`: every ID type is the same few
/// lines, and must stay the same.
#[macro_export]
macro_rules! id_type {
    ($(#[$doc:meta])* $vis:vis struct $name:ident;) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        $vis struct $name($crate::RawId);

        impl $crate::Id for $name {
            fn from_raw(raw: $crate::RawId) -> Self {
                Self(raw)
            }
            fn raw(self) -> $crate::RawId {
                self.0
            }
        }

        impl ::std::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}({:?})", stringify!($name), self.0)
            }
        }
    };
}

/// One place of an arena: its current generation, and the value if any.
#[derive(Clone, Debug, PartialEq)]
struct Slot<V> {
    generation: u32,
    value: Option<V>,
}

/// Values of type `V`, each identified by an ID of type `K` handed out
/// when it's inserted. Stored contiguously in a `Vec`; insertion, lookup
/// and removal take constant time; removed slots are reused.
///
/// An arena only takes its own kind of ID: this doesn't compile.
///
/// ```compile_fail,E0308
/// arena::id_type! { struct AppleId; }
/// arena::id_type! { struct PearId; }
/// let mut apples = arena::Arena::<AppleId, &str>::new();
/// let mut pears = arena::Arena::<PearId, &str>::new();
/// let pear = pears.insert("pear");
/// apples.get(pear); // error: expected `AppleId`, found `PearId`
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Arena<K, V> {
    slots: Vec<Slot<V>>,
    /// Indices of empty slots, reused before the `Vec` grows.
    free: Vec<u32>,
    len: usize,
    /// The ID type, without storing one (`fn(K)`: no ownership of a `K`).
    key: PhantomData<fn(K) -> K>,
}

impl<K: Id, V> Default for Arena<K, V> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            len: 0,
            key: PhantomData,
        }
    }
}

impl<K: Id, V> Arena<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores `value` and returns its new ID.
    pub fn insert(&mut self, value: V) -> K {
        self.len += 1;
        let index = match self.free.pop() {
            Some(index) => {
                self.slots[index as usize].value = Some(value);
                index
            }
            None => {
                self.slots.push(Slot {
                    generation: 0,
                    value: Some(value),
                });
                (self.slots.len() - 1) as u32
            }
        };
        let generation = self.slots[index as usize].generation;
        K::from_raw(RawId { index, generation })
    }

    /// The value with this ID, if it's still here.
    pub fn get(&self, id: K) -> Option<&V> {
        let RawId { index, generation } = id.raw();
        let slot = self.slots.get(index as usize)?;
        if slot.generation == generation {
            slot.value.as_ref()
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, id: K) -> Option<&mut V> {
        let RawId { index, generation } = id.raw();
        let slot = self.slots.get_mut(index as usize)?;
        if slot.generation == generation {
            slot.value.as_mut()
        } else {
            None
        }
    }

    /// Takes the value with this ID out, if it's still here. The ID, and
    /// any copy of it, then finds nothing, even once its slot is reused.
    pub fn remove(&mut self, id: K) -> Option<V> {
        let RawId { index, generation } = id.raw();
        let slot = self.slots.get_mut(index as usize)?;
        if slot.generation != generation {
            return None;
        }
        let value = slot.value.take()?;
        // The next value in this slot gets a new generation: old IDs to it
        // no longer match.
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(index);
        self.len -= 1;
        Some(value)
    }

    pub fn contains(&self, id: K) -> bool {
        self.get(id).is_some()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Every value with its ID, in slot order: always the same order for
    /// the same insertions and removals (determinism).
    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            let id = K::from_raw(RawId {
                index: index as u32,
                generation: slot.generation,
            });
            slot.value.as_ref().map(|value| (id, value))
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(index, slot)| {
                let id = K::from_raw(RawId {
                    index: index as u32,
                    generation: slot.generation,
                });
                slot.value.as_mut().map(|value| (id, value))
            })
    }
}

/// More data for IDs another [`Arena`] handed out, as an ECS stores a
/// component per entity (`slotmap`'s `SecondaryMap`): an entity's body
/// handle, its render proxy, where it's drawn. Indexed by the ID's slot,
/// so lookups are as fast as in the arena; the generation is checked, so
/// a stale ID finds nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct SecondaryMap<K, V> {
    slots: Vec<Option<(u32, V)>>,
    len: usize,
    key: PhantomData<fn(K) -> K>,
}

impl<K: Id, V> Default for SecondaryMap<K, V> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            len: 0,
            key: PhantomData,
        }
    }
}

impl<K: Id, V> SecondaryMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Attaches `value` to `id`, replacing what was there for it, or for
    /// an older ID in the same slot.
    pub fn insert(&mut self, id: K, value: V) {
        let RawId { index, generation } = id.raw();
        let index = index as usize;
        if self.slots.len() <= index {
            self.slots.resize_with(index + 1, || None);
        }
        if self.slots[index].is_none() {
            self.len += 1;
        }
        self.slots[index] = Some((generation, value));
    }

    pub fn get(&self, id: K) -> Option<&V> {
        let RawId { index, generation } = id.raw();
        match self.slots.get(index as usize)? {
            Some((stored, value)) if *stored == generation => Some(value),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: K) -> Option<&mut V> {
        let RawId { index, generation } = id.raw();
        match self.slots.get_mut(index as usize)? {
            Some((stored, value)) if *stored == generation => Some(value),
            _ => None,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Every ID with its value, in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.as_ref().map(|(generation, value)| {
                let id = K::from_raw(RawId {
                    index: index as u32,
                    generation: *generation,
                });
                (id, value)
            })
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(index, slot)| {
                slot.as_mut().map(|(generation, value)| {
                    let id = K::from_raw(RawId {
                        index: index as u32,
                        generation: *generation,
                    });
                    (id, value)
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    id_type! {
        /// IDs for the tests.
        struct ThingId;
    }

    #[test]
    fn inserted_values_are_found_by_their_ids() {
        let mut things = Arena::<ThingId, &str>::new();
        let a = things.insert("a");
        let b = things.insert("b");
        assert_ne!(a, b);
        assert_eq!((things.get(a), things.get(b)), (Some(&"a"), Some(&"b")));
        assert_eq!(things.len(), 2);
        *things.get_mut(a).expect("a is there") = "A";
        assert_eq!(things.get(a), Some(&"A"));
    }

    #[test]
    fn a_removed_value_is_gone_for_good_even_once_its_slot_is_reused() {
        let mut things = Arena::<ThingId, &str>::new();
        let old = things.insert("old");
        assert_eq!(things.remove(old), Some("old"));
        assert!(!things.contains(old));
        assert_eq!(things.remove(old), None, "only once");

        // The slot is reused...
        let new = things.insert("new");
        assert_eq!(new.raw().index, old.raw().index);
        // ...under a new generation: the old ID doesn't see the newcomer.
        assert_ne!(new, old);
        assert_eq!(things.get(old), None);
        assert_eq!(things.get_mut(old), None);
        assert_eq!(things.get(new), Some(&"new"));
        assert_eq!(things.len(), 1);
    }

    #[test]
    fn an_id_from_a_bigger_arena_finds_nothing() {
        let mut big = Arena::<ThingId, u8>::new();
        let small = Arena::<ThingId, u8>::new();
        let id = big.insert(1);
        assert_eq!(small.get(id), None);
        assert!(small.is_empty());
    }

    #[test]
    fn iteration_follows_slot_order_and_skips_removed_values() {
        let mut things = Arena::<ThingId, u8>::new();
        let ids: Vec<ThingId> = (0..4).map(|n| things.insert(n)).collect();
        things.remove(ids[1]);
        let seen: Vec<(ThingId, u8)> = things.iter().map(|(id, v)| (id, *v)).collect();
        assert_eq!(seen, [(ids[0], 0), (ids[2], 2), (ids[3], 3)]);
        for (_, value) in things.iter_mut() {
            *value *= 10;
        }
        assert_eq!(things.get(ids[3]), Some(&30));
    }

    #[test]
    fn ids_print_their_kind_slot_and_generation() {
        let mut things = Arena::<ThingId, ()>::new();
        let first = things.insert(());
        things.remove(first);
        let second = things.insert(());
        assert_eq!(format!("{first:?} {second:?}"), "ThingId(0v0) ThingId(0v1)");
    }

    #[test]
    fn a_secondary_map_attaches_data_to_another_arenas_ids() {
        let mut things = Arena::<ThingId, &str>::new();
        let (a, b) = (things.insert("a"), things.insert("b"));
        let mut sizes = SecondaryMap::<ThingId, usize>::new();
        assert!(sizes.is_empty());
        sizes.insert(b, 2);
        sizes.insert(a, 1);
        assert_eq!((sizes.get(a), sizes.get(b)), (Some(&1), Some(&2)));
        assert_eq!(sizes.len(), 2);
        *sizes.get_mut(a).expect("a has a size") += 10;
        let all: Vec<(ThingId, usize)> = sizes.iter().map(|(id, s)| (id, *s)).collect();
        assert_eq!(all, [(a, 11), (b, 2)]);
        for (_, size) in sizes.iter_mut() {
            *size = 0;
        }
        assert_eq!(sizes.get(b), Some(&0));
    }

    #[test]
    fn a_secondary_map_ignores_stale_ids() {
        let mut things = Arena::<ThingId, ()>::new();
        let old = things.insert(());
        let mut data = SecondaryMap::<ThingId, &str>::new();
        data.insert(old, "old");
        things.remove(old);
        let new = things.insert(()); // same slot, new generation
        assert_eq!(data.get(new), None);
        assert_eq!(data.get_mut(new), None);
        data.insert(new, "new"); // replaces the stale entry
        assert_eq!((data.get(old), data.get(new)), (None, Some(&"new")));
        assert_eq!(data.len(), 1);
    }
}
