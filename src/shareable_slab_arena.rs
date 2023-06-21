use std::cell::Cell;
use std::fmt::Debug;
use std::mem::replace;
use std::ptr::{NonNull, null};

use crate::generic::{SlabView, SlabViewWithSimpleRef, SlabWithSimpleRefs};
use crate::generic::slab::{Ref, RefMut, Slab};
use crate::shareable_slab_arena::rustc_arena::TypedArena;

/// Original code from [rustc_arena](https://doc.rust-lang.org/stable/nightly-rustc/rustc_arena/index.html).
/// There are some modifications, including removing the [DroplessArena] because we don't use,
/// converting unstable features to stable equivalents, and exposing more of the API when necessary
/// for `shareable_slab_arena`.
///
/// The arena, a fast but limited type of allocator.
///
/// Arenas are a type of allocator that destroy the objects within, all at
/// once, once the arena itself is destroyed. They do not support deallocation
/// of individual objects while the arena itself is still alive. The benefit
/// of an arena is very fast allocation; just a pointer bump.
mod rustc_arena;

/// B-Tree map based on `ShareableArena`.
pub type BTreeMap<'a, K, V> = crate::generic::BTreeMap<K, V, Index, &'a ShareableSlabArena<crate::generic::Node<K, V, Index>>>;

/// B-Tree set based on `ShareableArena`.
pub type BTreeSet<'a, T> = crate::generic::BTreeSet<T, Index, &'a ShareableSlabArena<crate::generic::Node<T, (), Index>>>;

/// An arena which can be shared by multiple b-trees, and multiple b-trees can simultaneously access,
/// mutate, delete, and insert entries, but can't delete or reuse old entries without a mutable
/// reference (while being used by b-trees).
#[derive(Debug)]
pub struct ShareableSlabArena<T> {
    /// Arena
    arena: TypedArena<Entry<T>>,
    /// Pointer to next free entry we've already allocated, or `None` if we need to allocate more.
    next_free: Cell<Option<NonNull<Entry<T>>>>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Index(*const ());

#[derive(Debug)]
enum Entry<T> {
    /// A value is present
    Occupied { value: T },
    /// A value is not present
    Vacant { next_free: Option<NonNull<Entry<T>>> },
}

impl<T> ShareableSlabArena<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            arena: TypedArena::new(),
            next_free: Cell::new(None),
        }
    }

    /// Clear the arena without deallocating
    #[inline]
    pub fn clear(&mut self) {
        self.arena.clear();
        self.next_free = Cell::new(None);
    }
}

impl<T> Default for ShareableSlabArena<T> {
    fn default() -> Self {
        ShareableSlabArena::new()
    }
}

impl<'a, T> SlabView<T> for &'a ShareableSlabArena<T> {
    type Index = Index;
    type Ref<'b, U: ?Sized + 'b> = &'b U where Self: 'b;

    fn get(&self, index: Self::Index) -> Option<Self::Ref<'_, T>> {
        let Some(index): Option<NonNull<Entry<T>>> = index.into() else {
            return None
        };

        // SAFETY: From b-tree invariants, we there is no active mutable reference at index
        match unsafe { index.as_ref() } {
            Entry::Occupied { value } => Some(value),
            Entry::Vacant { .. } => None,
        }
    }
}

impl<'a, T> SlabViewWithSimpleRef<T> for &'a ShareableSlabArena<T> {
    #[inline]
    fn convert_into_simple_ref<'b, U: ?Sized>(r#ref: Self::Ref<'b, U>) -> &'b U where Self: 'b {
        r#ref
    }

    //noinspection DuplicatedCode
    #[inline]
    fn convert_mapped_into_simple_ref<'b, U: ?Sized>(
        r#ref: <Self::Ref<'b, T> as Ref<'b, T>>::Mapped<U>
    ) -> &'b U where Self: 'b {
        r#ref
    }
}

impl<'a, T> Slab<T> for &'a ShareableSlabArena<T> {
    type RefMut<'b, U: ?Sized + 'b> = &'b mut U where Self: 'b;

    fn insert(&mut self, value: T) -> Self::Index {
        Index::from(Some(match self.next_free.get() {
            // This is so trivially non-null that the compiler will optimize away the unwrap, so
            // it's arguably better than doing new_unchecked (defensive coding)
            None => NonNull::new(self.arena.alloc(
                Entry::Occupied { value }
            ) as *const Entry<T> as *mut Entry<T>).unwrap(),
            Some(mut next_free) => {
                // SAFETY: This entry isn't being used by any b-trees, since its index was removed
                // from the b-tree which previously "owned" it
                let next_free = unsafe { next_free.as_mut() };
                match next_free {
                    Entry::Vacant { next_free: next_next_free } => {
                        self.next_free.set(*next_next_free);
                    }
                    Entry::Occupied { .. } => unreachable!("next_free should always be Vacant")
                }
                *next_free = Entry::Occupied { value };
                NonNull::new(next_free as *mut Entry<T>).unwrap()
            }
        }))
    }

    fn remove(&mut self, index: Self::Index) -> Option<T> {
        let Some(mut index): Option<NonNull<Entry<T>>> = index.into() else {
            return None
        };

        // SAFETY: From b-tree invariants, we there is no other active reference at index
        let entry = unsafe { index.as_mut() };
        // First we replace with what we would leave entry to be if it's occupied
        match replace(entry, Entry::Vacant { next_free: self.next_free.get() }) {
            // If the entry is occupied, complete adding to the free-list and return the value
            Entry::Occupied { value } => {
                self.next_free.set(Some(index));
                Some(value)
            }
            // If the entry is vacant, set it back and return `None`
            Entry::Vacant { next_free } => {
                *entry = Entry::Vacant { next_free };
                None
            }
        }
    }

    #[inline]
    fn get_mut(&mut self, index: Self::Index) -> Option<Self::RefMut<'_, T>> {
        let Some(mut index): Option<NonNull<Entry<T>>> = index.into() else {
            return None
        };

        // SAFETY: From b-tree invariants, we there is no other active reference at index
        match unsafe { index.as_mut() } {
            Entry::Occupied { value } => Some(value),
            Entry::Vacant { .. } => None,
        }
    }

    #[inline]
    fn clear_fast(&mut self) -> bool {
        // Not owned
        false
    }
}

impl<'a, T> SlabWithSimpleRefs<T> for &'a ShareableSlabArena<T> {
    #[inline]
    fn convert_into_simple_mut<'b, U: ?Sized>(r#ref: Self::RefMut<'b, U>) -> &'b mut U where Self: 'b {
        r#ref
    }

    //noinspection DuplicatedCode
    #[inline]
    fn convert_mapped_into_simple_mut<'b, U: ?Sized>(
        r#ref: <Self::RefMut<'b, T> as RefMut<'b, T>>::Mapped<U>
    ) -> &'b mut U where Self: 'b {
        r#ref
    }
}

impl<T> From<Option<NonNull<T>>> for Index {
    #[inline]
    fn from(value: Option<NonNull<T>>) -> Self {
        Self(match value {
            None => null(),
            Some(value) => value.as_ptr().cast_const().cast::<()>(),
        })
    }
}

impl<T> Into<Option<NonNull<T>>> for Index {
    #[inline]
    fn into(self) -> Option<NonNull<T>> {
        NonNull::new(self.0.cast::<T>().cast_mut())
    }
}

impl crate::generic::slab::Index for Index {
    fn nowhere() -> Self {
        Index(null())
    }

    fn is_nowhere(&self) -> bool {
        self.0.is_null()
    }
}