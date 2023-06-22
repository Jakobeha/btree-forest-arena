mod index;
mod r#ref;

pub use index::*;
pub use r#ref::*;

/// Trait for a generic immutable view of a store. It must support immutable indexing
pub trait StoreView<T> {
    /// The type of index or key used to remember and retrieve elements from this store
    type Index: Index;
    /// Type of shared references to elements in this store (e.g. regular shared reference,
    /// [std::cell::Ref])
    type Ref<'a, U: ?Sized + 'a>: Ref<'a, U> where Self: 'a;

    /// Get the element inserted at the given index, if any
    fn get(&self, index: Self::Index) -> Option<Self::Ref<'_, T>>;
}

/// Trait for a generic store. It must support insertion, removal, and indexing
pub trait Store<T>: StoreView<T> {
    /// Type of mutable references to elements in this store (e.g. regular mutable reference,
    /// [std::cell::RefMut])
    type RefMut<'a, U: ?Sized + 'a>: RefMut<'a, U> where Self: 'a;

    /// Insert an element into the store, returning the index at which it was inserted.
    /// Subsequent calls to [Self::get] and [Self::get_mut] with the returned index will return a
    /// reference to the inserted element.
    fn insert(&mut self, value: T) -> Self::Index;
    /// Remove an element from the store, returning the element that was removed.
    ///
    /// Subsequent calls to [Self::get] and [Self::get_mut] with the given index must return `None`
    /// until a new element is inserted, then they can refer to any new element.
    fn remove(&mut self, index: Self::Index) -> Option<T>;
    /// Get a mutable reference to an element in the store, if any.
    ///
    /// [Self::get] and [Self::get_mut] must both return `Some` or both return `None`, and if both
    /// `Some`, they must return a same reference to the same element.
    fn get_mut(&mut self, index: Self::Index) -> Option<Self::RefMut<'_, T>>;
    /// If this store is completely owned (not a reference to a shared store), clear all elements
    /// and return `true`. Otherwise return `false`.
    fn clear_fast(&mut self) -> bool;
}

/// Trait for a [StoreView] whose `Ref` can be converted into a simple reference.
pub trait SlabViewWithSimpleRef<T>: StoreView<T> {
    /// Convert the store view's `Ref` into a simple shared reference
    ///
    /// The implementation is probably:
    ///
    /// ```text
    /// #[inline]
    /// fn convert_into_simple_ref<'a, U: ?Sized>(r#ref: Self::Ref<'a, U>) -> &'a U where Self: 'a {
    ///     r#ref
    /// }
    /// ```
    fn convert_into_simple_ref<'a, U: ?Sized>(r#ref: Self::Ref<'a, U>) -> &'a U where Self: 'a;
    /// Convert the store view's mapped `Ref` into a simple shared reference
    ///
    /// The implementation is probably:
    ///
    /// ```text
    /// #[inline]
    /// fn convert_mapped_into_simple_ref<'a, U: ?Sized>(
    ///     r#ref: <Self::Ref<'a, T> as Ref<'a, T>>::Mapped<U>
    /// ) -> &'a U where Self: 'a {
    ///     r#ref
    /// }
    /// ```
    fn convert_mapped_into_simple_ref<'a, U: ?Sized>(
        r#ref: <Self::Ref<'a, T> as Ref<'a, T>>::Mapped<U>
    ) -> &'a U where Self: 'a;
}

/// Trait for a [Store] whose `Ref` and `RefMut` can be converted into simple references.
pub trait SlabWithSimpleRefs<T>: Store<T> + SlabViewWithSimpleRef<T> {
    /// Convert the store view's `RefMut` into a simple mutable reference
    ///
    /// The implementation is probably:
    ///
    /// ```text
    /// #[inline]
    /// fn convert_into_simple_mut<'a, U: ?Sized>(r#ref: Self::RefMut<'a, U>) -> &'a U where Self: 'a {
    ///     r#ref
    /// }
    /// ```
    fn convert_into_simple_mut<'a, U: ?Sized>(r#ref: Self::RefMut<'a, U>) -> &'a mut U where Self: 'a;
    /// Convert the store view's mapped `Ref` into a simple shared reference
    ///
    /// The implementation is probably:
    ///
    /// ```text
    /// #[inline]
    /// fn convert_mapped_into_simple_mut<'a, U: ?Sized>(
    ///     r#ref: <Self::RefMut<'a, T> as RefMut<'a, T>>::Mapped<U>
    /// ) -> &'a mut U where Self: 'a {
    ///     r#ref
    /// }
    /// ```
    fn convert_mapped_into_simple_mut<'a, U: ?Sized>(
        r#ref: <Self::RefMut<'a, T> as RefMut<'a, T>>::Mapped<U>
    ) -> &'a mut U where Self: 'a;
}

/// Marker trait for a store which is completely owned by a collection (not a derivative of a shared
/// store).
///
/// This means that there are no elements from other stores, so its length will always be the
/// collection's length, we can completely iterate/clear it when iterate/clearing the collection,
/// and other relations/operations are simplified.
pub trait OwnedSlab<T>: Store<T> {
    /// Clear all elements. This trait's `clear_fast` should call this and return then `true`, i.e.
    /// it should be implemented exactly via the following
    ///
    /// ```text
    /// #[inline]
    /// fn clear_fast(&mut self) -> bool {
    ///     self.clear();
    ///     true
    /// }
    /// ```
    fn clear(&mut self);
}

#[cfg(any(doc, feature = "slab"))]
impl<T> StoreView<T> for slab::Slab<T> {
    type Index = usize;
    type Ref<'a, U: ?Sized + 'a> = &'a U where T: 'a;

    #[inline]
    fn get(&self, index: Self::Index) -> Option<Self::Ref<'_, T>> {
        slab::Slab::get(self, index)
    }
}

#[cfg(any(doc, feature = "slab"))]
impl<T> SlabViewWithSimpleRef<T> for slab::Slab<T> {
    #[inline]
    fn convert_into_simple_ref<'a, U: ?Sized>(r#ref: Self::Ref<'a, U>) -> &'a U where Self: 'a {
        r#ref
    }

    #[inline]
    fn convert_mapped_into_simple_ref<'a, U: ?Sized>(
        r#ref: <Self::Ref<'a, T> as Ref<'a, T>>::Mapped<U>
    ) -> &'a U where Self: 'a {
        r#ref
    }
}

#[cfg(any(doc, feature = "slab"))]
impl<T> Store<T> for slab::Slab<T> {
    type RefMut<'a, U: ?Sized + 'a> = &'a mut U where T: 'a;

    #[inline]
    fn insert(&mut self, value: T) -> Self::Index {
        slab::Slab::insert(self, value)
    }

    #[inline]
    fn remove(&mut self, index: Self::Index) -> Option<T> {
        slab::Slab::try_remove(self, index)
    }

    #[inline]
    fn get_mut(&mut self, index: Self::Index) -> Option<Self::RefMut<'_, T>> {
        slab::Slab::get_mut(self, index)
    }

    #[inline]
    fn clear_fast(&mut self) -> bool {
        // Is owned
        self.clear();
        true
    }
}

#[cfg(any(doc, feature = "slab"))]
impl<T> OwnedSlab<T> for slab::Slab<T> {
    #[inline]
    fn clear(&mut self) {
        slab::Slab::clear(self);
    }
}

#[cfg(any(doc, feature = "slab"))]
impl<T> SlabWithSimpleRefs<T> for slab::Slab<T> {
    #[inline]
    fn convert_into_simple_mut<'a, U: ?Sized>(r#ref: Self::RefMut<'a, U>) -> &'a mut U where Self: 'a {
        r#ref
    }

    #[inline]
    fn convert_mapped_into_simple_mut<'a, U: ?Sized>(
        r#ref: <Self::RefMut<'a, T> as RefMut<'a, T>>::Mapped<U>
    ) -> &'a mut U where Self: 'a {
        r#ref
    }
}