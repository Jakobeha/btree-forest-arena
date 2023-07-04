use crate::{BTreeMap, BTreeStore};
use std::borrow::Borrow;
use std::ops::RangeBounds;

/// A b-tree set.
///
/// See [std::collections::BTreeSet] for more info.
pub struct BTreeSet<'store, T>(BTreeMap<'store, T, ()>);

impl<'store, T> BTreeSet<'store, T> {
    /// Creates an empty set.
    #[inline]
    pub fn new_in(store: &'store BTreeStore<T, ()>) -> Self {
        Self(BTreeMap::new_in(store))
    }

    /// Returns the number of elements in the set.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the set contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Clears the set, removing all values.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }

    /// The first value in the set, or `None` if empty.
    #[inline]
    pub fn first(&self) -> Option<&T> {
        self.0.first_key_value().map(|(k, &())| k)
    }

    /// The last value in the set, or `None` if empty.
    #[inline]
    pub fn last(&self) -> Option<&T> {
        self.0.last_key_value().map(|(k, &())| k)
    }

    /// Returns `true` if the set contains a value.
    #[inline]
    pub fn contains<U: Ord>(&self, value: &U) -> bool
    where
        T: Borrow<U>,
    {
        self.0.contains_key(value)
    }

    /// Inserts a value into the set. Returns `true` if the value was not already present.
    #[inline]
    pub fn insert(&mut self, value: T) -> bool
    where
        T: Clone + Ord,
    {
        self.0.insert(value, ()).is_none()
    }

    /// Removes a value from the set. Returns `true` if the value was present.
    #[inline]
    pub fn remove<U: Ord>(&mut self, value: &U) -> bool
    where
        T: Borrow<U> + Clone,
    {
        self.0.remove(value).is_some()
    }

    /// Removes the first value from the set.
    #[inline]
    pub fn pop_first(&mut self) -> Option<T>
    where
        T: Clone,
    {
        self.0.pop_first().map(|(k, ())| k)
    }

    /// Removes the last value from the set.
    #[inline]
    pub fn pop_last(&mut self) -> Option<T>
    where
        T: Clone,
    {
        self.0.pop_last().map(|(k, ())| k)
    }

    /// Returns an iterator over the set.
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter(self.0.iter())
    }

    /// Returns an iterator over the set within the given bounds
    #[inline]
    pub fn range<U: Ord>(&self, bounds: impl RangeBounds<U>) -> Range<T>
    where
        T: Borrow<U>,
    {
        Range(self.0.range(bounds))
    }
}

impl<'store, T> IntoIterator for BTreeSet<'store, T> {
    type Item = T;
    type IntoIter = IntoIter<'store, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.0.into_iter())
    }
}

impl<'a, 'store: 'a, T> IntoIterator for &'a BTreeSet<'store, T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct Iter<'a, T>(crate::map::Iter<'a, T, ()>);

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, &())| k)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct IntoIter<'store, T>(crate::map::IntoIter<'store, T, ()>);

impl<'store, T> Iterator for IntoIter<'store, T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, ())| k)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct Range<'a, T>(crate::map::Range<'a, T, ()>);

impl<'a, T> Iterator for Range<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, &())| k)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
