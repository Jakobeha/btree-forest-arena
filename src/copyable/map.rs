use crate::BTreeStore;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::mem::{size_of, transmute, MaybeUninit};
use std::ops::{Deref, RangeBounds};

/// A copyable, immutable b-tree map, which doesn't drop its contents.
pub struct BTreeMap<'store, K, V> {
    inner: RawBTreeMap<'store, K, V>,
}

pub type Iter<'a, K, V> = crate::map::Iter<'a, K, V>;
pub type Keys<'a, K, V> = crate::map::Keys<'a, K, V>;
pub type Values<'a, K, V> = crate::map::Values<'a, K, V>;
pub type Range<'a, K, V> = crate::map::Range<'a, K, V>;

impl<'store, K, V> From<crate::BTreeMap<'store, K, V>> for BTreeMap<'store, K, V> {
    /// Creates a copyable map from a non-copyable map. Afterwards, the map is no longer mutable and
    /// will no longer drop its contents.
    #[inline]
    fn from(inner: crate::BTreeMap<'store, K, V>) -> Self {
        Self {
            inner: RawBTreeMap::from(inner),
        }
    }
}

impl<'store, K, V> BTreeMap<'store, K, V> {
    /// Helper function to construct a copyable b-tree map by constructing a mutable one and then
    /// immediately wrapping it.
    ///
    /// This literally just creates the mutable map, runs the inner function, and then wraps it.
    #[inline]
    pub fn build(
        store: &'store BTreeStore<K, V>,
        f: impl FnOnce(&mut crate::BTreeMap<'store, K, V>),
    ) -> Self {
        let mut map = crate::BTreeMap::new_in(store);
        f(&mut map);
        Self::from(map)
    }

    // region length
    /// Returns the number of elements in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the map contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    // endregion

    // region retrieval
    /// Whether the map contains the key
    #[inline]
    pub fn contains_key<Q: Ord + ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.inner.contains_key(key)
    }

    /// Returns a reference to the value corresponding to the key.
    #[inline]
    pub fn get<Q: Ord + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.inner.get(key)
    }

    /// Returns a reference to the equivalent key
    ///
    /// This is (only) useful when `Q` is a different type than `K`.
    #[inline]
    pub fn get_key<Q: Ord + ?Sized>(&self, key: &Q) -> Option<&K>
    where
        K: Borrow<Q>,
    {
        self.inner.get_key(key)
    }

    /// Returns a reference to the equivalent key and associated value
    ///
    /// This is (only) useful when `Q` is a different type than `K`.
    #[inline]
    pub fn get_key_value<Q: Ord + ?Sized>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
    {
        self.inner.get_key_value(key)
    }

    /// Returns the first key and value
    #[inline]
    pub fn first_key_value(&self) -> Option<(&K, &V)> {
        self.inner.first_key_value()
    }

    /// Returns the last key and value
    #[inline]
    pub fn last_key_value(&self) -> Option<(&K, &V)> {
        self.inner.last_key_value()
    }
    // endregion

    // region advanced
    /// Validates the map, *panic*ing if it is invalid. Specifically, we check that the number of
    /// entries in each node is within the b-tree invariant bounds, and that the keys are in order.
    ///
    /// Ideally, this should always be a no-op.
    #[inline]
    pub fn validate(&self)
    where
        K: Debug + Ord,
        V: Debug,
    {
        self.inner.validate()
    }

    /// Prints the b-tree in ascii
    #[inline]
    pub fn print(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    where
        K: Debug,
        V: Debug,
    {
        self.inner.print(f)
    }
    // endregion

    // region iteration
    /// Iterates over the map's key-value pairs in order.
    #[inline]
    pub fn iter(&self) -> Iter<'_, K, V> {
        self.inner.iter()
    }

    /// Iterates over the map's keys in order.
    #[inline]
    pub fn keys(&self) -> Keys<'_, K, V> {
        self.inner.keys()
    }

    /// Iterates over the map's values in order.
    #[inline]
    pub fn values(&self) -> Values<'_, K, V> {
        self.inner.values()
    }

    /// Iterates over the map's key-value pairs in order, within the given range.
    #[inline]
    pub fn range<Q: Ord + ?Sized>(&self, bounds: impl RangeBounds<Q>) -> Range<'_, K, V>
    where
        K: Borrow<Q>,
    {
        self.inner.range(bounds)
    }

    /// Iterates over the map's keys in order, within the given range.
    #[inline]
    pub fn range_keys<Q: Ord + ?Sized>(
        &self,
        bounds: impl RangeBounds<Q>,
    ) -> impl Iterator<Item = &K> + '_
    where
        K: Borrow<Q>,
    {
        self.inner.range_keys(bounds)
    }

    /// Iterates over the map's values in order, within the given range.
    #[inline]
    pub fn range_values<Q: Ord + ?Sized>(
        &self,
        bounds: impl RangeBounds<Q>,
    ) -> impl Iterator<Item = &V> + '_
    where
        K: Borrow<Q>,
    {
        self.inner.range_values(bounds)
    }
}

// region common trait impls
impl<'store, K: Debug, V: Debug> Debug for BTreeMap<'store, K, V> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl<'store, K, V> Clone for BTreeMap<'store, K, V> {
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

impl<'store, K, V> Copy for BTreeMap<'store, K, V> {}

impl<'store, K: PartialEq, V: PartialEq> PartialEq for BTreeMap<'store, K, V> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        &*self.inner == &*other.inner
    }

    #[inline]
    fn ne(&self, other: &Self) -> bool {
        &*self.inner != &*other.inner
    }
}

impl<'store, K: Eq, V: Eq> Eq for BTreeMap<'store, K, V> {}

impl<'store, K: PartialOrd, V: PartialOrd> PartialOrd for BTreeMap<'store, K, V> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
}

impl<'store, K: Ord, V: Ord> Ord for BTreeMap<'store, K, V> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl<'store, K: Hash, V: Hash> Hash for BTreeMap<'store, K, V> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}
// endregion

// region RawBTreeMap
/// [crate::BTreeMap] but as raw data so it can be [Copy]'d. Also doesn't run drop code.
struct RawBTreeMap<'store, K, V> {
    // generic parameters may not be used in const operations
    // But fortunately [crate::BTreeMap]'s size doesn't depend on its generics, because everything
    // is under an indirect pointer, and `K` and `V` are [Sized]
    data: [MaybeUninit<u8>; size_of::<crate::BTreeMap<'static, (), ()>>()],
    _p: PhantomData<(&'store K, &'store V)>,
}

impl<'store, K, V> From<crate::BTreeMap<'store, K, V>> for RawBTreeMap<'store, K, V> {
    #[inline]
    fn from(inner: crate::BTreeMap<'store, K, V>) -> Self {
        Self {
            data: unsafe { transmute(inner) },
            _p: PhantomData,
        }
    }
}

impl<'store, K, V> Deref for RawBTreeMap<'store, K, V> {
    type Target = crate::BTreeMap<'store, K, V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.data.as_ptr() as *const crate::BTreeMap<'store, K, V>) }
    }
}

impl<'store, K, V> Clone for RawBTreeMap<'store, K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            data: self.data,
            _p: PhantomData,
        }
    }
}

impl<'store, K, V> Copy for RawBTreeMap<'store, K, V> {}
// endregion

//noinspection DuplicatedCode
// region iterator impls
impl<'store: 'a, 'a, K, V> IntoIterator for &'a BTreeMap<'store, K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
// endregion

impl<'store, K, V> crate::copyable::sealed::BTree<'store, K, V> for BTreeMap<'store, K, V> {
    #[inline]
    fn assert_store(&self, store: &BTreeStore<K, V>) {
        self.inner.assert_store(store)
    }

    #[inline]
    fn nodes(&self) -> crate::copyable::sealed::NodeIter<'store, K, V> {
        self.inner.nodes()
    }
}
