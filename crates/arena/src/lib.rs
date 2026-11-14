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

/// What a slot of an arena holds: a value, or nothing and a link in the
/// list of empty slots. One or the other, never both: an enum, as in
/// `generational-arena` and `thunderdome` (`slotmap` packs the same into a
/// union, at the cost of `unsafe` code).
#[derive(Clone, Debug, PartialEq)]
enum Content<V> {
    Occupied(V),
    /// Empty. `next_free`: the next empty slot to reuse after this one. The
    /// list of empty slots lives in the empty slots themselves, so
    /// removing a value never allocates.
    Vacant {
        next_free: Option<u32>,
    },
}

/// One place of an arena.
#[derive(Clone, Debug, PartialEq)]
struct Slot<V> {
    /// How many times this slot was emptied: part of the IDs to its values.
    generation: u32,
    content: Content<V>,
}

/// Values of type `V`, each identified by an ID of type `K` handed out
/// when it's inserted.
///
/// Memory: one `Vec` of slots, each holding a value (moved in and out,
/// never copied: lookups and iteration hand out references) or, when
/// empty, a link to the next empty slot; and its generation. Insertion,
/// lookup and removal take constant time. Removing never allocates;
/// inserting allocates only when the `Vec` is full and grows (doubling,
/// which moves the slots to the new buffer). Emptied slots are reused,
/// most recently emptied first, before the `Vec` grows.
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
///
/// Two arenas are equal (`PartialEq`) if their slots are: same values,
/// generations and free list, so the same history of insertions and
/// removals. Stricter than "the same values", as determinism tests want.
#[derive(Clone, Debug, PartialEq)]
pub struct Arena<K, V> {
    slots: Vec<Slot<V>>,
    /// The most recently emptied slot: the top of the stack of empty slots
    /// (`slotmap`'s `free_head`), the first to be reused. `None`: no empty
    /// slot, the next value goes at the end.
    free_head: Option<u32>,
    len: usize,
    /// The ID type, without storing one (`fn(K)`: no ownership of a `K`).
    key: PhantomData<fn(K) -> K>,
}

impl<K: Id, V> Default for Arena<K, V> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free_head: None,
            len: 0,
            key: PhantomData,
        }
    }
}

impl<K: Id, V> Arena<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Moves `value` in and returns its new ID.
    ///
    /// # Panics
    /// With 2³² slots in use (as a `Vec` panics past its capacity), or if
    /// the arena's own list of empty slots is broken (a bug in this code).
    pub fn insert(&mut self, value: V) -> K {
        let index = match self.free_head {
            Some(index) => {
                let slot = &mut self.slots[index as usize];
                // The free list only ever links empty slots: finding a
                // value here is a bug, and overwriting it would silently
                // lose it. Refuse, as `generational-arena` does.
                let Content::Vacant { next_free } = slot.content else {
                    panic!("corrupt free list: slot {index} is occupied");
                };
                self.free_head = next_free;
                slot.content = Content::Occupied(value);
                index
            }
            None => {
                let index = u32::try_from(self.slots.len()).expect("arena full: 2^32 slots");
                self.slots.push(Slot {
                    generation: 0,
                    content: Content::Occupied(value),
                });
                index
            }
        };
        self.len += 1;
        let generation = self.slots[index as usize].generation;
        K::from_raw(RawId { index, generation })
    }

    /// The value with this ID, if it's still here.
    pub fn get(&self, id: K) -> Option<&V> {
        let RawId { index, generation } = id.raw();
        match self.slots.get(index as usize)? {
            Slot {
                generation: current,
                content: Content::Occupied(value),
            } if *current == generation => Some(value),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: K) -> Option<&mut V> {
        let RawId { index, generation } = id.raw();
        match self.slots.get_mut(index as usize)? {
            Slot {
                generation: current,
                content: Content::Occupied(value),
            } if *current == generation => Some(value),
            _ => None,
        }
    }

    /// Moves the value with this ID out, if it's still here. The ID, and
    /// any copy of it, then finds nothing, even once its slot is reused.
    pub fn remove(&mut self, id: K) -> Option<V> {
        let RawId { index, generation } = id.raw();
        let slot = self.slots.get_mut(index as usize)?;
        let is_its_value =
            slot.generation == generation && matches!(slot.content, Content::Occupied(_));
        if !is_its_value {
            return None; // stale, or never there: nothing changes
        }
        // The next value in this slot gets a new generation: old IDs to it
        // no longer match. A slot whose generation can't grow any more is
        // retired (left empty, out of the free list, never reused), rather
        // than wrapping round to 0, where an ID from four billion reuses
        // ago would match again.
        let next_free = match slot.generation.checked_add(1) {
            Some(next) => {
                slot.generation = next;
                self.free_head.replace(index)
            }
            None => None,
        };
        self.len -= 1;
        match std::mem::replace(&mut slot.content, Content::Vacant { next_free }) {
            Content::Occupied(value) => Some(value),
            Content::Vacant { .. } => None, // checked above: can't happen
        }
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
            let Content::Occupied(value) = &slot.content else {
                return None;
            };
            let id = K::from_raw(RawId {
                index: index as u32,
                generation: slot.generation,
            });
            Some((id, value))
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(index, slot)| {
                let Content::Occupied(value) = &mut slot.content else {
                    return None;
                };
                let id = K::from_raw(RawId {
                    index: index as u32,
                    generation: slot.generation,
                });
                Some((id, value))
            })
    }
}

/// More data for IDs another [`Arena`] handed out, as an ECS stores a
/// component per entity (`slotmap`'s `SecondaryMap`): an entity's body
/// handle, its render proxy, where it's drawn. Indexed by the ID's slot,
/// so lookups are as fast as in the arena; the generation is checked, so
/// a stale ID finds nothing. Values are moved in and out, never copied.
#[derive(Clone, Debug, PartialEq)]
pub struct SecondaryMap<K, V> {
    /// By the IDs' slot index: the generation of the ID stored, and its
    /// value.
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
    /// an older ID in the same slot. Returns `false`, changing nothing, if
    /// `id` is itself older than the ID stored for its slot: a stale ID
    /// must not overwrite a newer entity's data. Grows (allocates) when
    /// `id`'s slot is beyond the map's end.
    pub fn insert(&mut self, id: K, value: V) -> bool {
        let RawId { index, generation } = id.raw();
        let index = index as usize;
        if self.slots.len() <= index {
            self.slots.resize_with(index + 1, || None);
        }
        let slot = &mut self.slots[index];
        match slot {
            // Generations only grow (see `Arena::remove`), so a smaller one
            // is older.
            Some((stored, _)) if *stored > generation => return false,
            Some(_) => {}
            None => self.len += 1,
        }
        *slot = Some((generation, value));
        true
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

    /// Moves out the value attached to `id`, if there is one.
    pub fn remove(&mut self, id: K) -> Option<V> {
        let RawId { index, generation } = id.raw();
        let slot = self.slots.get_mut(index as usize)?;
        match slot {
            Some((stored, _)) if *stored == generation => {
                self.len -= 1;
                slot.take().map(|(_, value)| value)
            }
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
    use alloc_count::{CountingAllocator, allocations_in};

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

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
    fn values_are_moved_in_and_out_never_copied() {
        // A `Vec`'s data lives on the heap: if the arena copied it, the
        // copy would be elsewhere. Same address all along: only the small
        // `Vec` header (pointer, length, capacity) moved.
        let mut things = Arena::<ThingId, Vec<u8>>::new();
        let data = vec![1, 2, 3];
        let address = data.as_ptr();
        let id = things.insert(data);
        assert_eq!(things.get(id).map(|v| v.as_ptr()), Some(address));
        assert_eq!(things.get_mut(id).map(|v| v.as_ptr()), Some(address));
        let out = things.remove(id).expect("it's there");
        assert_eq!(out.as_ptr(), address);

        let mut extra = SecondaryMap::<ThingId, Vec<u8>>::new();
        let id = things.insert(Vec::new());
        extra.insert(id, out);
        assert_eq!(extra.get(id).map(|v| v.as_ptr()), Some(address));
        assert_eq!(extra.remove(id).map(|v| v.as_ptr()), Some(address));
    }

    #[test]
    fn removing_never_allocates_and_reinserting_reuses_the_slots() {
        let mut things = Arena::<ThingId, u64>::new();
        let mut ids: Vec<ThingId> = (0..8).map(|n| things.insert(n)).collect();
        let count = allocations_in(|| {
            for round in 0..100 {
                for id in &mut ids {
                    things.remove(*id);
                    *id = things.insert(round);
                }
            }
        });
        assert_eq!(count, 0);
        assert_eq!(things.slots.len(), 8, "no slot added");
    }

    #[test]
    fn a_slot_out_of_generations_is_retired_not_reused() {
        // Pretend slot 0 was reused 2^32 - 1 times: one more would wrap to
        // generation 0, where IDs from long ago would match again.
        let mut things = Arena::<ThingId, &str>::new();
        things.insert("old");
        things.slots[0].generation = u32::MAX;
        let last = ThingId::from_raw(RawId {
            index: 0,
            generation: u32::MAX,
        });
        assert_eq!(things.remove(last), Some("old"));
        // Retired: the next value goes into a new slot, and nothing in
        // slot 0 is ever found again.
        let new = things.insert("new");
        assert_eq!(new.raw().index, 1);
        assert_eq!(things.get(last), None);
        let wrapped = ThingId::from_raw(RawId {
            index: 0,
            generation: 0,
        });
        assert_eq!(things.get(wrapped), None);
    }

    #[test]
    fn reinserting_reuses_the_last_emptied_slot_and_leaves_others_alone() {
        let mut things = Arena::<ThingId, &str>::new();
        let a = things.insert("a");
        let b = things.insert("b");
        let c = things.insert("c");
        things.remove(a);
        things.remove(b);
        // Most recently emptied first (a stack, as in `slotmap`): b's slot,
        // then a's.
        let d = things.insert("d");
        let e = things.insert("e");
        assert_eq!((d.raw().index, e.raw().index), (1, 0));
        // The live value was never touched, nor were the new ones.
        let values = (things.get(c), things.get(d), things.get(e));
        assert_eq!(values, (Some(&"c"), Some(&"d"), Some(&"e")));
        assert_eq!(things.len(), 3);
    }

    #[test]
    #[should_panic(expected = "corrupt free list: slot 0 is occupied")]
    fn a_free_list_pointing_at_a_value_is_refused_not_overwritten() {
        // Break the invariant by hand: the free list says slot 0 is empty,
        // but it holds a value. Inserting must not silently drop it.
        let mut things = Arena::<ThingId, &str>::new();
        things.insert("alive");
        things.free_head = Some(0);
        things.insert("intruder");
    }

    #[test]
    fn a_stale_id_cannot_overwrite_a_newer_ones_data() {
        let mut things = Arena::<ThingId, ()>::new();
        let old = things.insert(());
        things.remove(old);
        let new = things.insert(()); // same slot, newer generation
        let mut names = SecondaryMap::<ThingId, &str>::new();
        assert!(names.insert(new, "new"));
        assert!(!names.insert(old, "old"), "refused");
        assert_eq!((names.get(new), names.get(old)), (Some(&"new"), None));
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn a_secondary_value_can_be_removed() {
        let mut things = Arena::<ThingId, ()>::new();
        let (a, b) = (things.insert(()), things.insert(()));
        let mut names = SecondaryMap::<ThingId, &str>::new();
        names.insert(a, "a");
        names.insert(b, "b");
        assert_eq!(names.remove(a), Some("a"));
        assert_eq!(names.remove(a), None, "only once");
        assert_eq!((names.get(a), names.get(b)), (None, Some(&"b")));
        assert_eq!(names.len(), 1);
        // An ID beyond the map's end finds nothing to remove.
        let far = ThingId::from_raw(RawId {
            index: 99,
            generation: 0,
        });
        assert_eq!(names.remove(far), None);
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
