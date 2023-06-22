use parking_lot::{MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};
use crate::generic::StoreView;

/// B-Tree map based on [Store].
pub type BTreeMap<'a, K, V> = crate::generic::BTreeMap<K, V, usize, &'a Store<crate::generic::Node<K, V, usize>>>;

/// B-Tree set based on [Store].
pub type BTreeSet<'a, T> = crate::generic::BTreeSet<T, usize, &'a Store<crate::generic::Node<T, (), usize>>>;

/// Shareable storage implemented via a `RwLock<slab::Slab<T>>`. Can be shared by multiple b-trees,
/// but *panics* if there are simultaneous mutable accesses, or simultaneously any accesses and an
/// insertion or removal. Unlike `shareable_slab::Store`, this can be shared between threads, but
/// it's significantly slower.
#[derive(Debug)]
pub struct Store<T>(RwLock<slab::Slab<T>>);

impl<T> Store<T> {
    pub fn new() -> Self {
        Self(RwLock::new(slab::Slab::new()))
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(RwLock::new(slab::Slab::with_capacity(capacity)))
    }
}

impl<T> Default for Store<T> {
    fn default() -> Self {
        Self::new()
    }
}

// RwLock doesn't implement Clone, so we have to do it manually
impl<T: Clone> Clone for Store<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self(RwLock::new((&*self.0.read()).clone()))
    }
}

impl<T> From<slab::Slab<T>> for Store<T> {
    #[inline]
    fn from(value: slab::Slab<T>) -> Self {
        Self(RwLock::new(value))
    }
}

impl<T> Into<slab::Slab<T>> for Store<T> {
    #[inline]
    fn into(self) -> slab::Slab<T> {
        self.0.into_inner()
    }
}

impl<'a, T> StoreView<T> for &'a Store<T> {
    type Index = usize;
    type Ref<'b, U: ?Sized + 'b> = MappedRwLockReadGuard<'b, U> where Self: 'b;

    #[inline]
    fn get(&self, index: Self::Index) -> Option<Self::Ref<'_, T>> {
        RwLockReadGuard::try_map(self.0.read(), |this| this.get(index)).ok()
    }
}

impl<'a, T> crate::generic::store::Store<T> for &'a Store<T> {
    type RefMut<'b, U: ?Sized + 'b> = MappedRwLockWriteGuard<'b, U> where Self: 'b;

    #[inline]
    fn insert(&mut self, value: T) -> Self::Index {
        self.0.write().insert(value)
    }

    #[inline]
    fn remove(&mut self, index: Self::Index) -> Option<T> {
        self.0.write().try_remove(index)
    }

    #[inline]
    fn get_mut(&mut self, index: Self::Index) -> Option<Self::RefMut<'_, T>> {
        RwLockWriteGuard::try_map(self.0.write(), |this| this.get_mut(index)).ok()
    }

    #[inline]
    fn clear_fast(&mut self) -> bool {
        // Not owned
        false
    }
}