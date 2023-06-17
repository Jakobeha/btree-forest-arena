mod index;
mod r#ref;

pub use index::*;
pub use r#ref::*;

/// Trait for a generic immutable view of a slab. It must support immutable indexing
pub trait SlabView<T> {
    /// The type of index or key used to remember and retrieve elements from this slab
    type Index: Index;
    /// Type of shared references to elements in this slab (e.g. regular shared reference,
    /// [std::cell::Ref])
    type Ref<'a, U: ?Sized + 'a>: Ref<'a, U> where Self: 'a;

    /// Get the element inserted at the given index, if any
    fn get(&self, index: Self::Index) -> Option<Self::Ref<'_, T>>;
}

/// Trait for a generic slab. It must support insertion, removal, and indexing
pub trait Slab<T>: SlabView<T> {
    /// Type of mutable references to elements in this slab (e.g. regular mutable reference,
    /// [std::cell::RefMut])
    type RefMut<'a, U: ?Sized + 'a>: RefMut<'a, U> where Self: 'a;

    /// Insert an element into the slab, returning the index at which it was inserted.
    /// Subsequent calls to [Self::get] and [Self::get_mut] with the returned index must return a
    /// reference to the inserted element.
    fn insert(&mut self, value: T) -> Self::Index;
    /// Remove an element from the slab, returning the element that was removed.
    /// Subsequent calls to [Self::get] and [Self::get_mut] with the given index must return `None`
    /// until a new element is inserted, then they can refer to any new element.
    fn remove(&mut self, index: Self::Index) -> Option<T>;
    /// Get a mutable reference to an element in the slab, if any.
    /// [Self::get] and [Self::get_mut] must both return `Some` or both return `None`, and if both
    /// `Some`, they must return a same reference to the same element.
    fn get_mut(&mut self, index: Self::Index) -> Option<Self::RefMut<'_, T>>;
    /// If this slab is completely owned (not a derivative of a shared slab), clear all elements and
    /// return `true`. Otherwise return `false`.
    fn clear_fast(&mut self) -> bool;
}

/// Trait for a [SlabView] whose `Ref` can be converted into a simple reference.
pub trait SlabViewWithSimpleRef<T>: SlabView<T> {
    /// Convert the slab view's `Ref` into a simple shared reference
    ///
    /// The implementation is probably:
    ///
    /// ```text
    /// #[inline]
    /// fn convert_into_simple_ref<'a, U: ?Sized>(r#ref: Self::Ref<'a, U>) -> &'a U where T: 'a {
    ///     r#ref
    /// }
    /// ```
    fn convert_into_simple_ref<'a, U: ?Sized>(r#ref: Self::Ref<'a, U>) -> &'a U where T: 'a;
    /// Convert the slab view's mapped `Ref` into a simple shared reference
    ///
    /// The implementation is probably:
    ///
    /// ```text
    /// #[inline]
    /// fn convert_mapped_into_simple_ref<'a, U: ?Sized>(
    ///     r#ref: <Self::Ref<'a, T> as Ref<'a, T>>::Mapped<U>
    /// ) -> &'a U where T: 'a {
    ///     r#ref
    /// }
    /// ```
    fn convert_mapped_into_simple_ref<'a, U: ?Sized>(
        r#ref: <Self::Ref<'a, T> as Ref<'a, T>>::Mapped<U>
    ) -> &'a U where T: 'a;
}

/// Trait for a [Slab] whose `Ref` and `RefMut` can be converted into simple references.
pub trait SlabWithSimpleRefs<T>: Slab<T> + SlabViewWithSimpleRef<T> {
    /// Convert the slab view's `RefMut` into a simple mutable reference
    ///
    /// The implementation is probably:
    ///
    /// ```text
    /// #[inline]
    /// fn convert_into_simple_mut<'a, U: ?Sized>(r#ref: Self::RefMut<'a, U>) -> &'a U where T: 'a {
    ///     r#ref
    /// }
    /// ```
    fn convert_into_simple_mut<'a, U: ?Sized>(r#ref: Self::RefMut<'a, U>) -> &'a U where T: 'a;
    /// Convert the slab view's mapped `Ref` into a simple shared reference
    ///
    /// The implementation is probably:
    ///
    /// ```text
    /// #[inline]
    /// fn convert_mapped_into_simple_mut<'a, U: ?Sized>(
    ///     r#ref: <Self::RefMut<'a, T> as RefMut<'a, T>>::Mapped<U>
    /// ) -> &'a mut U where T: 'a {
    ///     r#ref
    /// }
    /// ```
    fn convert_mapped_into_simple_mut<'a, U: ?Sized>(
        r#ref: <Self::RefMut<'a, T> as RefMut<'a, T>>::Mapped<U>
    ) -> &'a mut U where T: 'a;
}

/// Marker trait for a slab which is completely owned by a collection (not a derivative of a shared
/// slab).
///
/// This means that there are no elements from other slabs, so its length will always be the
/// collection's length, we can completely iterate/clear it when iterate/clearing the collection,
/// and other relations/operations are simplified.
pub trait OwnedSlab<T>: Slab<T> {
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
impl<T> SlabView<T> for slab::Slab<T> {
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
    fn convert_into_simple_ref<'a, U: ?Sized>(r#ref: Self::Ref<'a, U>) -> &'a U where T: 'a {
        r#ref
    }

    #[inline]
    fn convert_mapped_into_simple_ref<'a, U: ?Sized>(
        r#ref: <Self::Ref<'a, T> as Ref<'a, T>>::Mapped<U>
    ) -> &'a U where T: 'a {
        r#ref
    }
}

#[cfg(any(doc, feature = "slab"))]
impl<T> Slab<T> for slab::Slab<T> {
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
    fn convert_into_simple_mut<'a, U: ?Sized>(r#ref: Self::RefMut<'a, U>) -> &'a U where T: 'a {
        r#ref
    }

    #[inline]
    fn convert_mapped_into_simple_mut<'a, U: ?Sized>(
        r#ref: <Self::RefMut<'a, T> as RefMut<'a, T>>::Mapped<U>
    ) -> &'a mut U where T: 'a {
        r#ref
    }
}