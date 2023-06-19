use crate::generic::{node::{Address, Balance, Item, Node, WouldUnderflow}, Slab, SlabView};
use std::{
	borrow::Borrow,
	cmp::Ordering,
	hash::{Hash, Hasher},
	iter::{DoubleEndedIterator, ExactSizeIterator, FromIterator, FusedIterator},
	marker::PhantomData,
	ops::{Bound, RangeBounds},
};

mod entry;
mod ext;

pub use entry::*;
pub use ext::*;
use crate::generic::slab::{Index, OwnedSlab, Ref, RefMut, SlabViewWithSimpleRef, SlabWithSimpleRefs};

/// Knuth order of the B-Trees.
///
/// Must be at least 4.
pub const M: usize = 8;

/// A map based on a B-Tree.
///
/// This offers an alternative over the standard implementation of B-Trees where nodes are
/// allocated in a contiguous array of [`Node`]s, reducing the cost of tree nodes allocations.
/// In addition the crate provides advanced functions to iterate through and update the map
/// efficiently.
///
/// # Basic usage
///
/// Basic usage is similar to the map data structures offered by the standard library.
/// ```
/// use btree_store::slab::BTreeMap;
///
/// // type inference lets us omit an explicit type signature (which
/// // would be `BTreeMap<&str, &str>` in this example).
/// let mut movie_reviews = BTreeMap::new();
///
/// // review some movies.
/// movie_reviews.insert("Office Space",       "Deals with real issues in the workplace.");
/// movie_reviews.insert("Pulp Fiction",       "Masterpiece.");
/// movie_reviews.insert("The Godfather",      "Very enjoyable.");
/// movie_reviews.insert("The Blues Brothers", "Eye lyked it a lot.");
///
/// // check for a specific one.
/// if !movie_reviews.contains_key("Les Misérables") {
///     println!("We've got {} reviews, but Les Misérables ain't one.",
///              movie_reviews.len());
/// }
///
/// // oops, this review has a lot of spelling mistakes, let's delete it.
/// movie_reviews.remove("The Blues Brothers");
///
/// // look up the values associated with some keys.
/// let to_find = ["Up!", "Office Space"];
/// for movie in &to_find {
///     match movie_reviews.get(movie) {
///        Some(review) => println!("{}: {}", movie, review),
///        None => println!("{} is unreviewed.", movie)
///     }
/// }
///
/// // Look up the value for a key (will panic if the key is not found).
/// println!("Movie review: {}", movie_reviews["Office Space"]);
///
/// // iterate over everything.
/// for (movie, review) in movie_reviews.iter().map(|kv| kv.into_pair()) {
///     println!("{}: \"{}\"", movie, review);
/// }
/// ```
///
/// # Advanced usage
///
/// ## Entry API
///
/// This crate also reproduces the Entry API defined by the standard library,
/// which allows for more complex methods of getting, setting, updating and removing keys and
/// their values:
/// ```
/// use btree_store::slab::BTreeMap;
///
/// // type inference lets us omit an explicit type signature (which
/// // would be `BTreeMap<&str, u8>` in this example).
/// let mut player_stats: BTreeMap<&str, u8> = BTreeMap::new();
///
/// fn random_stat_buff() -> u8 {
///     // could actually return some random value here - let's just return
///     // some fixed value for now
///     42
/// }
///
/// // insert a key only if it doesn't already exist
/// player_stats.entry("health").or_insert(100);
///
/// // insert a key using a function that provides a new value only if it
/// // doesn't already exist
/// player_stats.entry("defence").or_insert_with(random_stat_buff);
///
/// // update a key, guarding against the key possibly not being set
/// let stat = player_stats.entry("attack").or_insert(100);
/// *stat += random_stat_buff();
/// ```
///
/// ## Mutable iterators
///
/// This type provides two iterators providing mutable references to the entries:
///   - [`IterMut`] is a double-ended iterator following the standard
///     [`std::collections::btree_map::IterMut`] implementation.
///   - [`EntriesMut`] is a single-ended iterator that allows, in addition,
///     insertion and deletion of entries at the current iterator's position in the map.
///     An example is given below.
///
/// ```
/// use btree_store::slab::BTreeMap;
///
/// let mut map = BTreeMap::new();
/// map.insert("a", 1);
/// map.insert("b", 2);
/// map.insert("d", 4);
///
/// let mut entries = map.entries_mut();
/// entries.next();
/// entries.next();
/// entries.insert("c", 3); // the inserted key must preserve the order of the map.
///
/// let entries: Vec<_> = map.into_iter().collect();
/// assert_eq!(entries, vec![("a", 1), ("b", 2), ("c", 3), ("d", 4)]);
/// ```
///
/// ## Custom allocation
///
/// This data structure is built on top of a slab data structure,
/// but is agnostic of the actual slab implementation which is taken as parameter (`C`).
/// If the `slab` feature is enabled,
/// the [`slab::Slab`] implementation is used by default by reexporting
/// `BTreeMap<K, V, slab::Slab<_>>` at the root of the crate.
/// Any container implementing "slab-like" functionalities can be used.
///
/// You can also pass an existing allocator. For instance, if you want to store multiple maps in the
/// same slab, you can use [`shareable_slab::BTreeMap`]s and pass them each a reference to the same
/// [`shareable_slab::ShareableSlab`]
///
/// ```
/// #![cfg(feature = "shareable-slab")]
/// use btree_store::shareable_slab::{ShareableSlab, BTreeMap};
///
/// // create a shareable slab
/// let slab = ShareableSlab::new();
///
/// // create 2 maps in our shareable slab
/// let mut movie_reviews = BTreeMap::new_in(&slab);
/// let mut book_reviews = BTreeMap::new_in(&slab);
///
/// // review some movies and books.
/// movie_reviews.insert("Office Space", "Deals with real issues in the workplace.");
/// book_reviews.insert("Hamlet", "Great book");
/// movie_reviews.insert("Pulp Fiction", "Masterpiece.");
/// movie_reviews.insert("The Godfather", "Very enjoyable.");
/// book_reviews.insert("Percy Jackson and the Lightning Thief", "Great book");
/// movie_reviews.insert("Hunger Games", "Better than the book");
/// book_reviews.insert("Hunger Games", "Better than the movie");
/// book_reviews.insert("Introduction to Python", "Good");
/// book_reviews.insert("Introduction to Java", "Better");
/// book_reviews.insert("Introduction to Rust", "Best");
/// movie_reviews.insert("The Blues Brothers", "Eye lyked it a lot.");
///
/// // check for a specific one.
/// if !movie_reviews.contains_key("Les Misérables") {
///     println!("We've got {} reviews, but Les Misérables ain't one.",
///             movie_reviews.len());
/// }
/// if !book_reviews.contains_key("Introduction to COBOL") {
///     println!("We've got {} reviews, but Introduction to COBOL ain't one.",
///             book_reviews.len());
/// }
///
/// // oops, this review has a lot of spelling mistakes, let's delete it.
/// movie_reviews.remove("The Blues Brothers").expect("This review should exist");
///
/// // delete some book reviews
/// book_reviews.remove("Introduction to Java").unwrap();
/// book_reviews.remove("Percy Jackson and the Lightning Thief").unwrap();
/// let None = book_reviews.remove("Percy Jackson and the Lightning Thief") else {
///     panic!("This review was already removed")
/// };
///
/// // look up the values associated with some keys.
/// let to_find = ["Up!", "Office Space", "Hamlet", "Hunger Games"];
/// for book_or_movie in &to_find {
///     match (movie_reviews.get(book_or_movie), book_reviews.get(book_or_movie)) {
///         (Some(movie_review), Some(book_review)) => {
///             println!("{}: {} (movie), {} (book)", book_or_movie, movie_review, book_review)
///         }
///         (Some(movie_review), None) => {
///             println!("{}: {} (movie only)", book_or_movie, movie_review)
///         }
///         (None, Some(book_review)) => {
///             println!("{}: {} (book only)", book_or_movie, book_review)
///         }
///         (None, None) => println!("{} (no reviews).", book_or_movie)
///     }
/// }
///
/// // Look up the value for a key (will panic if the key is not found).
/// // Can't do that because these are [std::cell::Ref]s!
/// // println!("Movie review: {}", movie_reviews["Office Space"]);
/// // println!("Book review: {}", book_reviews["Introduction to Rust"]);
///
/// // iterate over and consume everything.
/// println!("Movie reviews:");
/// for (movie, review) in movie_reviews {
///     println!("  {}: \"{}\"", movie, review);
/// }
/// println!("Book reviews:");
/// for (movie, review) in book_reviews {
///     println!("  {}: \"{}\"", movie, review);
/// }
/// ```
///
/// ## Extended API
///
/// This crate provides the two traits [`BTreeExt`] and [`BTreeExtMut`] that can be imported to
/// expose low-level operations on [`BTreeMap`].
/// The extended API allows the caller to directly navigate and access the entries of the tree
/// using their [`Address`].
/// These functions are not intended to be directly called by the users,
/// but can be used to extend the data structure with new functionalities.
///
/// # Correctness
///
/// It is a logic error for a key to be modified in such a way that the key's ordering relative
/// to any other key, as determined by the [`Ord`] trait, changes while it is in the map.
/// This is normally only possible through [`Cell`](`std::cell::Cell`),
/// [`RefCell`](`std::cell::RefCell`), global state, I/O, or unsafe code.
#[derive(Clone)]
pub struct BTreeMap<K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> {
	/// Contains allocated nodes. May contain other data as well, as long as it preserves the node
	/// indices.
	store: C,
	/// Root node id.
	root: Option<I>,
	/// Number of items in the tree.
	len: usize,
	k: PhantomData<K>,
	v: PhantomData<V>,
}

/// `Deref`-able pointer to a key in a [BTreeMap]
pub type KeyRef<'a, K, V, I, C> = <<C as SlabView<Node<K, V, I>>>::Ref<'a, Node<K, V, I>> as Ref<'a, Node<K, V, I>>>::Mapped<K>;
/// `Deref`-able pointer to a value in a [BTreeMap]
pub type ValueRef<'a, K, V, I, C> = <<C as SlabView<Node<K, V, I>>>::Ref<'a, Node<K, V, I>> as Ref<'a, Node<K, V, I>>>::Mapped<V>;
/// `DerefMut`-able pointer to a value in a [BTreeMap]
pub type ValueMut<'a, K, V, I, C> = <<C as Slab<Node<K, V, I>>>::RefMut<'a, Node<K, V, I>> as RefMut<'a, Node<K, V, I>>>::Mapped<V>;
/// `Deref`-able pointer to an item (key and value, like an entry but different) in a [BTreeMap]
pub type ItemRef<'a, K, V, I, C> = <<C as SlabView<Node<K, V, I>>>::Ref<'a, Node<K, V, I>> as Ref<'a, Node<K, V, I>>>::Mapped<Item<K, V>>;
/// `DerefMut`-able pointer to an item (key and value, like an entry but different) in a [BTreeMap]
pub type ItemMut<'a, K, V, I, C> = <<C as Slab<Node<K, V, I>>>::RefMut<'a, Node<K, V, I>> as RefMut<'a, Node<K, V, I>>>::Mapped<Item<K, V>>;

/// Stores a shared reference to a key and value
pub struct KeyValueRef<'a, K: 'a, V: 'a, I: Index + 'a, C: SlabView<Node<K, V, I>, Index=I> + 'a>(
	ItemRef<'a, K, V, I, C>
);


/// Stores a shared reference to a key and mutable reference to a value
pub struct KeyRefValueMut<'a, K: 'a, V: 'a, I: Index + 'a, C: Slab<Node<K, V, I>, Index=I> + 'a>(
	ItemMut<'a, K, V, I, C>
);

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> KeyValueRef<'a, K, V, I, C> {
	#[inline]
	pub fn key(&self) -> &K {
		self.0.key()
	}

	#[inline]
	pub fn value(&self) -> &V {
		self.0.value()
	}

	#[inline]
	pub fn as_pair(&self) -> (&K, &V) {
		self.0.as_pair()
	}

	#[inline]
	pub fn into_key_ref(self) -> KeyRef<'a, K, V, I, C> {
		C::Ref::<'a, Node<K, V, I>>::cast_map_transitive::<Item<K, V>, K>(self.0.map(|i| i.key()))
	}

	#[inline]
	pub fn into_value_ref(self) -> ValueRef<'a, K, V, I, C> {
		C::Ref::<'a, Node<K, V, I>>::cast_map_transitive::<Item<K, V>, V>(self.0.map(|i| i.value()))
	}
}

impl<'a, K, V, I: Index, C: SlabViewWithSimpleRef<Node<K, V, I>, Index=I>> KeyValueRef<'a, K, V, I, C> {
	#[inline]
	pub fn into_pair(self) -> (&'a K, &'a V) {
		C::convert_mapped_into_simple_ref(self.0).as_pair()
	}
}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> KeyRefValueMut<'a, K, V, I, C> {
	#[inline]
	pub fn key(&self) -> &K {
		self.0.key()
	}

	#[inline]
	pub fn value(&self) -> &V {
		self.0.value()
	}

	#[inline]
	pub fn value_mut(&mut self) -> &mut V {
		self.0.value_mut()
	}

	#[inline]
	pub fn as_pair(&self) -> (&K, &V) {
		self.0.as_pair()
	}

	#[inline]
	pub fn as_pair_mut(&mut self) -> (&K, &mut V) {
		let (key, value) = self.0.as_pair_mut();
		(key as &K, value)
	}

	#[inline]
	pub fn into_value_mut(self) -> ValueMut<'a, K, V, I, C> {
		C::RefMut::<'a, Node<K, V, I>>::cast_map_transitive::<Item<K, V>, V>(self.0.map(|i| i.value_mut()))
	}
}

impl<'a, K, V, I: Index, C: SlabWithSimpleRefs<Node<K, V, I>, Index=I>> KeyRefValueMut<'a, K, V, I, C> {
	#[inline]
	pub fn into_pair(self) -> (&'a K, &'a mut V) {
		let (key, value) = C::convert_mapped_into_simple_mut(self.0).as_pair_mut();
		(key as &K, value)
	}
}

impl<K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I> + Default> BTreeMap<K, V, I, C> {
	/// Create a new empty B-tree with a new store.
	#[inline]
	pub fn new() -> BTreeMap<K, V, I, C> {
		BTreeMap {
			store: Default::default(),
			root: None,
			len: 0,
			k: PhantomData,
			v: PhantomData,
		}
	}
}

impl<K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> BTreeMap<K, V, I, C> {
	/// Create a new empty B-tree with a pre-existing store.
	#[inline]
	pub fn new_in(store: C) -> BTreeMap<K, V, I, C> {
		BTreeMap {
			store,
			root: None,
			len: 0,
			k: PhantomData,
			v: PhantomData,
		}
	}

	/// Returns `true` if the map contains no elements.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut a = BTreeMap::new();
	/// assert!(a.is_empty());
	/// a.insert(1, "a");
	/// assert!(!a.is_empty());
	/// ```
	#[inline]
	pub fn is_empty(&self) -> bool {
		self.root.is_none()
	}

	/// Returns the number of elements in the map.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut a = BTreeMap::new();
	/// assert_eq!(a.len(), 0);
	/// a.insert(1, "a");
	/// assert_eq!(a.len(), 1);
	/// ```
	#[inline]
	pub fn len(&self) -> usize {
		self.len
	}

	/// Destroy this and return the store. Any elements will *not* be cleared, which will "leak"
	/// as long as the store is still alive.
	///
	/// # Example
	///
	/// ```
	/// use slab::Slab;
	/// use btree_store::slab::BTreeMap;
	///
	/// let store = Slab::new();
	/// assert_eq!(store.len(), 0);
	/// assert_eq!(store.capacity(), 0);
	///
	/// let mut a = BTreeMap::new_in(store);
	/// a.insert(1, "a");
	/// a.insert(2, "b");
	/// a.insert(3, "c");
	/// let store = a.into_store_dont_clear();
	///
	/// // We didn't remove any elements. Note that the b-trees don't allocate for each element
	/// // and N > 3, so there's only 1 item in the store
	/// assert!(store.len() > 0);
	/// ```
	#[inline]
	pub fn into_store_dont_clear(self) -> C {
		self.store
	}
}

impl<K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> BTreeMap<K, V, I, C> {
	/// Returns the value corresponding to the supplied key.
	///
	/// The supplied key may be any borrowed form of the map's key type, but the ordering
	/// on the borrowed form *must* match the ordering on the key type.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map: BTreeMap<i32, &str> = BTreeMap::new();
	/// map.insert(1, "a");
	/// assert_eq!(map.get(&1), Some(&"a"));
	/// assert_eq!(map.get(&2), None);
	/// ```
	#[inline]
	pub fn get<Q: Ord + ?Sized>(&self, key: &Q) -> Option<ValueRef<'_, K, V, I, C>> where K: Borrow<Q> {
		match self.root {
			Some(id) => self.get_in(key, id),
			None => None,
		}
	}

	/// Returns the key-value pair corresponding to the supplied key.
	///
	/// The supplied key may be any borrowed form of the map's key type, but the ordering
	/// on the borrowed form *must* match the ordering on the key type.
	///
	/// # Examples
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// map.insert(1, "a");
	/// assert_eq!(map.get_key_value(&1).map(|kv| kv.into_pair()), Some((&1, &"a")));
	/// assert_eq!(map.get_key_value(&2).map(|kv| kv.into_pair()), None);
	/// ```
	#[inline]
	pub fn get_key_value<Q: Ord + ?Sized>(&self, k: &Q) -> Option<KeyValueRef<'_, K, V, I, C>> where K: Borrow<Q> {
		match self.address_of(k) {
			Ok(addr) => {
				let item = self.item(addr).unwrap();
				Some(KeyValueRef(item))
			}
			Err(_) => None,
		}
	}

	/// Returns the first key-value pair in the map.
	/// The key in this pair is the minimum key in the map.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// assert_eq!(map.first_key_value().map(|kv| kv.into_pair()), None);
	/// map.insert(1, "b");
	/// map.insert(2, "a");
	/// assert_eq!(map.first_key_value().map(|kv| kv.into_pair()), Some((&1, &"b")));
	/// ```
	#[inline]
	pub fn first_key_value(&self) -> Option<KeyValueRef<'_, K, V, I, C>> {
		match self.first_item_address() {
			Some(addr) => {
				let item = self.item(addr).unwrap();
				Some(KeyValueRef(item))
			}
			None => None,
		}
	}

	/// Returns the last key-value pair in the map.
	/// The key in this pair is the maximum key in the map.
	///
	/// # Examples
	///
	/// Basic usage:
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// map.insert(1, "b");
	/// map.insert(2, "a");
	/// assert_eq!(map.last_key_value().map(|kv| kv.into_pair()), Some((&2, &"a")));
	/// ```
	#[inline]
	pub fn last_key_value(&self) -> Option<KeyValueRef<'_, K, V, I, C>> {
		match self.last_item_address() {
			Some(addr) => {
				let item = self.item(addr).unwrap();
				Some(KeyValueRef(item))
			}
			None => None,
		}
	}

	/// Gets an iterator over the entries of the map, sorted by key.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// map.insert(3, "c");
	/// map.insert(2, "b");
	/// map.insert(1, "a");
	///
	/// for (key, value) in map.iter().map(|kv| kv.into_pair()) {
	///     println!("{}: {}", key, value);
	/// }
	///
	/// let (first_key, first_value) = map.iter().next().unwrap().into_pair();
	/// assert_eq!((*first_key, *first_value), (1, "a"));
	/// ```
	#[inline]
	pub fn iter(&self) -> Iter<K, V, I, C> {
		Iter::new(self)
	}

	/// Gets an iterator over the keys of the map, in sorted order.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut a = BTreeMap::new();
	/// a.insert(2, "b");
	/// a.insert(1, "a");
	///
	/// let keys: Vec<_> = a.keys().cloned().collect();
	/// assert_eq!(keys, [1, 2]);
	/// ```
	#[inline]
	pub fn keys(&self) -> Keys<K, V, I, C> {
		Keys { inner: self.iter() }
	}

	/// Gets an iterator over the values of the map, in order by key.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut a = BTreeMap::new();
	/// a.insert(1, "hello");
	/// a.insert(2, "goodbye");
	///
	/// let values: Vec<&str> = a.values().cloned().collect();
	/// assert_eq!(values, ["hello", "goodbye"]);
	/// ```
	#[inline]
	pub fn values(&self) -> Values<K, V, I, C> {
		Values { inner: self.iter() }
	}

	/// Constructs a double-ended iterator over a sub-range of elements in the map.
	/// The simplest way is to use the range syntax `min..max`, thus `range(min..max)` will
	/// yield elements from min (inclusive) to max (exclusive).
	/// The range may also be entered as `(Bound<T>, Bound<T>)`, so for example
	/// `range((Excluded(4), Included(10)))` will yield a left-exclusive, right-inclusive
	/// range from 4 to 10.
	///
	/// # Panics
	///
	/// Panics if range `start > end`.
	/// Panics if range `start == end` and both bounds are `Excluded`.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	/// use std::ops::Bound::Included;
	///
	/// let mut map = BTreeMap::new();
	/// map.insert(3, "a");
	/// map.insert(5, "b");
	/// map.insert(8, "c");
	/// for key_value in map.range((Included(&4), Included(&8))) {
	///     println!("{}: {}", key_value.key(), key_value.value());
	/// }
	/// assert_eq!(Some((&5, &"b")), map.range(4..).next().map(|e| e.into_pair()));
	/// ```
	#[inline]
	pub fn range<T: Ord + ?Sized>(
		&self,
		range: impl RangeBounds<T>
	) -> Range<K, V, I, C> where K: Borrow<T> {
		Range::new(self, range)
	}

	/// Returns `true` if the map contains a value for the specified key.
	///
	/// The key may be any borrowed form of the map's key type, but the ordering
	/// on the borrowed form *must* match the ordering on the key type.
	///
	/// # Example
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map: BTreeMap<i32, &str> = BTreeMap::new();
	/// map.insert(1, "a");
	/// assert_eq!(map.contains_key(&1), true);
	/// assert_eq!(map.contains_key(&2), false);
	/// ```
	#[inline]
	pub fn contains_key<Q: Ord + ?Sized>(&self, key: &Q) -> bool where K: Borrow<Q> {
		self.get(key).is_some()
	}
}

impl<K: std::fmt::Display, V: std::fmt::Display, I: Index + std::fmt::Display, C: SlabView<Node<K, V, I>, Index=I>> BTreeMap<K, V, I, C> {
	/// Write the tree in the DOT graph descrption language.
	///
	/// Requires the `dot` feature.
	#[cfg(any(doc, feature = "dot"))]
	#[inline]
	pub fn dot_write(&self, f: &mut impl std::io::Write) -> std::io::Result<()> {
		write!(f, "digraph tree {{\n\tnode [shape=record];\n")?;
		if let Some(id) = self.root {
			self.dot_write_node(f, id)?
		}
		write!(f, "}}")
	}

	/// Write the given node in the DOT graph descrption language.
	///
	/// Requires the `dot` feature.
	#[cfg(any(doc, feature = "dot"))]
	#[inline]
	fn dot_write_node(&self, f: &mut impl std::io::Write, id: I) -> std::io::Result<()> {
		let name = format!("n{}", id);
		let node = self.node(id);

		write!(f, "\t{} [label=\"", name)?;
		if let Some(parent) = node.parent() {
			write!(f, "({})|", parent)?;
		}

		node.dot_write_label(f)?;
		writeln!(f, "({})\"];", id)?;

		for child_id in node.children() {
			self.dot_write_node(f, child_id)?;
			let child_name = format!("n{}", child_id);
			writeln!(f, "\t{} -> {}", name, child_name)?;
		}

		Ok(())
	}
}

impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> BTreeMap<K, V, I, C> {
	/// Returns a mutable reference to the value corresponding to the supplied key.
	///
	/// The supplied key may be any borrowed form of the map's key type, but the ordering
	/// on the borrowed form *must* match the ordering on the key type.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map: BTreeMap<i32, &str> = BTreeMap::new();
	/// map.insert(1, "a");
	/// assert_eq!(map.get_mut(&1).copied(), Some("a"));
	/// *map.get_mut(&1).unwrap() = "b";
	/// assert_eq!(map.get_mut(&1).copied(), Some("b"));
	/// assert_eq!(map.get_mut(&2), None);
	/// ```
	#[inline]
	pub fn get_mut<Q: Ord + ?Sized>(&mut self, key: &Q) -> Option<ValueMut<'_, K, V, I, C>> where K: Borrow<Q> {
		match self.root {
			Some(id) => self.get_mut_in(key, id),
			None => None,
		}
	}

	/// Returns the key-value pair corresponding to the supplied key, with a mutable reference to
	/// the value.
	///
	/// The supplied key may be any borrowed form of the map's key type, but the ordering
	/// on the borrowed form *must* match the ordering on the key type.
	///
	/// # Examples
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// map.insert(1, "a");
	/// assert_eq!(map.get_key_value_mut(&1).map(|kv| kv.into_pair()).map(|(k, v)| (*k, *v)), Some((1, "a")));
	/// *map.get_key_value_mut(&1).unwrap().value_mut() = "b";
	/// assert_eq!(map.get_key_value_mut(&1).map(|kv| kv.into_pair()).map(|(k, v)| (*k, *v)), Some((1, "b")));
	/// assert!(map.get_key_value_mut(&2).is_none());
	/// ```
	#[inline]
	pub fn get_key_value_mut<Q: Ord + ?Sized>(&mut self, k: &Q) -> Option<KeyRefValueMut<'_, K, V, I, C>> where K: Borrow<Q> {
		match self.address_of(k) {
			Ok(addr) => {
				let item = self.item_mut(addr).unwrap();
				Some(KeyRefValueMut(item))
			}
			Err(_) => None,
		}
	}

	/// Returns the first key-value pair in the map, with a mutable reference to the value.
	/// The key in this pair is the minimum key in the map.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// assert_eq!(map.first_key_value_mut().map(|kv| kv.into_pair()).map(|(k, v)| (*k, *v)), None);
	/// map.insert(1, "b");
	/// map.insert(2, "a");
	/// assert_eq!(map.first_key_value_mut().map(|kv| kv.into_pair()).map(|(k, v)| (*k, *v)), Some((1, "b")));
	/// *map.first_key_value_mut().unwrap().value_mut() = "c";
	/// assert_eq!(map.first_key_value_mut().map(|kv| kv.into_pair()).map(|(k, v)| (*k, *v)), Some((1, "c")));
	/// ```
	#[inline]
	pub fn first_key_value_mut(&mut self) -> Option<KeyRefValueMut<'_, K, V, I, C>> {
		match self.first_item_address() {
			Some(addr) => {
				let item = self.item_mut(addr).unwrap();
				Some(KeyRefValueMut(item))
			}
			None => None,
		}
	}

	/// Returns the last key-value pair in the map, with a mutable reference to the value
	/// The key in this pair is the maximum key in the map.
	///
	/// # Examples
	///
	/// Basic usage:
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// map.insert(1, "b");
	/// map.insert(2, "a");
	/// assert_eq!(map.last_key_value_mut().map(|kv| kv.into_pair()).map(|(k, v)| (*k, *v)), Some((2, "a")));
	/// *map.last_key_value_mut().unwrap().value_mut() = "c";
	/// assert_eq!(map.last_key_value_mut().map(|kv| kv.into_pair()).map(|(k, v)| (*k, *v)), Some((2, "c")));
	/// ```
	#[inline]
	pub fn last_key_value_mut(&mut self) -> Option<KeyRefValueMut<'_, K, V, I, C>> {
		match self.last_item_address() {
			Some(addr) => {
				let item = self.item_mut(addr).unwrap();
				Some(KeyRefValueMut(item))
			}
			None => None,
		}
	}

	/// Clears the map, removing all elements.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut a = BTreeMap::new();
	/// a.insert(1, "a");
	/// a.clear();
	/// assert!(a.is_empty());
	/// ```
	#[inline]
	pub fn clear(&mut self) {
		if !self.store.clear_fast() {
			// Remove each item individually
			self.retain(|_, _| false);
		}
		self.root = None;
		self.len = 0;
	}

	/// Clear this, then destroy and return the store
	///
	/// # Example
	///
	/// ```
	/// use slab::Slab;
	/// use btree_store::slab::BTreeMap;
	///
	/// let store = Slab::new();
	/// assert_eq!(store.len(), 0);
	/// assert_eq!(store.capacity(), 0);
	///
	/// let mut a = BTreeMap::new_in(store);
	/// a.insert(1, "a");
	/// a.insert(2, "b");
	/// a.insert(3, "c");
	/// let store = a.into_store_do_clear();
	///
	/// // We removed all elements
	/// assert_eq!(store.len(), 0);
	///
	/// // But we didn't remove the allocation
	/// assert!(store.capacity() > 0);
	/// ```
	#[inline]
	pub fn into_store_do_clear(mut self) -> C {
		self.clear();
		self.into_store_dont_clear()
	}

	/// Returns the first entry in the map for in-place manipulation.
	/// The key of this entry is the minimum key in the map.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// map.insert(1, "a");
	/// map.insert(2, "b");
	/// if let Some(mut entry) = map.first_entry() {
	///     if *entry.key() > 0 {
	///         entry.insert("first");
	///     }
	/// }
	/// assert_eq!(*map.get(&1).unwrap(), "first");
	/// assert_eq!(*map.get(&2).unwrap(), "b");
	/// ```
	#[inline]
	pub fn first_entry(&mut self) -> Option<OccupiedEntry<K, V, I, C>> {
		self.first_item_address()
			.map(move |addr| OccupiedEntry { map: self, addr })
	}

	/// Returns the last entry in the map for in-place manipulation.
	/// The key of this entry is the maximum key in the map.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// map.insert(1, "a");
	/// map.insert(2, "b");
	/// if let Some(mut entry) = map.last_entry() {
	///     if *entry.key() > 0 {
	///         entry.insert("last");
	///     }
	/// }
	/// assert_eq!(*map.get(&1).unwrap(), "a");
	/// assert_eq!(*map.get(&2).unwrap(), "last");
	/// ```
	#[inline]
	pub fn last_entry(&mut self) -> Option<OccupiedEntry<K, V, I, C>> {
		self.last_item_address()
			.map(move |addr| OccupiedEntry { map: self, addr })
	}

	/// Removes and returns the first element in the map.
	/// The key of this element is the minimum key that was in the map.
	///
	/// # Example
	///
	/// Draining elements in ascending order, while keeping a usable map each iteration.
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// map.insert(1, "a");
	/// map.insert(2, "b");
	/// while let Some((key, _val)) = map.pop_first() {
	///     assert!(map.iter().all(|kv| *kv.key() > key));
	/// }
	/// assert!(map.is_empty());
	/// ```
	#[inline]
	pub fn pop_first(&mut self) -> Option<(K, V)> {
		self.first_entry().map(|entry| entry.remove_entry())
	}

	/// Removes and returns the last element in the map.
	/// The key of this element is the maximum key that was in the map.
	///
	/// # Example
	///
	/// Draining elements in descending order, while keeping a usable map each iteration.
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// map.insert(1, "a");
	/// map.insert(2, "b");
	/// while let Some((key, _val)) = map.pop_last() {
	///     assert!(map.iter().all(|kv| *kv.key() < key));
	/// }
	/// assert!(map.is_empty());
	/// ```
	#[inline]
	pub fn pop_last(&mut self) -> Option<(K, V)> {
		self.last_entry().map(|entry| entry.remove_entry())
	}

	/// Removes a key from the map, returning the value at the key if the key
	/// was previously in the map.
	///
	/// The key may be any borrowed form of the map's key type, but the ordering
	/// on the borrowed form *must* match the ordering on the key type.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// map.insert(1, "a");
	/// assert_eq!(map.remove(&1), Some("a"));
	/// assert_eq!(map.remove(&1), None);
	/// ```
	#[inline]
	pub fn remove<Q: Ord + ?Sized>(&mut self, key: &Q) -> Option<V> where K: Borrow<Q> {
		match self.address_of(key) {
			Ok(addr) => {
				let (item, _) = self.remove_at(addr).unwrap();
				Some(item.into_value())
			}
			Err(_) => None,
		}
	}

	/// Removes a key from the map, returning the stored key and value if the key
	/// was previously in the map.
	///
	/// The key may be any borrowed form of the map's key type, but the ordering
	/// on the borrowed form *must* match the ordering on the key type.
	///
	/// # Example
	///
	/// Basic usage:
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// map.insert(1, "a");
	/// assert_eq!(map.remove_entry(&1), Some((1, "a")));
	/// assert_eq!(map.remove_entry(&1), None);
	/// ```
	#[inline]
	pub fn remove_entry<Q: Ord + ?Sized>(&mut self, key: &Q) -> Option<(K, V)> where K: Borrow<Q> {
		match self.address_of(key) {
			Ok(addr) => {
				let (item, _) = self.remove_at(addr).unwrap();
				Some(item.into_pair())
			}
			Err(_) => None,
		}
	}

	/// Gets a mutable iterator over the entries of the map, sorted by key.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// map.insert("a", 1);
	/// map.insert("b", 2);
	/// map.insert("c", 3);
	///
	/// // add 10 to the value if the key isn't "a"
	/// for (key, value) in map.iter_mut().map(|kv| kv.into_pair()) {
	///     if key != &"a" {
	///         *value += 10;
	///     }
	/// }
	/// ```
	#[inline]
	pub fn iter_mut(&mut self) -> IterMut<K, V, I, C> {
		IterMut::new(self)
	}

	/// Gets a mutable iterator over the entries of the map, sorted by key, that allows insertion and deletion of the iterated entries.
	///
	/// # Correctness
	///
	/// It is safe to insert any key-value pair while iterating,
	/// however this might break the well-formedness
	/// of the underlying tree, which relies on several invariants.
	/// To preserve these invariants,
	/// the inserted key must be *strictly greater* than the previous visited item's key,
	/// and *strictly less* than the next visited item
	/// (which you can retrive through [`EntriesMut::peek`] without moving the iterator).
	/// If this rule is not respected, the data structure will become unusable
	/// (invalidate the specification of every method of the API).
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map = BTreeMap::new();
	/// map.insert("a", 1);
	/// map.insert("b", 2);
	/// map.insert("d", 4);
	///
	/// let mut entries = map.entries_mut();
	/// entries.next();
	/// entries.next();
	/// entries.insert("c", 3);
	///
	/// let entries: Vec<_> = map.into_iter().collect();
	/// assert_eq!(entries, vec![("a", 1), ("b", 2), ("c", 3), ("d", 4)]);
	/// ```
	#[inline]
	pub fn entries_mut(&mut self) -> EntriesMut<K, V, I, C> {
		EntriesMut::new(self)
	}

	/// Constructs a mutable double-ended iterator over a sub-range of elements in the map.
	/// The simplest way is to use the range syntax `min..max`, thus `range(min..max)` will
	/// yield elements from min (inclusive) to max (exclusive).
	/// The range may also be entered as `(Bound<T>, Bound<T>)`, so for example
	/// `range((Excluded(4), Included(10)))` will yield a left-exclusive, right-inclusive
	/// range from 4 to 10.
	///
	/// # Panics
	///
	/// Panics if range `start > end`.
	/// Panics if range `start == end` and both bounds are `Excluded`.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map: BTreeMap<&str, i32> = ["Alice", "Bob", "Carol", "Cheryl"]
	///     .iter()
	///     .map(|&s| (s, 0))
	///     .collect();
	/// for mut key_value in map.range_mut("B".."Cheryl") {
	///     *key_value.value_mut() += 100;
	/// }
	/// for name_balance in &map {
	///     let name = name_balance.key();
	///     let balance = name_balance.value();
	///     println!("{} => {}", name, balance);
	/// }
	/// ```
	#[inline]
	pub fn range_mut<T: Ord + ?Sized>(
		&mut self,
		range: impl RangeBounds<T>
	) -> RangeMut<K, V, I, C> where K: Borrow<T> {
		RangeMut::new(self, range)
	}

	/// Gets a mutable iterator over the values of the map, in order by key.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut a = BTreeMap::new();
	/// a.insert(1, String::from("hello"));
	/// a.insert(2, String::from("goodbye"));
	///
	/// for value in a.values_mut() {
	///     value.push_str("!");
	/// }
	///
	/// let values: Vec<String> = a.values().cloned().collect();
	/// assert_eq!(values, [String::from("hello!"),
	///                     String::from("goodbye!")]);
	/// ```
	#[inline]
	pub fn values_mut(&mut self) -> ValuesMut<K, V, I, C> {
		ValuesMut {
			inner: self.iter_mut(),
		}
	}

	/// Creates an iterator which uses a closure to determine if an element should be removed.
	///
	/// If the closure returns true, the element is removed from the map and yielded.
	/// If the closure returns false, or panics, the element remains in the map and will not be
	/// yielded.
	///
	/// Note that `drain_filter` lets you mutate every value in the filter closure, regardless of
	/// whether you choose to keep or remove it.
	///
	/// If the iterator is only partially consumed or not consumed at all, each of the remaining
	/// elements will still be subjected to the closure and removed and dropped if it returns true.
	///
	/// It is unspecified how many more elements will be subjected to the closure
	/// if a panic occurs in the closure, or a panic occurs while dropping an element,
	/// or if the `DrainFilter` value is leaked.
	///
	/// # Example
	///
	/// Splitting a map into even and odd keys, reusing the original map:
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map: BTreeMap<i32, i32> = (0..8).map(|x| (x, x)).collect();
	/// let evens: BTreeMap<_, _> = map.drain_filter(|k, _v| k % 2 == 0).collect();
	/// let odds = map;
	/// assert_eq!(evens.keys().copied().collect::<Vec<_>>(), vec![0, 2, 4, 6]);
	/// assert_eq!(odds.keys().copied().collect::<Vec<_>>(), vec![1, 3, 5, 7]);
	/// ```
	#[inline]
	pub fn drain_filter<F: FnMut(&K, &mut V) -> bool>(
		&mut self,
		pred: F
	) -> DrainFilter<K, V, I, C, F> {
		DrainFilter::new(self, pred)
	}

	/// Retains only the elements specified by the predicate.
	///
	/// In other words, remove all pairs `(k, v)` such that `f(&k, &mut v)` returns `false`.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut map: BTreeMap<i32, i32> = (0..8).map(|x| (x, x*10)).collect();
	/// // Keep only the elements with even-numbered keys.
	/// map.retain(|&k, _| k % 2 == 0);
	/// assert!(map.into_iter().eq(vec![(0, 0), (2, 20), (4, 40), (6, 60)]));
	/// ```
	#[inline]
	pub fn retain(&mut self, mut f: impl FnMut(&K, &mut V) -> bool) {
		self.drain_filter(|k, v| !f(k, v));
	}

	/// Creates a consuming iterator visiting all the keys, in sorted order.
	/// The map cannot be used after calling this.
	/// The iterator element type is `K`.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut a = BTreeMap::new();
	/// a.insert(2, "b");
	/// a.insert(1, "a");
	///
	/// let keys: Vec<i32> = a.into_keys().collect();
	/// assert_eq!(keys, [1, 2]);
	/// ```
	#[inline]
	pub fn into_keys(self) -> IntoKeys<K, V, I, C> {
		IntoKeys {
			inner: self.into_iter(),
		}
	}

	/// Creates a consuming iterator visiting all the values, in order by key.
	/// The map cannot be used after calling this.
	/// The iterator element type is `V`.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut a = BTreeMap::new();
	/// a.insert(1, "hello");
	/// a.insert(2, "goodbye");
	///
	/// let values: Vec<&str> = a.into_values().collect();
	/// assert_eq!(values, ["hello", "goodbye"]);
	/// ```
	#[inline]
	pub fn into_values(self) -> IntoValues<K, V, I, C> {
		IntoValues {
			inner: self.into_iter(),
		}
	}

	/// Try to rotate left the node `id` to benefits the child number `deficient_child_index`.
	///
	/// Returns true if the rotation succeeded, of false if the target child has no right sibling,
	/// or if this sibling would underflow.
	#[inline]
	fn try_rotate_left(
		&mut self,
		id: I,
		deficient_child_index: usize,
		addr: &mut Address<I>,
	) -> bool {
		let pivot_offset = deficient_child_index.into();
		let right_sibling_index = deficient_child_index + 1;
		let (right_sibling_id, deficient_child_id) = {
			let node = self.node(id);

			if right_sibling_index >= node.child_count() {
				return false; // no right sibling
			}

			(
				node.child_id(right_sibling_index),
				node.child_id(deficient_child_index),
			)
		};

		let x = self.node_mut(right_sibling_id).pop_left();
		match x {
			Ok((mut value, opt_child_id)) => {
				std::mem::swap(
					&mut value,
					self.node_mut(id).item_mut(pivot_offset).unwrap(),
				);
				let left_offset = self
					.node_mut(deficient_child_id)
					.push_right(value, opt_child_id);

				// update opt_child's parent
				if let Some(child_id) = opt_child_id {
					self.node_mut(child_id).set_parent(Some(deficient_child_id))
				}

				// update address.
				if addr.id == right_sibling_id {
					// addressed item is in the right node.
					if addr.offset == 0 {
						// addressed item is moving to pivot.
						addr.id = id;
						addr.offset = pivot_offset;
					} else {
						// addressed item stays on right.
						addr.offset.decr();
					}
				} else if addr.id == id {
					// addressed item is in the parent node.
					if addr.offset == pivot_offset {
						// addressed item is the pivot, moving to the left (deficient) node.
						addr.id = deficient_child_id;
						addr.offset = left_offset;
					}
				}

				true // rotation succeeded
			}
			Err(WouldUnderflow) => false, // the right sibling would underflow.
		}
	}

	/// Try to rotate right the node `id` to benefits the child number `deficient_child_index`.
	///
	/// Returns true if the rotation succeeded, of false if the target child has no left sibling,
	/// or if this sibling would underflow.
	#[inline]
	fn try_rotate_right(
		&mut self,
		id: I,
		deficient_child_index: usize,
		addr: &mut Address<I>,
	) -> bool {
		if deficient_child_index > 0 {
			let left_sibling_index = deficient_child_index - 1;
			let pivot_offset = left_sibling_index.into();
			let (left_sibling_id, deficient_child_id) = {
				let node = self.node(id);
				(
					node.child_id(left_sibling_index),
					node.child_id(deficient_child_index),
				)
			};
			let x = self.node_mut(left_sibling_id).pop_right();
			match x {
				Ok((left_offset, mut value, opt_child_id)) => {
					std::mem::swap(
						&mut value,
						self.node_mut(id).item_mut(pivot_offset).unwrap(),
					);
					self.node_mut(deficient_child_id)
						.push_left(value, opt_child_id);

					// update opt_child's parent
					if let Some(child_id) = opt_child_id {
						self.node_mut(child_id).set_parent(Some(deficient_child_id))
					}

					// update address.
					if addr.id == deficient_child_id {
						// addressed item is in the right (deficient) node.
						addr.offset.incr();
					} else if addr.id == left_sibling_id {
						// addressed item is in the left node.
						if addr.offset == left_offset {
							// addressed item is moving to pivot.
							addr.id = id;
							addr.offset = pivot_offset;
						}
					} else if addr.id == id {
						// addressed item is in the parent node.
						if addr.offset == pivot_offset {
							// addressed item is the pivot, moving to the left (deficient) node.
							addr.id = deficient_child_id;
							addr.offset = 0.into();
						}
					}

					true // rotation succeeded
				}
				Err(WouldUnderflow) => false, // the left sibling would underflow.
			}
		} else {
			false // no left sibling.
		}
	}

	/// Merge the child `deficient_child_index` in node `id` with one of its direct sibling.
	#[inline]
	fn merge(
		&mut self,
		id: I,
		deficient_child_index: usize,
		mut addr: Address<I>,
	) -> (Balance, Address<I>) {
		let (offset, left_id, right_id, separator, balance) = if deficient_child_index > 0 {
			// merge with left sibling
			self.node_mut(id)
				.merge(deficient_child_index - 1, deficient_child_index)
		} else {
			// merge with right sibling
			self.node_mut(id)
				.merge(deficient_child_index, deficient_child_index + 1)
		};

		// update children's parent.
		let right_node = self.release_node(right_id);
		for right_child_id in right_node.children() {
			self.node_mut(right_child_id).set_parent(Some(left_id));
		}

		// actually merge.
		let left_offset = self.node_mut(left_id).append(separator, right_node);

		// update addr.
		if addr.id == id {
			match addr.offset.partial_cmp(&offset) {
				Some(Ordering::Equal) => {
					addr.id = left_id;
					addr.offset = left_offset
				}
				Some(Ordering::Greater) => addr.offset.decr(),
				_ => (),
			}
		} else if addr.id == right_id {
			addr.id = left_id;
			addr.offset = (addr.offset.unwrap() + left_offset.unwrap() + 1).into();
		}

		(balance, addr)
	}
}

impl<K: Ord, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> BTreeMap<K, V, I, C> {
	/// Gets the given key's corresponding entry in the map for in-place manipulation.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut letters = BTreeMap::new();
	///
	/// for ch in "a short treatise on fungi".chars() {
	///     let counter = letters.entry(ch).or_insert(0);
	///     *counter += 1;
	/// }
	///
	/// assert_eq!(letters[&'s'], 2);
	/// assert_eq!(letters[&'t'], 3);
	/// assert_eq!(letters[&'u'], 1);
	/// assert_eq!(letters.get(&'y'), None);
	/// ```
	#[inline]
	pub fn entry(&mut self, key: K) -> Entry<K, V, I, C> {
		match self.address_of(&key) {
			Ok(addr) => Entry::Occupied(OccupiedEntry { map: self, addr }),
			Err(addr) => Entry::Vacant(VacantEntry {
				map: self,
				key,
				addr,
			}),
		}
	}

	/// Insert a key-value pair in the tree.
	#[inline]
	pub fn insert(&mut self, key: K, value: V) -> Option<V> {
		match self.address_of(&key) {
			Ok(addr) => Some(self.replace_value_at(addr, value)),
			Err(addr) => {
				self.insert_exactly_at(addr, Item::new(key, value), None);
				None
			}
		}
	}

	/// Replace a key-value pair in the tree.
	#[inline]
	pub fn replace(&mut self, key: K, value: V) -> Option<(K, V)> {
		match self.address_of(&key) {
			Ok(addr) => Some(self.replace_at(addr, key, value)),
			Err(addr) => {
				self.insert_exactly_at(addr, Item::new(key, value), None);
				None
			}
		}
	}

	/// General-purpose update function.
	///
	/// This can be used to insert, compare, replace or remove the value associated to the given
	/// `key` in the tree.
	/// The action to perform is specified by the `action` function.
	/// This function is called once with:
	///  - `Some(value)` when `value` is aready associated to `key` or
	///  - `None` when the `key` is not associated to any value.
	///
	/// The `action` function must return a pair (`new_value`, `result`) where
	/// `new_value` is the new value to be associated to `key`
	/// (if it is `None` any previous binding is removed) and
	/// `result` is the value returned by the entire `update` function call.
	#[inline]
	pub fn update<T>(&mut self, key: K, action: impl FnOnce(Option<V>) -> (Option<V>, T)) -> T {
		match self.root {
			Some(id) => self.update_in(id, key, action),
			None => {
				let (to_insert, result) = action(None);

				if let Some(value) = to_insert {
					let new_root = Node::leaf(None, Item::new(key, value));
					self.root = Some(self.allocate_node(new_root));
					self.len += 1;
				}

				result
			}
		}
	}
}

impl<K: Ord, V, I: Index, C: OwnedSlab<Node<K, V, I>, Index=I> + Default> BTreeMap<K, V, I, C> {
	/// Moves all elements from `other` into `Self`, leaving `other` empty.
	///
	/// # Example
	///
	/// ```
	/// use btree_store::slab::BTreeMap;
	///
	/// let mut a = BTreeMap::new();
	/// a.insert(1, "a");
	/// a.insert(2, "b");
	/// a.insert(3, "c");
	///
	/// let mut b = BTreeMap::new();
	/// b.insert(3, "d");
	/// b.insert(4, "e");
	/// b.insert(5, "f");
	///
	/// a.append1(&mut b);
	///
	/// assert_eq!(a.len(), 5);
	/// assert_eq!(b.len(), 0);
	///
	/// assert_eq!(a[&1], "a");
	/// assert_eq!(a[&2], "b");
	/// assert_eq!(a[&3], "d");
	/// assert_eq!(a[&4], "e");
	/// assert_eq!(a[&5], "f");
	/// ```
	#[inline]
	pub fn append1(&mut self, other: &mut Self) {
		// Do we have to append anything at all?
		if other.is_empty() {
			return;
		}

		// We can just swap `self` and `other` if `self` is empty.
		if self.is_empty() {
			std::mem::swap(self, other);
			return;
		}

		let other = std::mem::take(other);
		for (key, value) in other {
			self.insert(key, value);
		}
	}
}

impl<'a, K: Ord, V, I: Index, C> BTreeMap<K, V, I, &'a C> where &'a C: Slab<Node<K, V, I>, Index=I> {
	/// Asserts both allocators are the same, then we can possibly append smarter (TODO)
	#[inline]
	pub fn append2(&mut self, other: &mut Self) {
		assert!(
			std::ptr::eq(self.store as *const C, other.store as *const C),
			"Cannot append BTreeMaps with different allocators"
		);
		// Do we have to append anything at all?
		if other.is_empty() {
			return;
		}

		// We can just swap `self` and `other` if `self` is empty.
		if self.is_empty() {
			std::mem::swap(self, other);
			return;
		}

		let other = std::mem::replace(other, Self::new_in(self.store));
		for (key, value) in other {
			self.insert(key, value);
		}
	}
}

impl<K: Borrow<Q>, Q: Ord + ?Sized, V, I: Index, C: SlabViewWithSimpleRef<Node<K, V, I>, Index=I>>
	std::ops::Index<&Q> for BTreeMap<K, V, I, C> {
	type Output = V;

	/// Returns a reference to the value corresponding to the supplied key.
	///
	/// # Panics
	///
	/// Panics if the key is not present in the `BTreeMap`.
	#[inline]
	fn index(&self, key: &Q) -> &V {
		C::convert_mapped_into_simple_ref(self.get(key).expect("no entry found for key"))
	}
}

impl<K: Borrow<Q>, Q: Ord + ?Sized, V, I: Index, C: SlabWithSimpleRefs<Node<K, V, I>, Index=I>>
	std::ops::IndexMut<&Q> for BTreeMap<K, V, I, C> {
	/// Returns a reference to the value corresponding to the supplied key.
	///
	/// # Panics
	///
	/// Panics if the key is not present in the `BTreeMap`.
	#[inline]
	fn index_mut(&mut self, key: &Q) -> &mut V {
		C::convert_mapped_into_simple_mut(self.get_mut(key).expect("no entry found for key"))
	}
}

impl<
	K,
	L: PartialEq<K>,
	V,
	W: PartialEq<V>,
	I: Index,
	J: Index + PartialEq<I>,
	C: SlabView<Node<K, V, I>, Index=I>,
	D: SlabView<Node<L, W, J>, Index=J>
> PartialEq<BTreeMap<L, W, J, D>> for BTreeMap<K, V, I, C> {
	fn eq(&self, other: &BTreeMap<L, W, J, D>) -> bool {
		if self.len() == other.len() {
			let mut it1 = self.iter();
			let mut it2 = other.iter();

			loop {
				match (it1.next(), it2.next()) {
					(None, None) => break,
					(Some(kv), Some(lw)) => {
						if lw.key() != kv.key() || lw.value() != kv.value() {
							return false;
						}
					}
					_ => return false,
				}
			}

			true
		} else {
			false
		}
	}
}

impl<K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I> + Default> Default for BTreeMap<K, V, I, C> {
	#[inline]
	fn default() -> Self {
		BTreeMap::new()
	}
}

impl<K: Ord, V, I: Index, C: Slab<Node<K, V, I>, Index=I> + Default> FromIterator<(K, V)> for BTreeMap<K, V, I, C> {
	#[inline]
	fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> BTreeMap<K, V, I, C> {
		let mut map = BTreeMap::new();

		for (key, value) in iter {
			map.insert(key, value);
		}

		map
	}
}

impl<K: Ord, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> Extend<(K, V)> for BTreeMap<K, V, I, C> {
	#[inline]
	fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
		for (key, value) in iter {
			self.insert(key, value);
		}
	}
}

impl<'a, K: Ord + Copy, V: Copy, I: Index, C: Slab<Node<K, V, I>, Index=I>> Extend<(&'a K, &'a V)>
	for BTreeMap<K, V, I, C> {
	#[inline]
	fn extend<T: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: T) {
		self.extend(iter.into_iter().map(|(&key, &value)| (key, value)));
	}
}

impl<K: Eq, V: Eq, I: Index, C: SlabView<Node<K, V, I>, Index=I>> Eq for BTreeMap<K, V, I, C> {}

impl<
	K,
	L: PartialOrd<K>,
	V,
	W: PartialOrd<V>,
	I: Index,
	J: Index + PartialOrd<I>,
	C: SlabView<Node<K, V, I>, Index=I>,
	D: SlabView<Node<L, W, J>, Index=J>
> PartialOrd<BTreeMap<L, W, J, D>> for BTreeMap<K, V, I, C> {
	fn partial_cmp(&self, other: &BTreeMap<L, W, J, D>) -> Option<Ordering> {
		let mut it1 = self.iter();
		let mut it2 = other.iter();

		loop {
			match (it1.next(), it2.next()) {
				(None, None) => return Some(Ordering::Equal),
				(_, None) => return Some(Ordering::Greater),
				(None, _) => return Some(Ordering::Less),
				(Some(kv), Some(lw)) => match lw.key().partial_cmp(kv.key()) {
					Some(Ordering::Greater) => return Some(Ordering::Less),
					Some(Ordering::Less) => return Some(Ordering::Greater),
					Some(Ordering::Equal) => match lw.value().partial_cmp(kv.value()) {
						Some(Ordering::Greater) => return Some(Ordering::Less),
						Some(Ordering::Less) => return Some(Ordering::Greater),
						Some(Ordering::Equal) => (),
						None => return None,
					},
					None => return None,
				},
			}
		}
	}
}

impl<K: Ord, V: Ord, I: Index + Ord, C: SlabView<Node<K, V, I>, Index=I>> Ord for BTreeMap<K, V, I, C> {
	fn cmp(&self, other: &BTreeMap<K, V, I, C>) -> Ordering {
		let mut it1 = self.iter();
		let mut it2 = other.iter();

		loop {
			match (it1.next(), it2.next()) {
				(None, None) => return Ordering::Equal,
				(_, None) => return Ordering::Greater,
				(None, _) => return Ordering::Less,
				(Some(kv), Some(lw)) => match lw.key().cmp(kv.key()) {
					Ordering::Greater => return Ordering::Less,
					Ordering::Less => return Ordering::Greater,
					Ordering::Equal => match lw.value().cmp(kv.value()) {
						Ordering::Greater => return Ordering::Less,
						Ordering::Less => return Ordering::Greater,
						Ordering::Equal => (),
					},
				},
			}
		}
	}
}

impl<K: Hash, V: Hash, I: Index + Hash, C: SlabView<Node<K, V, I>, Index=I>> Hash for BTreeMap<K, V, I, C> {
	#[inline]
	fn hash<H: Hasher>(&self, h: &mut H) {
		for kv in self {
			kv.key().hash(h);
			kv.value().hash(h);
		}
	}
}

pub struct Iter<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> {
	/// The tree reference.
	btree: &'a BTreeMap<K, V, I, C>,
	/// Address of the next item.
	addr: Option<Address<I>>,
	end: Option<Address<I>>,
	len: usize,
}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> Iter<'a, K, V, I, C> {
	#[inline]
	fn new(btree: &'a BTreeMap<K, V, I, C>) -> Self {
		let addr = btree.first_item_address();
		let len = btree.len();
		Iter {
			btree,
			addr,
			end: None,
			len,
		}
	}
}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> Iterator for Iter<'a, K, V, I, C> {
	type Item = KeyValueRef<'a, K, V, I, C>;

	#[inline]
	fn next(&mut self) -> Option<KeyValueRef<'a, K, V, I, C>> {
		match self.addr {
			Some(addr) => {
				if self.len > 0 {
					self.len -= 1;

					let item = self.btree.item(addr).unwrap();
					self.addr = self.btree.next_item_address(addr);
					Some(KeyValueRef(item))
				} else {
					None
				}
			}
			None => None,
		}
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		(self.len, Some(self.len))
	}
}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> FusedIterator for Iter<'a, K, V, I, C> {}
impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> ExactSizeIterator for Iter<'a, K, V, I, C> where {}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> DoubleEndedIterator for Iter<'a, K, V, I, C> {
	#[inline]
	fn next_back(&mut self) -> Option<KeyValueRef<'a, K, V, I, C>> {
		if self.len > 0 {
			let addr = match self.end {
				Some(addr) => self.btree.previous_item_address(addr).unwrap(),
				None => self.btree.last_item_address().unwrap(),
			};

			self.len -= 1;

			let item = self.btree.item(addr).unwrap();
			self.end = Some(addr);
			Some(KeyValueRef(item))
		} else {
			None
		}
	}
}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> IntoIterator for &'a BTreeMap<K, V, I, C> {
	type Item = KeyValueRef<'a, K, V, I, C>;
	type IntoIter = Iter<'a, K, V, I, C>;

	#[inline]
	fn into_iter(self) -> Iter<'a, K, V, I, C> {
		self.iter()
	}
}

pub struct IterMut<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> {
	/// The tree reference.
	btree: &'a mut BTreeMap<K, V, I, C>,
	/// Address of the next item.
	addr: Option<Address<I>>,
	end: Option<Address<I>>,
	len: usize,
}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> IterMut<'a, K, V, I, C> {
	#[inline]
	fn new(btree: &'a mut BTreeMap<K, V, I, C>) -> Self {
		let addr = btree.first_item_address();
		let len = btree.len();
		IterMut {
			btree,
			addr,
			end: None,
			len,
		}
	}

	#[inline]
	fn next_item(&mut self) -> Option<ItemMut<'a, K, V, I, C>> {
		match self.addr {
			Some(addr) => {
				if self.len > 0 {
					self.len -= 1;

					self.addr = self.btree.next_item_address(addr);
					let item = self.btree.item_mut(addr).unwrap();
					Some(unsafe { alter_item_lifetime::<K, V, I, C>(item) }) // this is safe because only one mutable reference to the same item can be emitted.
				} else {
					None
				}
			}
			None => None,
		}
	}

	#[inline]
	fn next_back_item(&mut self) -> Option<ItemMut<'a, K, V, I, C>> {
		if self.len > 0 {
			let addr = match self.end {
				Some(addr) => self.btree.previous_item_address(addr).unwrap(),
				None => self.btree.last_item_address().unwrap(),
			};

			self.len -= 1;

			let item = self.btree.item_mut(addr).unwrap();
			self.end = Some(addr);
			Some(unsafe { alter_item_lifetime::<K, V, I, C>(item) }) // this is safe because only one mutable reference to the same item can be emitted.s
		} else {
			None
		}
	}
}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> Iterator for IterMut<'a, K, V, I, C> {
	type Item = KeyRefValueMut<'a, K, V, I, C>;

	#[inline]
	fn next(&mut self) -> Option<KeyRefValueMut<'a, K, V, I, C>> {
		self.next_item().map(KeyRefValueMut)
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		(self.len, Some(self.len))
	}
}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> FusedIterator for IterMut<'a, K, V, I, C> {}
impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> ExactSizeIterator for IterMut<'a, K, V, I, C> {}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> DoubleEndedIterator for IterMut<'a, K, V, I, C> {
	#[inline]
	fn next_back(&mut self) -> Option<KeyRefValueMut<'a, K, V, I, C>> {
		self.next_back_item().map(KeyRefValueMut)
	}
}

/// Iterator that can mutate the tree in place.
pub struct EntriesMut<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> {
	/// The tree reference.
	btree: &'a mut BTreeMap<K, V, I, C>,
	/// Address of the next item, or last valid address.
	addr: Address<I>,
	len: usize,
}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> EntriesMut<'a, K, V, I, C> {
	/// Create a new iterator over all the items of the map.
	#[inline]
	fn new(btree: &'a mut BTreeMap<K, V, I, C>) -> EntriesMut<'a, K, V, I, C> {
		let addr = btree.first_back_address();
		let len = btree.len();
		EntriesMut { btree, addr, len }
	}

	/// Get the next visited item without moving the iterator position.
	#[inline]
	pub fn peek(&'a self) -> Option<ItemRef<'a, K, V, I, C>> {
		self.btree.item(self.addr)
	}

	/// Get the next visited item without moving the iterator position.
	#[inline]
	pub fn peek_mut(&'a mut self) -> Option<ItemMut<'a, K, V, I, C>> {
		self.btree.item_mut(self.addr)
	}

	/// Get the next item and move the iterator to the next position.
	#[inline]
	pub fn next_item(&mut self) -> Option<ItemMut<'a, K, V, I, C>> {
		let after_addr = self.btree.next_item_or_back_address(self.addr);
		match self.btree.item_mut(self.addr) {
			Some(item) => {
				self.len -= 1;
				self.addr = after_addr.unwrap();
				Some(unsafe { alter_item_lifetime::<K, V, I, C>(item) }) // this is safe because only one mutable reference to the same item can be emitted.
			},
			None => None,
		}
	}

	/// Insert a new item in the map before the next item.
	///
	/// ## Correctness
	///
	/// It is safe to insert any key-value pair here, however this might break the well-formedness
	/// of the underlying tree, which relies on several invariants.
	/// To preserve these invariants,
	/// the key must be *strictly greater* than the previous visited item's key,
	/// and *strictly less* than the next visited item
	/// (which you can retrive through `IterMut::peek` without moving the iterator).
	/// If this rule is not respected, the data structure will become unusable
	/// (invalidate the specification of every method of the API).
	#[inline]
	pub fn insert(&mut self, key: K, value: V) {
		let addr = self.btree.insert_at(self.addr, Item::new(key, value));
		self.btree.next_item_or_back_address(addr);
		self.len += 1;
	}

	/// Remove the next item and return it.
	#[inline]
	pub fn remove(&mut self) -> Option<Item<K, V>> {
		match self.btree.remove_at(self.addr) {
			Some((item, addr)) => {
				self.len -= 1;
				self.addr = addr;
				Some(item)
			}
			None => None,
		}
	}
}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> Iterator for EntriesMut<'a, K, V, I, C> {
	type Item = KeyRefValueMut<'a, K, V, I, C>;

	#[inline]
	fn next(&mut self) -> Option<KeyRefValueMut<'a, K, V, I, C>> {
		self.next_item().map(KeyRefValueMut)
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		(self.len, Some(self.len))
	}
}

/// An owning iterator over the entries of a `BTreeMap`.
///
/// This `struct` is created by the [`into_iter`] method on [`BTreeMap`]
/// (provided by the `IntoIterator` trait). See its documentation for more.
///
/// [`into_iter`]: IntoIterator::into_iter
pub struct IntoIter<K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> {
	/// The tree reference.
	btree: BTreeMap<K, V, I, C>,

	/// Address of the next item, or the last valid address.
	addr: Option<Address<I>>,

	/// Address following the last item.
	end: Option<Address<I>>,

	/// Number of remaining items.
	len: usize,
}

impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> IntoIter<K, V, I, C> {
	#[inline]
	pub fn new(btree: BTreeMap<K, V, I, C>) -> Self {
		let addr = btree.first_item_address();
		let len = btree.len();
		IntoIter {
			btree,
			addr,
			end: None,
			len,
		}
	}

	#[inline]
	fn cleanup(&mut self) {
		if self.end.is_some() {
			while self.addr != self.end {
				let addr = self.addr.unwrap();
				self.addr = self.btree.next_back_address(addr);

				if addr.offset >= self.btree.node(addr.id).item_count() {
					let node = self.btree.release_node(addr.id);
					std::mem::forget(node); // do not call `drop` on the node since items have been moved.
				}
			}
		}

		if let Some(addr) = self.addr {
			let mut id = Some(addr.id);
			while let Some(node_id) = id {
				let node = self.btree.release_node(node_id);
				id = node.parent();
				std::mem::forget(node); // do not call `drop` on the node since items have been moved.
			}
		}
	}
}

impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> FusedIterator for IntoIter<K, V, I, C> {}
impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> ExactSizeIterator for IntoIter<K, V, I, C> {}

impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> Iterator for IntoIter<K, V, I, C> {
	type Item = (K, V);

	#[inline]
	fn next(&mut self) -> Option<(K, V)> {
		match self.addr {
			Some(addr) => {
				if self.len > 0 {
					self.len -= 1;

					let item = unsafe {
						// this is safe because the item at `self.addr` exists and is never touched again.
						std::ptr::read(self.btree.item(addr).unwrap().as_ptr())
					};

					if self.len > 0 {
						self.addr = self.btree.next_back_address(addr); // an item address is always followed by a valid address.

						while let Some(addr) = self.addr {
							if addr.offset < self.btree.node(addr.id).item_count() {
								break; // we have found an item address.
							} else {
								self.addr = self.btree.next_back_address(addr);

								// we have gove through every item of the node, we can release it.
								let node = self.btree.release_node(addr.id);
								std::mem::forget(node); // do not call `drop` on the node since items have been moved.
							}
						}
					} else {
						self.cleanup();
					}

					Some(item.into_pair())
				} else {
					None
				}
			}
			None => None,
		}
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		(self.len, Some(self.len))
	}
}

impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> DoubleEndedIterator for IntoIter<K, V, I, C> {
	fn next_back(&mut self) -> Option<(K, V)> {
		if self.len > 0 {
			let addr = match self.end {
				Some(mut addr) => {
					addr = self.btree.previous_front_address(addr).unwrap();
					while addr.offset.is_before() {
						let id = addr.id;
						addr = self.btree.previous_front_address(addr).unwrap();

						// we have gove through every item of the node, we can release it.
						let node = self.btree.release_node(id);
						std::mem::forget(node); // do not call `drop` on the node since items have been moved.
					}

					addr
				}
				None => self.btree.last_item_address().unwrap(),
			};

			self.len -= 1;

			let item = unsafe {
				// this is safe because the item at `self.end` exists and is never touched again.
				std::ptr::read(self.btree.item(addr).unwrap().as_ptr())
			};

			self.end = Some(addr);

			if self.len == 0 {
				self.cleanup();
			}

			Some(item.into_pair())
		} else {
			None
		}
	}
}

impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> IntoIterator for BTreeMap<K, V, I, C> {
	type Item = (K, V);
	type IntoIter = IntoIter<K, V, I, C>;

	#[inline]
	fn into_iter(self) -> IntoIter<K, V, I, C> {
		IntoIter::new(self)
	}
}

pub(crate) struct DrainFilterInner<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> {
	/// The tree reference.
	btree: &'a mut BTreeMap<K, V, I, C>,
	/// Address of the next item, or last valid address.
	addr: Address<I>,
	len: usize,
}

impl<'a, K: 'a, V: 'a, I: Index, C: Slab<Node<K, V, I>, Index=I>> DrainFilterInner<'a, K, V, I, C> {
	#[inline]
	pub fn new(btree: &'a mut BTreeMap<K, V, I, C>) -> Self {
		let addr = btree.first_back_address();
		let len = btree.len();
		DrainFilterInner { btree, addr, len }
	}

	#[inline]
	pub fn size_hint(&self) -> (usize, Option<usize>) {
		(0, Some(self.len))
	}

	#[inline]
	fn next_item<F: FnMut(&K, &mut V) -> bool>(&mut self, pred: &mut F) -> Option<Item<K, V>> {
		if self.addr.id.is_nowhere() {
			debug_assert_eq!(self.len, 0);
			return None;
		}

		loop {
			let drain = {
				let item = self.btree.item_mut(self.addr);
				match item {
					Some(mut item) => {
						let (key, value) = item.as_pair_mut();
						self.len -= 1;
						(*pred)(key, value)
					}
					None => return None,
				}
			};
			if drain {
				let (item, next_addr) = self.btree.remove_at(self.addr).unwrap();
				self.addr = next_addr;
				return Some(item);
			} else {
				self.addr = self.btree.next_item_or_back_address(self.addr).unwrap();
			}
		}
	}

	#[inline]
	pub fn next<F: FnMut(&K, &mut V) -> bool>(&mut self, pred: &mut F) -> Option<(K, V)> {
		self.next_item(pred).map(Item::into_pair)
	}
}

pub struct DrainFilter<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>, F: FnMut(&K, &mut V) -> bool> {
	pred: F,
	inner: DrainFilterInner<'a, K, V, I, C>,
}

impl<'a, K: 'a, V: 'a, I: Index, C: Slab<Node<K, V, I>, Index=I>, F: FnMut(&K, &mut V) -> bool>
	DrainFilter<'a, K, V, I, C, F> {
	#[inline]
	fn new(btree: &'a mut BTreeMap<K, V, I, C>, pred: F) -> Self {
		DrainFilter {
			pred,
			inner: DrainFilterInner::new(btree),
		}
	}
}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>, F: FnMut(&K, &mut V) -> bool> FusedIterator for DrainFilter<'a, K, V, I, C, F> {}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>, F: FnMut(&K, &mut V) -> bool> Iterator for DrainFilter<'a, K, V, I, C, F> {
	type Item = (K, V);

	#[inline]
	fn next(&mut self) -> Option<(K, V)> {
		self.inner.next(&mut self.pred)
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.inner.size_hint()
	}
}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>, F: FnMut(&K, &mut V) -> bool> Drop for DrainFilter<'a, K, V, I, C, F> {
	#[inline]
	fn drop(&mut self) {
		// keep calling next until we run out of items
		while self.next().is_some() {}
	}
}

pub struct Keys<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> {
	inner: Iter<'a, K, V, I, C>,
}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> FusedIterator for Keys<'a, K, V, I, C> {}
impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> ExactSizeIterator for Keys<'a, K, V, I, C> where {}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> Iterator for Keys<'a, K, V, I, C> {
	type Item = KeyRef<'a, K, V, I, C>;

	#[inline]
	fn next(&mut self) -> Option<KeyRef<'a, K, V, I, C>> {
		self.inner.next().map(|kv| kv.into_key_ref())
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.inner.size_hint()
	}
}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> DoubleEndedIterator for Keys<'a, K, V, I, C> {
	#[inline]
	fn next_back(&mut self) -> Option<KeyRef<'a, K, V, I, C>> {
		self.inner.next_back().map(|kv| kv.into_key_ref())
	}
}

impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> FusedIterator for IntoKeys<K, V, I, C> {}
impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> ExactSizeIterator for IntoKeys<K, V, I, C> {}

pub struct IntoKeys<K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> {
	inner: IntoIter<K, V, I, C>,
}

impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> Iterator for IntoKeys<K, V, I, C> {
	type Item = K;

	#[inline]
	fn next(&mut self) -> Option<K> {
		self.inner.next().map(|(k, _)| k)
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.inner.size_hint()
	}
}

impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> DoubleEndedIterator for IntoKeys<K, V, I, C> {
	#[inline]
	fn next_back(&mut self) -> Option<K> {
		self.inner.next_back().map(|(k, _)| k)
	}
}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> FusedIterator for Values<'a, K, V, I, C> {}
impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> ExactSizeIterator for Values<'a, K, V, I, C> {}

pub struct Values<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> {
	inner: Iter<'a, K, V, I, C>,
}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> Iterator for Values<'a, K, V, I, C> {
	type Item = ValueRef<'a, K, V, I, C>;

	#[inline]
	fn next(&mut self) -> Option<ValueRef<'a, K, V, I, C>> {
		self.inner.next().map(|kv| kv.into_value_ref())
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.inner.size_hint()
	}
}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> DoubleEndedIterator for Values<'a, K, V, I, C> {
	#[inline]
	fn next_back(&mut self) -> Option<ValueRef<'a, K, V, I, C>> {
		self.inner.next_back().map(|kv| kv.into_value_ref())
	}
}

pub struct ValuesMut<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> {
	inner: IterMut<'a, K, V, I, C>,
}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> FusedIterator for ValuesMut<'a, K, V, I, C> {}
impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> ExactSizeIterator for ValuesMut<'a, K, V, I, C> {}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> Iterator for ValuesMut<'a, K, V, I, C> {
	type Item = ValueMut<'a, K, V, I, C>;

	#[inline]
	fn next(&mut self) -> Option<ValueMut<'a, K, V, I, C>> {
		self.inner.next().map(|kv| kv.into_value_mut())
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.inner.size_hint()
	}
}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> DoubleEndedIterator for ValuesMut<'a, K, V, I, C> {
	#[inline]
	fn next_back(&mut self) -> Option<ValueMut<'a, K, V, I, C>> {
		self.inner.next_back().map(|kv| kv.into_value_mut())
	}
}

pub struct IntoValues<K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> {
	inner: IntoIter<K, V, I, C>,
}

impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> FusedIterator for IntoValues<K, V, I, C> {}
impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> ExactSizeIterator for IntoValues<K, V, I, C> {}

impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> Iterator for IntoValues<K, V, I, C> {
	type Item = V;

	#[inline]
	fn next(&mut self) -> Option<V> {
		self.inner.next().map(|(_, v)| v)
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		self.inner.size_hint()
	}
}

impl<K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> DoubleEndedIterator for IntoValues<K, V, I, C> {
	#[inline]
	fn next_back(&mut self) -> Option<V> {
		self.inner.next_back().map(|(_, v)| v)
	}
}

fn is_valid_range<T: Ord + ?Sized, R: RangeBounds<T>>(range: &R) -> bool {
	match (range.start_bound(), range.end_bound()) {
		(Bound::Included(start), Bound::Included(end)) => start <= end,
		(Bound::Included(start), Bound::Excluded(end)) => start <= end,
		(Bound::Included(_), Bound::Unbounded) => true,
		(Bound::Excluded(start), Bound::Included(end)) => start <= end,
		(Bound::Excluded(start), Bound::Excluded(end)) => start < end,
		(Bound::Excluded(_), Bound::Unbounded) => true,
		(Bound::Unbounded, _) => true,
	}
}

pub struct Range<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> {
	/// The tree reference.
	btree: &'a BTreeMap<K, V, I, C>,
	/// Address of the next item or last back address.
	addr: Address<I>,
	end: Address<I>,
}

/// If addr is past the end of a node, make it the start of the next node
#[inline]
fn shift_to_start<K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>>(
	btree: &BTreeMap<K, V, I, C>,
	addr: Address<I>
) -> Address<I> {
	match btree.item(addr) {
		None => btree.next_item_or_back_address(addr).unwrap(),
		Some(_) => addr
	}
}

#[inline]
fn range_addr_end<T: Ord + ?Sized, K: Borrow<T>, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>>(
	btree: &BTreeMap<K, V, I, C>,
	range: impl RangeBounds<T>
) -> (Address<I>, Address<I>) {
	if !is_valid_range(&range) {
		panic!("Invalid range");
	}

	let addr = match range.start_bound() {
		Bound::Included(start) => match btree.address_of(start) {
			Ok(addr) => addr,
			Err(addr) => shift_to_start(btree, addr),
		},
		Bound::Excluded(start) => match btree.address_of(start) {
			Ok(addr) => btree.next_item_or_back_address(addr).unwrap(),
			Err(addr) => shift_to_start(btree, addr),
		},
		Bound::Unbounded => btree.first_back_address(),
	};

	let end = match range.end_bound() {
		Bound::Included(end) => match btree.address_of(end) {
			Ok(addr) => btree.next_item_or_back_address(addr).unwrap(),
			Err(addr) => shift_to_start(btree, addr),
		},
		Bound::Excluded(end) => match btree.address_of(end) {
			Ok(addr) => addr,
			Err(addr) => shift_to_start(btree, addr),
		},
		Bound::Unbounded => btree.first_back_address(),
	};

	(addr, end)
}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> Range<'a, K, V, I, C> {
	fn new<T: Ord + ?Sized>(
		btree: &'a BTreeMap<K, V, I, C>,
		range: impl RangeBounds<T>,
	) -> Self where K: Borrow<T> {
		let (addr, end) = range_addr_end(btree, range);
		Range { btree, addr, end }
	}
}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> Iterator for Range<'a, K, V, I, C> {
	type Item = KeyValueRef<'a, K, V, I, C>;

	#[inline]
	fn next(&mut self) -> Option<KeyValueRef<'a, K, V, I, C>> {
		if self.addr != self.end {
			let item = self.btree.item(self.addr).unwrap();
			self.addr = self.btree.next_item_or_back_address(self.addr).unwrap();
			Some(KeyValueRef(item))
		} else {
			None
		}
	}
}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> FusedIterator for Range<'a, K, V, I, C> {}

impl<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> DoubleEndedIterator for Range<'a, K, V, I, C> {
	#[inline]
	fn next_back(&mut self) -> Option<KeyValueRef<'a, K, V, I, C>> {
		if self.addr != self.end {
			let addr = self.btree.previous_item_address(self.addr).unwrap();
			let item = self.btree.item(addr).unwrap();
			self.end = addr;
			Some(KeyValueRef(item))
		} else {
			None
		}
	}
}

pub struct RangeMut<'a, K, V, I: Index, C: SlabView<Node<K, V, I>, Index=I>> {
	/// The tree reference.
	btree: &'a mut BTreeMap<K, V, I, C>,
	/// Address of the next item or last back address.
	addr: Address<I>,
	end: Address<I>,
}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> RangeMut<'a, K, V, I, C> {
	fn new<T: Ord + ?Sized>(
		btree: &'a mut BTreeMap<K, V, I, C>,
		range: impl RangeBounds<T>
	) -> Self where K: Borrow<T> {
		let (addr, end) = range_addr_end(btree, range);
		RangeMut { btree, addr, end }
	}

	#[inline]
	fn next_item(&mut self) -> Option<ItemMut<'a, K, V, I, C>> {
		if self.addr != self.end {
			let addr = self.addr;
			self.addr = self.btree.next_item_or_back_address(addr).unwrap();
			let item = self.btree.item_mut(addr).unwrap();
			Some(unsafe { alter_item_lifetime::<K, V, I, C>(item) }) // this is safe because only one mutable reference to the same item can be emitted.
		} else {
			None
		}
	}

	#[inline]
	fn next_back_item(&mut self) -> Option<ItemMut<'a, K, V, I, C>> {
		if self.addr != self.end {
			let addr = self.btree.previous_item_address(self.addr).unwrap();
			let item = self.btree.item_mut(addr).unwrap();
			self.end = addr;
			Some(unsafe { alter_item_lifetime::<K, V, I, C>(item) }) // this is safe because only one mutable reference to the same item can be emitted.s
		} else {
			None
		}
	}
}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> Iterator for RangeMut<'a, K, V, I, C> {
	type Item = KeyRefValueMut<'a, K, V, I, C>;

	#[inline]
	fn next(&mut self) -> Option<KeyRefValueMut<'a, K, V, I, C>> {
		self.next_item().map(KeyRefValueMut)
	}
}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> FusedIterator for RangeMut<'a, K, V, I, C> {}

impl<'a, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>> DoubleEndedIterator for RangeMut<'a, K, V, I, C> {
	#[inline]
	fn next_back(&mut self) -> Option<KeyRefValueMut<'a, K, V, I, C>> {
		self.next_back_item().map(KeyRefValueMut)
	}
}

unsafe fn alter_item_lifetime<'a, 'b, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>>(
	item: ItemMut<'a, K, V, I, C>
) -> ItemMut<'b, K, V, I, C> {
	std::mem::transmute(item)
}

unsafe fn alter_value_lifetime<'a, 'b, K, V, I: Index, C: Slab<Node<K, V, I>, Index=I>>(
	item: ValueMut<'a, K, V, I, C>
) -> ValueMut<'b, K, V, I, C> {
	std::mem::transmute(item)
}
