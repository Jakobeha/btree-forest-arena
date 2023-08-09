use crate::BTreeStore;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::mem::{size_of, transmute, MaybeUninit};
use std::ops::{Deref, RangeBounds};

/// A copyable, immutable b-tree set, which doesn't drop its contents.
pub struct BTreeSet<'store, T> {
    inner: RawBTreeSet<'store, T>,
}

pub type Iter<'a, T> = crate::set::Iter<'a, T>;
pub type Range<'a, T> = crate::set::Range<'a, T>;

impl<'store, T> From<crate::BTreeSet<'store, T>> for BTreeSet<'store, T> {
    /// Creates a copyable set from a non-copyable set. Afterwards, the set is no longer mutable and
    /// will no longer drop its contents.
    #[inline]
    fn from(inner: crate::BTreeSet<'store, T>) -> Self {
        Self {
            inner: RawBTreeSet::from(inner),
        }
    }
}

impl<'store, T> BTreeSet<'store, T> {
    /// Helper function to construct a copyable b-tree set by constructing a mutable one and then
    /// immediately wrapping it.
    ///
    /// This literally just creates the mutable set, runs the inner function, and then wraps it.
    #[inline]
    pub fn build(
        store: &'store BTreeStore<T, ()>,
        f: impl FnOnce(&mut crate::BTreeSet<'store, T>),
    ) -> Self {
        let mut set = crate::BTreeSet::new_in(store);
        f(&mut set);
        Self::from(set)
    }

    /// Returns the number of elements in the set.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the set contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// The first value in the set, or `None` if empty.
    #[inline]
    pub fn first(&self) -> Option<&T> {
        self.inner.first()
    }

    /// The last value in the set, or `None` if empty.
    #[inline]
    pub fn last(&self) -> Option<&T> {
        self.inner.last()
    }

    /// Returns `true` if the set contains a value.
    #[inline]
    pub fn contains<U: Ord + ?Sized>(&self, value: &U) -> bool
    where
        T: Borrow<U>,
    {
        self.inner.contains(value)
    }

    /// Returns a reference to the equivalent value in the set, if any.
    ///
    /// This is (only) useful when `U` is a different type than `T`.
    #[inline]
    pub fn get<U: Ord + ?Sized>(&self, value: &U) -> Option<&T>
    where
        T: Borrow<U>,
    {
        self.inner.get(value)
    }

    /// Validates the set, *panic*ing if it is invalid. Specifically, we check that the number of
    /// entries in each node is within the b-tree invariant bounds, and that the elements are in
    /// order.
    ///
    /// Ideally, this should always be a no-op.
    #[inline]
    pub fn validate(&self)
    where
        T: Debug + Ord,
    {
        self.inner.validate()
    }

    /// Prints the b-tree in ascii
    #[inline]
    pub fn print(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    where
        T: Debug,
    {
        self.inner.print(f)
    }

    /// Returns an iterator over the set.
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        self.inner.iter()
    }

    /// Returns an iterator over the set within the given bounds
    #[inline]
    pub fn range<U: Ord + ?Sized>(&self, bounds: impl RangeBounds<U>) -> Range<T>
    where
        T: Borrow<U>,
    {
        self.inner.range(bounds)
    }
}

// region common trait impls
impl<'store, T: Debug> Debug for BTreeSet<'store, T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl<'store, T> Clone for BTreeSet<'store, T> {
    #[inline]
    fn clone(&self) -> Self {
        // SAFETY: This is copy-able because:
        // - All of the fields (all of `inner`'s fields) are copy-able
        // - `inner` implements [Drop], but we wrapped it in [ManuallyDrop]
        // - Most importantly, we only allow access to functions which access inner's indirect data
        //   via shared references, which means we can safely create multiple copies which point to
        //   the same indirect data.
        *self
    }
}

impl<'store, T> Copy for BTreeSet<'store, T> {}

impl<'store, T: PartialEq> PartialEq for BTreeSet<'store, T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        &*self.inner == &*other.inner
    }

    #[inline]
    fn ne(&self, other: &Self) -> bool {
        &*self.inner != &*other.inner
    }
}

impl<'store, T: Eq> Eq for BTreeSet<'store, T> {}

impl<'store, T: PartialOrd> PartialOrd for BTreeSet<'store, T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
}

impl<'store, T: Ord> Ord for BTreeSet<'store, T> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl<'store, T: Hash> Hash for BTreeSet<'store, T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}
// endregion

// region RawBTreeSet
/// [crate::BTreeSet] but as raw data so it can be [Copy]'d. Also doesn't run drop code.
struct RawBTreeSet<'store, T> {
    // generic parameters may not be used in const operations
    // But fortunately [crate::BTreeSet]'s size doesn't depend on its generics, because everything
    // is under an indirect pointer, and `T` is [Sized]
    data: [MaybeUninit<u8>; size_of::<crate::BTreeSet<'static, ()>>()],
    _p: PhantomData<&'store T>,
}

impl<'store, T> From<crate::BTreeSet<'store, T>> for RawBTreeSet<'store, T> {
    #[inline]
    fn from(inner: crate::BTreeSet<'store, T>) -> Self {
        Self {
            data: unsafe { transmute(inner) },
            _p: PhantomData,
        }
    }
}

impl<'store, T> Deref for RawBTreeSet<'store, T> {
    type Target = crate::BTreeSet<'store, T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.data.as_ptr() as *const crate::BTreeSet<'store, T>) }
    }
}

impl<'store, T> Clone for RawBTreeSet<'store, T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            data: self.data,
            _p: PhantomData,
        }
    }
}

impl<'store, T> Copy for RawBTreeSet<'store, T> {}
// endregion

//noinspection DuplicatedCode
impl<'a, 'store: 'a, T> IntoIterator for &'a BTreeSet<'store, T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'store, T> crate::copyable::sealed::BTree<'store, T, ()> for BTreeSet<'store, T> {
    #[inline]
    fn assert_store(&self, store: &BTreeStore<T, ()>) {
        self.inner.assert_store(store)
    }

    #[inline]
    fn nodes(&self) -> crate::copyable::sealed::NodeIter<'store, T, ()> {
        self.inner.nodes()
    }
}
