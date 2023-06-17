use std::cell::{Ref, RefCell, RefMut};

use crate::generic::SlabView;
use crate::generic::slab::Slab;

/// A slab which can be shared by multiple b-trees, but *panics* if there are simultaneous mutable
/// accesses, or simultaneously any accesses and an insertion or removal.
pub struct ShareableSlab<T>(RefCell<slab::Slab<T>>);

impl<T> ShareableSlab<T> {
    pub fn new() -> Self {
        Self(RefCell::new(slab::Slab::new()))
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(RefCell::new(slab::Slab::with_capacity(capacity)))
    }
}

impl<T> From<slab::Slab<T>> for ShareableSlab<T> {
    fn from(value: slab::Slab<T>) -> Self {
        Self(RefCell::new(value))
    }
}

impl<T> Into<slab::Slab<T>> for ShareableSlab<T> {
    fn into(self) -> slab::Slab<T> {
        self.0.into_inner()
    }
}

impl<'a, T> SlabView<T> for &'a ShareableSlab<T> {
    type Index = usize;
    type Ref<'b, U: ?Sized + 'b> = Ref<'b, U> where Self: 'b;

    fn get(&self, index: Self::Index) -> Option<Self::Ref<'_, T>> {
        Ref::filter_map(self.0.borrow(), |this| this.get(index)).ok()
    }
}

impl<'a, T> Slab<T> for &'a ShareableSlab<T> {
    type RefMut<'b, U: ?Sized + 'b> = RefMut<'b, U> where Self: 'b;

    fn insert(&mut self, value: T) -> Self::Index {
        self.0.borrow_mut().insert(value)
    }

    fn remove(&mut self, index: Self::Index) -> Option<T> {
        self.0.borrow_mut().try_remove(index)
    }

    fn get_mut(&mut self, index: Self::Index) -> Option<Self::RefMut<'_, T>> {
        RefMut::filter_map(self.0.borrow_mut(), |this| this.get_mut(index)).ok()
    }

    fn clear_fast(&mut self) -> bool {
        // Not owned
        false
    }
}