use crate::generic::{map, node::Node, BTreeMap, SlabView, Slab};
use std::{
	borrow::Borrow,
	cmp::Ordering,
	hash::{Hash, Hasher},
	iter::{DoubleEndedIterator, ExactSizeIterator, FromIterator, FusedIterator, Peekable},
	ops::RangeBounds,
};
use std::collections::hash_map::DefaultHasher;
use std::ops::Deref;
use crate::generic::slab::{Index, OwnedSlab, Ref};

/// A set based on a B-Tree.
///
/// See [`BTreeMap`]'s documentation for a detailed discussion of this collection's performance benefits and drawbacks.
///
/// It is a logic error for an item to be modified in such a way that the item's ordering relative
/// to any other item, as determined by the [`Ord`] trait, changes while it is in the set. This is
/// normally only possible through [`Cell`], [`RefCell`], global state, I/O, or unsafe code.
///
/// [`Ord`]: Ord
/// [`Cell`]: core::cell::Cell
/// [`RefCell`]: core::cell::RefCell
pub struct BTreeSet<T, I: Index, C: SlabView<Node<T, (), I>, Index=I>> {
	map: BTreeMap<T, (), I, C>,
}

/// `Deref`-able pointer to an element in a [BTreeSet]
pub type ElemRef<'a, T, I, C> = <<C as SlabView<Node<T, (), I>>>::Ref<'a, Node<T, (), I>> as Ref<'a, Node<T, (), I>>>::Mapped<T>;

/// `Deref`-able pointer to an element in one of 2 [BTreeSet]s
pub struct EitherElemRef<
	'a,
	T: 'a,
	I: Index + 'a,
	J: Index + 'a,
	C: SlabView<Node<T, (), I>, Index=I> + 'a,
	D: SlabView<Node<T, (), J>, Index=J> + 'a
>(_EitherElemRef<'a, T, I, J, C, D>);

pub enum _EitherElemRef<
	'a,
	T: 'a,
	I: Index + 'a,
	J: Index + 'a,
	C: SlabView<Node<T, (), I>, Index=I> + 'a,
	D: SlabView<Node<T, (), J>, Index=J> + 'a
> {
	Left(ElemRef<'a, T, I, C>),
	Right(ElemRef<'a, T, J, D>)
}

impl<
	'a,
	T,
	I: Index,
	J: Index,
	C: SlabView<Node<T, (), I>, Index=I>,
	D: SlabView<Node<T, (), J>, Index=J>
> EitherElemRef<'a, T, I, J, C, D> {
	pub fn left(left: ElemRef<'a, T, I, C>) -> Self {
		Self(_EitherElemRef::Left(left))
	}

	pub fn right(right: ElemRef<'a, T, J, D>) -> Self {
		Self(_EitherElemRef::Right(right))
	}

	/// Chooses one based on rust's randomized hashing
	pub fn either(left: ElemRef<'a, T, I, C>, right: ElemRef<'a, T, J, D>) -> Self {
		let random_like_hash = 0.hash(&mut DefaultHasher::new()) < 1.hash(&mut DefaultHasher::new());
		match random_like_hash {
			false => Self::left(left),
			true => Self::right(right)
		}
	}
}

impl<
	'a,
	T,
	I: Index,
	J: Index,
	C: SlabView<Node<T, (), I>, Index=I>,
	D: SlabView<Node<T, (), J>, Index=J>
> Deref for EitherElemRef<'a, T, I, J, C, D> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		match &self.0 {
			_EitherElemRef::Left(left) => &*left,
			_EitherElemRef::Right(right) => &*right
		}
	}
}

impl<T, I: Index, C: SlabView<Node<T, (), I>, Index=I>> BTreeSet<T, I, C> {
	/// Makes a new, empty `BTreeSet` in a new allocator.
	///
	/// # Example
	///
	/// ```
	/// # #![allow(unused_mut)]
	/// use btree_slab::BTreeSet;
	///
	/// let mut set: BTreeSet<i32> = BTreeSet::new();
	/// ```
	#[inline]
	pub fn new() -> Self where C: Default {
		Self { map: BTreeMap::new() }
	}

	/// Makes a new, empty `BTreeSet` in the given allocator.
	///
	/// # Example
	///
	/// ```
	/// # #![allow(unused_mut)]
	/// use btree_slab::BTreeSet;
	///
	/// let mut set: BTreeSet<i32> = BTreeSet::new();
	/// ```
	#[inline]
	pub fn new_in(store: C) -> Self {
		Self { map: BTreeMap::new_in(store) }
	}

	/// Returns the number of elements in the set.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut v = BTreeSet::new();
	/// assert_eq!(v.len(), 0);
	/// v.insert(1);
	/// assert_eq!(v.len(), 1);
	/// ```
	#[inline]
	pub fn len(&self) -> usize {
		self.map.len()
	}

	/// Returns `true` if the set contains no elements.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut v = BTreeSet::new();
	/// assert!(v.is_empty());
	/// v.insert(1);
	/// assert!(!v.is_empty());
	/// ```
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}
}

impl<T, I: Index, C: SlabView<Node<T, (), I>, Index=I> + Default> Default for BTreeSet<T, I, C> {
	fn default() -> Self {
		BTreeSet {
			map: BTreeMap::default(),
		}
	}
}

impl<T, I: Index, C: SlabView<Node<T, (), I>, Index=I>> BTreeSet<T, I, C> {
	/// Gets an iterator that visits the values in the `BTreeSet` in ascending order.
	///
	/// # Examples
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let set: BTreeSet<usize> = [1, 2, 3].iter().cloned().collect();
	/// let mut set_iter = set.iter();
	/// assert_eq!(set_iter.next(), Some(&1));
	/// assert_eq!(set_iter.next(), Some(&2));
	/// assert_eq!(set_iter.next(), Some(&3));
	/// assert_eq!(set_iter.next(), None);
	/// ```
	///
	/// Values returned by the iterator are returned in ascending order:
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let set: BTreeSet<usize> = [3, 1, 2].iter().cloned().collect();
	/// let mut set_iter = set.iter();
	/// assert_eq!(set_iter.next(), Some(&1));
	/// assert_eq!(set_iter.next(), Some(&2));
	/// assert_eq!(set_iter.next(), Some(&3));
	/// assert_eq!(set_iter.next(), None);
	/// ```
	#[inline]
	pub fn iter(&self) -> Iter<T, I, C> {
		Iter {
			inner: self.map.keys(),
		}
	}
}

impl<T, I: Index, C: SlabView<Node<T, (), I>, Index=I>> BTreeSet<T, I, C> {
	/// Returns `true` if the set contains a value.
	///
	/// The value may be any borrowed form of the set's value type,
	/// but the ordering on the borrowed form *must* match the
	/// ordering on the value type.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let set: BTreeSet<_> = [1, 2, 3].iter().cloned().collect();
	/// assert_eq!(set.contains(&1), true);
	/// assert_eq!(set.contains(&4), false);
	/// ```
	#[inline]
	pub fn contains<Q: Ord + ?Sized>(&self, value: &Q) -> bool where T: Borrow<Q> {
		self.map.contains_key(value)
	}

	/// Returns a reference to the value in the set, if any, that is equal to the given value.
	///
	/// The value may be any borrowed form of the set's value type,
	/// but the ordering on the borrowed form *must* match the
	/// ordering on the value type.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let set: BTreeSet<_> = [1, 2, 3].iter().cloned().collect();
	/// assert_eq!(set.get(&2), Some(&2));
	/// assert_eq!(set.get(&4), None);
	/// ```
	#[inline]
	pub fn get<Q: Ord + ?Sized>(&self, value: &Q) -> Option<ElemRef<'_, T, I, C>> where T: Borrow<Q> {
		match self.map.get_key_value(value) {
			Some(kv) => Some(kv.into_key_ref()),
			None => None,
		}
	}

	/// Constructs a double-ended iterator over a sub-range of elements in the set.
	/// The simplest way is to use the range syntax `min..max`, thus `range(min..max)` will
	/// yield elements from min (inclusive) to max (exclusive).
	/// The range may also be entered as `(Bound<T>, Bound<T>)`, so for example
	/// `range((Excluded(4), Included(10)))` will yield a left-exclusive, right-inclusive
	/// range from 4 to 10.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	/// use std::ops::Bound::Included;
	///
	/// let mut set = BTreeSet::new();
	/// set.insert(3);
	/// set.insert(5);
	/// set.insert(8);
	/// for &elem in set.range((Included(&4), Included(&8))) {
	///     println!("{}", elem);
	/// }
	/// assert_eq!(Some(&5), set.range(4..).next());
	/// ```
	#[inline]
	pub fn range<K: Ord + ?Sized>(&self, range: impl RangeBounds<K>) -> Range<T, I, C> where T: Borrow<K> {
		Range {
			inner: self.map.range(range),
		}
	}

	/// Visits the values representing the union,
	/// i.e., all the values in `self` or `other`, without duplicates,
	/// in ascending order.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut a = BTreeSet::new();
	/// a.insert(1);
	///
	/// let mut b = BTreeSet::new();
	/// b.insert(2);
	///
	/// let union: Vec<_> = a.union(&b).map(|x| *x).collect();
	/// assert_eq!(union, [1, 2]);
	/// ```
	#[inline]
	pub fn union<'a, J: Index, D: SlabView<Node<T, (), J>, Index=J>>(
		&'a self,
		other: &'a BTreeSet<T, J, D>,
	) -> Union<'a, T, I, J, C, D> {
		Union {
			it1: self.iter().peekable(),
			it2: other.iter().peekable(),
		}
	}

	/// Visits the values representing the intersection,
	/// i.e., the values that are both in `self` and `other`,
	/// in ascending order.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut a = BTreeSet::new();
	/// a.insert(1);
	/// a.insert(2);
	///
	/// let mut b = BTreeSet::new();
	/// b.insert(2);
	/// b.insert(3);
	///
	/// let intersection: Vec<_> = a.intersection(&b).cloned().collect();
	/// assert_eq!(intersection, [2]);
	/// ```
	#[inline]
	pub fn intersection<'a, J: Index, D: SlabView<Node<T, (), J>, Index=J>>(
		&'a self,
		other: &'a BTreeSet<T, J, D>,
	) -> Intersection<'a, T, I, J, C, D> {
		Intersection {
			it1: self.iter(),
			it2: other.iter().peekable(),
		}
	}

	/// Visits the values representing the difference,
	/// i.e., the values that are in `self` but not in `other`,
	/// in ascending order.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut a = BTreeSet::new();
	/// a.insert(1);
	/// a.insert(2);
	///
	/// let mut b = BTreeSet::new();
	/// b.insert(2);
	/// b.insert(3);
	///
	/// let diff: Vec<_> = a.difference(&b).cloned().collect();
	/// assert_eq!(diff, [1]);
	/// ```
	#[inline]
	pub fn difference<'a, J: Index, D: SlabView<Node<T, (), J>, Index=J>>(
		&'a self,
		other: &'a BTreeSet<T, J, D>,
	) -> Difference<'a, T, I, J, C, D> {
		Difference {
			it1: self.iter(),
			it2: other.iter().peekable(),
		}
	}

	/// Visits the values representing the symmetric difference,
	/// i.e., the values that are in `self` or in `other` but not in both,
	/// in ascending order.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut a = BTreeSet::new();
	/// a.insert(1);
	/// a.insert(2);
	///
	/// let mut b = BTreeSet::new();
	/// b.insert(2);
	/// b.insert(3);
	///
	/// let sym_diff: Vec<_> = a.symmetric_difference(&b).map(|x| *x).collect();
	/// assert_eq!(sym_diff, [1, 3]);
	/// ```
	#[inline]
	pub fn symmetric_difference<'a, J: Index, D: SlabView<Node<T, (), J>, Index=J>>(
		&'a self,
		other: &'a BTreeSet<T, J, D>,
	) -> SymmetricDifference<'a, T, I, J, C, D> {
		SymmetricDifference {
			it1: self.iter().peekable(),
			it2: other.iter().peekable(),
		}
	}

	/// Returns `true` if `self` has no elements in common with `other`.
	/// This is equivalent to checking for an empty intersection.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let a: BTreeSet<_> = [1, 2, 3].iter().cloned().collect();
	/// let mut b = BTreeSet::new();
	///
	/// assert_eq!(a.is_disjoint(&b), true);
	/// b.insert(4);
	/// assert_eq!(a.is_disjoint(&b), true);
	/// b.insert(1);
	/// assert_eq!(a.is_disjoint(&b), false);
	/// ```
	#[inline]
	pub fn is_disjoint<J: Index, D: SlabView<Node<T, (), J>, Index=J>>(
		&self,
		other: &BTreeSet<T, J, D>
	) -> bool where T: Ord {
		self.intersection(other).next().is_none()
	}

	/// Returns `true` if the set is a subset of another,
	/// i.e., `other` contains at least all the values in `self`.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let sup: BTreeSet<_> = [1, 2, 3].iter().cloned().collect();
	/// let mut set = BTreeSet::new();
	///
	/// assert_eq!(set.is_subset(&sup), true);
	/// set.insert(2);
	/// assert_eq!(set.is_subset(&sup), true);
	/// set.insert(4);
	/// assert_eq!(set.is_subset(&sup), false);
	/// ```
	#[inline]
	pub fn is_subset<J: Index, D: SlabView<Node<T, (), J>, Index=J>>(&self, other: &BTreeSet<T, J, D>) -> bool where T: Ord {
		self.difference(other).next().is_none()
	}

	/// Returns `true` if the set is a superset of another,
	/// i.e., `self` contains at least all the values in `other`.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let sub: BTreeSet<_> = [1, 2].iter().cloned().collect();
	/// let mut set = BTreeSet::new();
	///
	/// assert_eq!(set.is_superset(&sub), false);
	///
	/// set.insert(0);
	/// set.insert(1);
	/// assert_eq!(set.is_superset(&sub), false);
	///
	/// set.insert(2);
	/// assert_eq!(set.is_superset(&sub), true);
	/// ```
	#[inline]
	pub fn is_superset<J: Index, D: SlabView<Node<T, (), J>, Index=J>>(
		&self,
		other: &BTreeSet<T, J, D>
	) -> bool where T: Ord {
		other.is_subset(self)
	}

	/// Returns a reference to the first value in the set, if any.
	/// This value is always the minimum of all values in the set.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut map = BTreeSet::new();
	/// assert_eq!(map.first(), None);
	/// map.insert(1);
	/// assert_eq!(map.first(), Some(&1));
	/// map.insert(2);
	/// assert_eq!(map.first(), Some(&1));
	/// ```
	#[inline]
	pub fn first(&self) -> Option<ElemRef<'_, T, I, C>> {
		self.map.first_key_value().map(|kv| kv.into_key_ref())
	}

	/// Returns a reference to the last value in the set, if any.
	/// This value is always the maximum of all values in the set.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut map = BTreeSet::new();
	/// assert_eq!(map.first(), None);
	/// map.insert(1);
	/// assert_eq!(map.last(), Some(&1));
	/// map.insert(2);
	/// assert_eq!(map.last(), Some(&2));
	/// ```
	#[inline]
	pub fn last(&self) -> Option<ElemRef<'_, T, I, C>> {
		self.map.last_key_value().map(|kv| kv.into_key_ref())
	}
}

impl<T, I: Index, C: Slab<Node<T, (), I>, Index=I>> BTreeSet<T, I, C> {
	/// Clears the set, removing all values.
	///
	/// # Examples
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut v = BTreeSet::new();
	/// v.insert(1);
	/// v.clear();
	/// assert!(v.is_empty());
	/// ```
	#[inline]
	pub fn clear(&mut self) {
		self.map.clear()
	}

	/// Adds a value to the set.
	///
	/// If the set did not have this value present, `true` is returned.
	///
	/// If the set did have this value present, `false` is returned, and the
	/// entry is not updated. See the [module-level documentation] for more.
	///
	/// [module-level documentation]: index.html#insert-and-complex-keys
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut set = BTreeSet::new();
	///
	/// assert_eq!(set.insert(2), true);
	/// assert_eq!(set.insert(2), false);
	/// assert_eq!(set.len(), 1);
	/// ```
	#[inline]
	pub fn insert(&mut self, element: T) -> bool where T: Ord {
		self.map.insert(element, ()).is_none()
	}

	/// Removes a value from the set. Returns whether the value was
	/// present in the set.
	///
	/// The value may be any borrowed form of the set's value type,
	/// but the ordering on the borrowed form *must* match the
	/// ordering on the value type.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut set = BTreeSet::new();
	///
	/// set.insert(2);
	/// assert_eq!(set.remove(&2), true);
	/// assert_eq!(set.remove(&2), false);
	/// ```
	#[inline]
	pub fn remove<Q: Ord + ?Sized>(&mut self, value: &Q) -> bool where T: Borrow<Q> {
		self.map.remove(value).is_some()
	}

	/// Removes and returns the value in the set, if any, that is equal to the given one.
	///
	/// The value may be any borrowed form of the set's value type,
	/// but the ordering on the borrowed form *must* match the
	/// ordering on the value type.
	///
	/// # Examples
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut set: BTreeSet<_> = [1, 2, 3].iter().cloned().collect();
	/// assert_eq!(set.take(&2), Some(2));
	/// assert_eq!(set.take(&2), None);
	/// ```
	#[inline]
	pub fn take<Q: Ord + ?Sized>(&mut self, value: &Q) -> Option<T> where T: Borrow<Q> {
		self.map.remove_entry(value).map(|(t, _)| t)
	}

	/// Adds a value to the set, replacing the existing value, if any, that is equal to the given
	/// one. Returns the replaced value.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut set = BTreeSet::new();
	/// set.insert(Vec::<i32>::new());
	///
	/// assert_eq!(set.get(&[][..]).unwrap().capacity(), 0);
	/// set.replace(Vec::with_capacity(10));
	/// assert_eq!(set.get(&[][..]).unwrap().capacity(), 10);
	/// ```
	#[inline]
	pub fn replace(&mut self, value: T) -> Option<T> where T: Ord {
		self.map.replace(value, ()).map(|(t, ())| t)
	}

	/// Removes the first value from the set and returns it, if any.
	/// The first value is always the minimum value in the set.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut set = BTreeSet::new();
	///
	/// set.insert(1);
	/// while let Some(n) = set.pop_first() {
	///     assert_eq!(n, 1);
	/// }
	/// assert!(set.is_empty());
	/// ```
	#[inline]
	pub fn pop_first(&mut self) -> Option<T> {
		self.map.pop_first().map(|kv| kv.0)
	}

	/// Removes the last value from the set and returns it, if any.
	/// The last value is always the maximum value in the set.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut set = BTreeSet::new();
	///
	/// set.insert(1);
	/// while let Some(n) = set.pop_last() {
	///     assert_eq!(n, 1);
	/// }
	/// assert!(set.is_empty());
	/// ```
	#[inline]
	pub fn pop_last(&mut self) -> Option<T> {
		self.map.pop_last().map(|kv| kv.0)
	}

	/// Retains only the elements specified by the predicate.
	///
	/// In other words, remove all elements `e` such that `f(&e)` returns `false`.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let xs = [1, 2, 3, 4, 5, 6];
	/// let mut set: BTreeSet<i32> = xs.iter().cloned().collect();
	/// // Keep only the even numbers.
	/// set.retain(|&k| k % 2 == 0);
	/// assert!(set.iter().eq([2, 4, 6].iter()));
	/// ```
	#[inline]
	pub fn retain(&mut self, mut f: impl FnMut(&T) -> bool) {
		self.drain_filter(|v| !f(v));
	}

	/// Creates an iterator which uses a closure to determine if a value should be removed.
	///
	/// If the closure returns true, then the value is removed and yielded.
	/// If the closure returns false, the value will remain in the list and will not be yielded
	/// by the iterator.
	///
	/// If the iterator is only partially consumed or not consumed at all, each of the remaining
	/// values will still be subjected to the closure and removed and dropped if it returns true.
	///
	/// It is unspecified how many more values will be subjected to the closure
	/// if a panic occurs in the closure, or if a panic occurs while dropping a value, or if the
	/// `DrainFilter` itself is leaked.
	///
	/// # Example
	///
	/// Splitting a set into even and odd values, reusing the original set:
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut set: BTreeSet<i32> = (0..8).collect();
	/// let evens: BTreeSet<_> = set.drain_filter(|v| v % 2 == 0).collect();
	/// let odds = set;
	/// assert_eq!(evens.into_iter().collect::<Vec<_>>(), vec![0, 2, 4, 6]);
	/// assert_eq!(odds.into_iter().collect::<Vec<_>>(), vec![1, 3, 5, 7]);
	/// ```
	#[inline]
	pub fn drain_filter<'a, F: 'a + FnMut(&T) -> bool>(
		&'a mut self,
		pred: F
	) -> DrainFilter<'a, T, I, C, F> {
		DrainFilter::new(self, pred)
	}
}

impl<T: Ord, I: Index, C: OwnedSlab<Node<T, (), I>, Index=I> + Default> BTreeSet<T, I, C> {
	/// Moves all elements from `other` into `Self`, leaving `other` empty and with a new store.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::BTreeSet;
	///
	/// let mut a = BTreeSet::new();
	/// a.insert(1);
	/// a.insert(2);
	/// a.insert(3);
	///
	/// let mut b = BTreeSet::new();
	/// b.insert(3);
	/// b.insert(4);
	/// b.insert(5);
	///
	/// a.append1(&mut b);
	///
	/// assert_eq!(a.len(), 5);
	/// assert_eq!(b.len(), 0);
	///
	/// assert!(a.contains(&1));
	/// assert!(a.contains(&2));
	/// assert!(a.contains(&3));
	/// assert!(a.contains(&4));
	/// assert!(a.contains(&5));
	/// ```
	#[inline]
	pub fn append1(&mut self, other: &mut Self) {
		self.map.append1(&mut other.map);
	}
}

impl<'a, T: Ord, I: Index, C> BTreeSet<T, I, &'a C> where &'a C: Slab<Node<T, (), I>, Index=I> {
	/// Asserts that `self` and `other` have the same store. Then, moves all elements from `other`
	/// into `Self`, leaving `other` empty.
	///
	/// # Example
	///
	/// ```
	/// use btree_slab::{SharingBTreeSet, shareable_slab::ShareableSlab};
	///
	/// let data = ShareableSlab::new();
	///
	/// let mut a = SharingBTreeSet::new_in(&data);
	/// a.insert(1);
	/// a.insert(2);
	/// a.insert(3);
	///
	/// let mut b = SharingBTreeSet::new_in(&data);
	/// b.insert(3);
	/// b.insert(4);
	/// b.insert(5);
	///
	/// a.append2(&mut b);
	///
	/// assert_eq!(a.len(), 5);
	/// assert_eq!(b.len(), 0);
	///
	/// assert!(a.contains(&1));
	/// assert!(a.contains(&2));
	/// assert!(a.contains(&3));
	/// assert!(a.contains(&4));
	/// assert!(a.contains(&5));
	/// ```
	#[inline]
	pub fn append2(&mut self, other: &mut Self) {
		self.map.append2(&mut other.map);
	}
}

impl<T: Clone, I: Index, C: SlabView<Node<T, (), I>, Index=I> + Clone> Clone for BTreeSet<T, I, C> {
	#[inline]
	fn clone(&self) -> Self {
		BTreeSet {
			map: self.map.clone(),
		}
	}

	#[inline]
	fn clone_from(&mut self, other: &Self) {
		self.map.clone_from(&other.map);
	}
}

impl<T: Ord, J: Index, C: Slab<Node<T, (), J>, Index=J> + Default> FromIterator<T> for BTreeSet<T, J, C> {
	#[inline]
	fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
		let mut set = BTreeSet::new();
		set.extend(iter);
		set
	}
}

impl<T, I: Index, C: Slab<Node<T, (), I>, Index=I>> IntoIterator for BTreeSet<T, I, C> {
	type Item = T;
	type IntoIter = IntoIter<T, I, C>;

	#[inline]
	fn into_iter(self) -> IntoIter<T, I, C> {
		IntoIter {
			inner: self.map.into_keys(),
		}
	}
}

impl<'a, T, I: Index, C: Slab<Node<T, (), I>, Index=I>> IntoIterator for &'a BTreeSet<T, I, C> {
	type Item = ElemRef<'a, T, I, C>;
	type IntoIter = Iter<'a, T, I, C>;

	#[inline]
	fn into_iter(self) -> Iter<'a, T, I, C> {
		self.iter()
	}
}

impl<T: Ord, J: Index, C: Slab<Node<T, (), J>, Index=J>> Extend<T> for BTreeSet<T, J, C> {
	#[inline]
	fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
		for t in iter {
			self.insert(t);
		}
	}
}

impl<'a, T: 'a + Ord + Copy, J: Index, C: Slab<Node<T, (), J>, Index=J>> Extend<&'a T> for BTreeSet<T, J, C> {
	#[inline]
	fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
		self.extend(iter.into_iter().copied())
	}
}

impl<
	T,
	L: PartialEq<T>,
	I: Index,
	J: Index + PartialEq<I>,
	C: SlabView<Node<T, (), I>, Index=I>,
	D: SlabView<Node<L, (), J>, Index=J>
> PartialEq<BTreeSet<L, J, D>> for BTreeSet<T, I, C> {
	#[inline]
	fn eq(&self, other: &BTreeSet<L, J, D>) -> bool {
		self.map.eq(&other.map)
	}
}

impl<T: Eq, I: Index, C: SlabView<Node<T, (), I>, Index=I>> Eq for BTreeSet<T, I, C> {}

impl<
	T,
	L: PartialOrd<T>,
	I: Index,
	J: Index + PartialOrd<I>,
	C: SlabView<Node<T, (), I>, Index=I>,
	D: SlabView<Node<L, (), J>, Index=J>
> PartialOrd<BTreeSet<L, J, D>>
	for BTreeSet<T, I, C> {
	#[inline]
	fn partial_cmp(&self, other: &BTreeSet<L, J, D>) -> Option<Ordering> {
		self.map.partial_cmp(&other.map)
	}
}

impl<T: Ord, I: Index + Ord, C: SlabView<Node<T, (), I>, Index=I>> Ord for BTreeSet<T, I, C> {
	#[inline]
	fn cmp(&self, other: &BTreeSet<T, I, C>) -> Ordering {
		self.map.cmp(&other.map)
	}
}

impl<T: Hash, I: Index + Hash, C: SlabView<Node<T, (), I>, Index=I>> Hash for BTreeSet<T, I, C> {
	#[inline]
	fn hash<H: Hasher>(&self, h: &mut H) {
		self.map.hash(h)
	}
}

pub struct Iter<'a, T, I: Index, C: SlabView<Node<T, (), I>, Index=I>> {
	inner: map::Keys<'a, T, (), I, C>,
}

impl<'a, T, I: Index, C: SlabView<Node<T, (), I>, Index=I>> Iterator for Iter<'a, T, I, C> {
	type Item = ElemRef<'a, T, I, C>;

	#[inline]
	fn next(&mut self) -> Option<ElemRef<'a, T, I, C>> {
		self.inner.next()
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.inner.size_hint()
	}
}

impl<'a, T, I: Index, C: SlabView<Node<T, (), I>, Index=I>> DoubleEndedIterator for Iter<'a, T, I, C> {
	#[inline]
	fn next_back(&mut self) -> Option<ElemRef<'a, T, I, C>> {
		self.inner.next_back()
	}
}

impl<'a, T, I: Index, C: SlabView<Node<T, (), I>, Index=I>> FusedIterator for Iter<'a, T, I, C> {}
impl<'a, T, I: Index, C: SlabView<Node<T, (), I>, Index=I>> ExactSizeIterator for Iter<'a, T, I, C> {}

pub struct IntoIter<T, I: Index, C: SlabView<Node<T, (), I>, Index=I>> {
	inner: map::IntoKeys<T, (), I, C>,
}

impl<T, I: Index, C: Slab<Node<T, (), I>, Index=I>> Iterator for IntoIter<T, I, C> {
	type Item = T;

	#[inline]
	fn next(&mut self) -> Option<T> {
		self.inner.next()
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.inner.size_hint()
	}
}

impl<T, I: Index, C: Slab<Node<T, (), I>, Index=I>> DoubleEndedIterator for IntoIter<T, I, C> {
	#[inline]
	fn next_back(&mut self) -> Option<T> {
		self.inner.next_back()
	}
}

impl<T, I: Index, C: Slab<Node<T, (), I>, Index=I>> FusedIterator for IntoIter<T, I, C> {}
impl<T, I: Index, C: Slab<Node<T, (), I>, Index=I>> ExactSizeIterator for IntoIter<T, I, C> {}

pub struct Union<'a, T, I: Index, J: Index, C: SlabView<Node<T, (), I>, Index=I>, D: SlabView<Node<T, (), J>, Index=J>> {
	it1: Peekable<Iter<'a, T, I, C>>,
	it2: Peekable<Iter<'a, T, J, D>>,
}

impl<'a, T: Ord, I: Index, J: Index, C: SlabView<Node<T, (), I>, Index=I>, D: SlabView<Node<T, (), J>, Index=J>> Iterator for Union<'a, T, I, J, C, D> {
	type Item = EitherElemRef<'a, T, I, J, C, D>;

	#[inline]
	fn next(&mut self) -> Option<EitherElemRef<'a, T, I, J, C, D>> {
		match (self.it1.peek(), self.it2.peek()) {
			(Some(v1), Some(v2)) => Some(match v1.cmp(v2) {
				Ordering::Equal => EitherElemRef::either(
					self.it1.next().unwrap(),
					self.it2.next().unwrap()
				),
				Ordering::Less => EitherElemRef::left(self.it1.next().unwrap()),
				Ordering::Greater => EitherElemRef::right(self.it2.next().unwrap()),
			}),
			(Some(_), None) => Some(EitherElemRef::left(self.it1.next().unwrap())),
			(None, Some(_)) => Some(EitherElemRef::right(self.it2.next().unwrap())),
			(None, None) => None,
		}
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		let len1 = self.it1.len();
		let len2 = self.it2.len();

		(std::cmp::min(len1, len2), Some(std::cmp::max(len1, len2)))
	}
}

impl<'a, T: Ord, I: Index, J: Index, C: SlabView<Node<T, (), I>, Index=I>, D: SlabView<Node<T, (), J>, Index=J>> FusedIterator for Union<'a, T, I, J, C, D> {}

pub struct Intersection<'a, T, I: Index, J: Index, C: SlabView<Node<T, (), I>, Index=I>, D: SlabView<Node<T, (), J>, Index=J>> {
	it1: Iter<'a, T, I, C>,
	it2: Peekable<Iter<'a, T, J, D>>,
}

impl<'a, T: Ord, I: Index, J: Index, C: SlabView<Node<T, (), I>, Index=I>, D: SlabView<Node<T, (), J>, Index=J>> Iterator for Intersection<'a, T, I, J, C, D> {
	type Item = ElemRef<'a, T, I, C>;

	#[inline]
	fn next(&mut self) -> Option<ElemRef<'a, T, I, C>> {
		loop {
			match self.it1.next() {
				Some(value) => {
					let keep = loop {
						match self.it2.peek() {
							Some(other) => match value.cmp(other) {
								Ordering::Equal => break true,
								Ordering::Greater => {
									self.it2.next();
								}
								Ordering::Less => break false,
							},
							None => break false,
						}
					};

					if keep {
						break Some(value);
					}
				}
				None => break None,
			}
		}
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		let len1 = self.it1.len();
		let len2 = self.it2.len();

		(0, Some(std::cmp::min(len1, len2)))
	}
}

impl<'a, T: Ord, I: Index, J: Index, C: SlabView<Node<T, (), I>, Index=I>, D: SlabView<Node<T, (), J>, Index=J>> FusedIterator
	for Intersection<'a, T, I, J, C, D> {}

pub struct Difference<'a, T, I: Index, J: Index, C: SlabView<Node<T, (), I>, Index=I>, D: SlabView<Node<T, (), J>, Index=J>> {
	it1: Iter<'a, T, I, C>,
	it2: Peekable<Iter<'a, T, J, D>>,
}

impl<'a, T: Ord, I: Index, J: Index, C: SlabView<Node<T, (), I>, Index=I>, D: SlabView<Node<T, (), J>, Index=J>> Iterator for Difference<'a, T, I, J, C, D> {
	type Item = ElemRef<'a, T, I, C>;

	#[inline]
	fn next(&mut self) -> Option<ElemRef<'a, T, I, C>> {
		loop {
			match self.it1.next() {
				Some(value) => {
					let keep = loop {
						match self.it2.peek() {
							Some(other) => match value.cmp(other) {
								Ordering::Equal => break false,
								Ordering::Greater => {
									self.it2.next();
								}
								Ordering::Less => break true,
							},
							None => break true,
						}
					};

					if keep {
						break Some(value);
					}
				}
				None => break None,
			}
		}
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		let len1 = self.it1.len();
		let len2 = self.it2.len();

		(len1.saturating_sub(len2), Some(self.it1.len()))
	}
}

impl<'a, T: Ord, I: Index, J: Index, C: SlabView<Node<T, (), I>, Index=I>, D: SlabView<Node<T, (), J>, Index=J>> FusedIterator
	for Difference<'a, T, I, J, C, D> {}

pub struct SymmetricDifference<'a, T, I: Index, J: Index, C: SlabView<Node<T, (), I>, Index=I>, D: SlabView<Node<T, (), J>, Index=J>> {
	it1: Peekable<Iter<'a, T, I, C>>,
	it2: Peekable<Iter<'a, T, J, D>>,
}

impl<'a, T: Ord, I: Index, J: Index, C: SlabView<Node<T, (), I>, Index=I>, D: SlabView<Node<T, (), J>, Index=J>> Iterator
	for SymmetricDifference<'a, T, I, J, C, D> {
	type Item = EitherElemRef<'a, T, I, J, C, D>;

	#[inline]
	fn next(&mut self) -> Option<EitherElemRef<'a, T, I, J, C, D>> {
		loop {
			match (self.it1.peek(), self.it2.peek()) {
				(Some(v1), Some(v2)) => match v1.cmp(v2) {
					Ordering::Equal => {
						self.it1.next().unwrap();
						self.it2.next().unwrap();
					}
					Ordering::Less => break Some(EitherElemRef::left(self.it1.next().unwrap())),
					Ordering::Greater => break Some(EitherElemRef::right(self.it2.next().unwrap())),
				},
				(Some(_), None) => break Some(EitherElemRef::left(self.it1.next().unwrap())),
				(None, Some(_)) => break Some(EitherElemRef::right(self.it2.next().unwrap())),
				(None, None) => break None,
			}
		}
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		let len1 = self.it1.len();
		let len2 = self.it2.len();

		(0, len1.checked_add(len2))
	}
}

impl<'a, T: Ord, I: Index, J: Index, C: SlabView<Node<T, (), I>, Index=I>, D: SlabView<Node<T, (), J>, Index=J>> FusedIterator
	for SymmetricDifference<'a, T, I, J, C, D> {}

pub struct DrainFilter<'a, T, I: Index, C: Slab<Node<T, (), I>, Index=I>, F: FnMut(&T) -> bool> {
	pred: F,
	inner: map::DrainFilterInner<'a, T, (), I, C>,
}

impl<'a, T: 'a, I: Index, C: Slab<Node<T, (), I>, Index=I>, F: FnMut(&T) -> bool> DrainFilter<'a, T, I, C, F> {
	#[inline]
	pub fn new(set: &'a mut BTreeSet<T, I, C>, pred: F) -> Self {
		DrainFilter {
			pred,
			inner: map::DrainFilterInner::new(&mut set.map),
		}
	}
}

impl<'a, T, I: Index, C: Slab<Node<T, (), I>, Index=I>, F: FnMut(&T) -> bool> FusedIterator for DrainFilter<'a, T, I, C, F> {}

impl<'a, T, I: Index, C: Slab<Node<T, (), I>, Index=I>, F: FnMut(&T) -> bool> Iterator for DrainFilter<'a, T, I, C, F> {
	type Item = T;

	#[inline]
	fn next(&mut self) -> Option<T> {
		let pred = &mut self.pred;
		self.inner.next(&mut |t, _| (*pred)(t)).map(|(t, ())| t)
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.inner.size_hint()
	}
}

impl<'a, T, I: Index, C: Slab<Node<T, (), I>, Index=I>, F: FnMut(&T) -> bool> Drop for DrainFilter<'a, T, I, C, F> {
	fn drop(&mut self) {
		loop {
			if self.next().is_none() {
				break;
			}
		}
	}
}

pub struct Range<'a, T, I: Index, C: SlabView<Node<T, (), I>, Index=I>> {
	inner: map::Range<'a, T, (), I, C>,
}

impl<'a, T, I: Index, C: SlabView<Node<T, (), I>, Index=I>> Iterator for Range<'a, T, I, C> {
	type Item = ElemRef<'a, T, I, C>;

	#[inline]
	fn next(&mut self) -> Option<ElemRef<'a, T, I, C>> {
		self.inner.next().map(|kv| kv.into_key_ref())
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.inner.size_hint()
	}
}

impl<'a, T, I: Index, C: SlabView<Node<T, (), I>, Index=I>> DoubleEndedIterator for Range<'a, T, I, C> {
	#[inline]
	fn next_back(&mut self) -> Option<ElemRef<'a, T, I, C>> {
		self.inner.next_back().map(|kv| kv.into_key_ref())
	}
}

impl<'a, T, I: Index, C: SlabView<Node<T, (), I>, Index=I>> FusedIterator for Range<'a, T, I, C> {}
