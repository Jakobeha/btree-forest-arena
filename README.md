# btree-store: Flexible B-trees whose storage and indexing is abstracted, so they can be stored on the stack, in the same allocator, and in other ways

[![CI](https://github.com/Jakobeha/btree-store/workflows/CI/badge.svg)](https://github.com/Jakobeha/btree-store/actions)
[![Crate informations](https://img.shields.io/crates/v/btree-store.svg?style=flat-square)](https://crates.io/crates/btree-store)
[![License](https://img.shields.io/crates/l/btree-store.svg?style=flat-square)](https://github.com/Jakobeha/btree-store#license)
[![Documentation](https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square)](https://docs.rs/btree-store)

Forked from [btree-slab](https://github.com/timothee-haudebourg/btree-slab).

## Why would you want this?

- You want to perform complex operations on a b-tree which aren't possible via methods exposed by the standard library, in stable Rust
- You want a more memory-compact b-tree
- You want to store a b-tree on the stack or some other pre-allocated region, and control how nodes are allocated
- You have many tiny b-trees and want to store them all in the same memory region, reducing allocations and increasing localization

## What is it?

Mainly, [`generic::BTreeMap<K, V, I, C>`](https://docs.rs/btree-store/latest/btree-store/generic/struct.BTreeMap.html) and [`generic::BTreeSet<T, I, C>`](https://docs.rs/btree-store/latest/btree-store/generic/struct.BTreeSet.html): alternative implementations of the standard `BTreeMap` and `BTreeSet` data structures, with abstracted storage (`C` type parameter) and indexing (`I` type parameter). These provide all operations the of standard library's b-trees and more, and also allow you to use initialize the b-tree in a custom store, which uses a custom index.

### Instantiations

This library also provides useful instantiations under various features, under their respective modules (so definitions in feature `slab` are under the `slab` module)

- `slab`: Enabled by default. Defines [`BTreeMap<K, V>`] and [`BTreeSet<T>`] which use a slab allocator
- Shareable
  - `shareable-slab`: Defines [`BTreeMap<'_, K, V>`] and [`BTreeSet<'_, T>`] which use a reference to a shared slab allocator, [`ShareableSlab<T>`]. `ShareableSlab` is internally a slab within a `RefCell`, so it will *panic* at runtime if you hold a reference to one element while inserting or mutating another (this is because insertion can cause the underlying `Vec` to grow and reallocate; simultaneous mutation is theoretically safe when the store is only used by b-trees, but currently not supported).
  - `concurrent-shareable-slab`: Defines [`BTreeMap<'_, K, V>`] and [`BTreeSet<'_, T>`] which use a reference to [`ShareableSlab<T>`], a slab within a `RwLock`. This allows b-trees on separate threads to use the same storage, but not at the same time.
  - `shareable-slab-simultaneous-mutation`: Defines [`BTreeMap<'_, K, V>`], [`BTreeSet<'_, T>`], and [`ShareableSlab<T>`] which uses `UnsafeCell` and pointer indices to allow concurrent access, mutation, and removal, but not insertion (still *panics* if there are active references on insertion). References to this `ShareableSlab<T>` must be created in an `unsafe` block, with the invariant that indices don't get simultaneously used in ways which violate Rust's borrowing rules (they won't if they're only passed to b-trees).
  - `shareable-slab-arena`: Defines [`BTreeMap<'_, K, V>`], [`BTreeSet<'_, T>`], and [`ShareableSlabArena<T>`] which is a slab backed by an arena (`Vec<Vec<T>>`), so that it supports concurrent access, mutation, removal, and insertion (no way to make it panic). References to this `ShareableSlab<T>` must also be created `unsafe`ly.
- Alternate memory representation
  - `small-slab`: Defines [`BTreeMap<K, V>`] and [`BTreeSet<T>`] which use an owned [`SmallSlab`], which is a slab allocator backed by a [`smallvec::SmallVec`]. It will be stored on the stack unless either it grows too large, or operations cause one of the nodes to spill instead of performing an expensive rebalance. Therefore, this option is good if you are repeatedly creating and then deleting a lot of usually-small b-trees, or creating a lot of small b-trees but don't want to use shared storage.
  - `array-slab`: Defines [`BTreeMap<K, V>`] and [`BTreeSet<T>`] which use an owned [`ArraySlab`], which is a slab allocator backed by an array instead of a vector. This means they can be stored on the stack and in fixed memory regions. *TODO: the current implementation of `BTreeMap` and `BTreeSet` will occasionally spill onto the heap instead of doing expensive rebalancing, so you still can't use it in no-std or rely on to be entirely stored in a fixed memory region e.g. for easy serialization and deep copying. Add an option to the store to *panic* instead.*
  - `shareable-bump`: Defines [`BTreeMap<'_, K, V>`] and [`BTreeSet<'_, T>`] which store their contents in [`bumpalo::Bump`]. Unlike the slab allocators, a single `Bump` can store multiple b-trees of *different types*. However, it will never reuse freed memory, and requires every type to be dropless (have no `Drop` impl or nested field with a `Drop` impl). Allows simultaneous access, mutation, removal, and insertion.

## Usage

From the user point of view, the collection provided by this crate can be used just like the standard `BTreeMap` and `BTreeSet` collections.

```rust
use btree_store::slab::BTreeMap;

// type inference lets us omit an explicit type signature (which
// would be `BTreeMap<&str, &str>` in this example).
let mut movie_reviews = BTreeMap::new();

// review some movies.
movie_reviews.insert("Office Space",       "Deals with real issues in the workplace.");
movie_reviews.insert("Pulp Fiction",       "Masterpiece.");
movie_reviews.insert("The Godfather",      "Very enjoyable.");
movie_reviews.insert("The Blues Brothers", "Eye lyked it a lot.");

// check for a specific one.
if !movie_reviews.contains_key("Les Misérables") {
    println!("We've got {} reviews, but Les Misérables ain't one.",
             movie_reviews.len());
}

// oops, this review has a lot of spelling mistakes, let's delete it.
movie_reviews.remove("The Blues Brothers");

// look up the values associated with some keys.
let to_find = ["Up!", "Office Space"];
for movie in &to_find {
    match movie_reviews.get(movie) {
       Some(review) => println!("{}: {}", movie, review),
       None => println!("{} is unreviewed.", movie)
    }
}

// Look up the value for a key (will panic if the key is not found).
println!("Movie review: {}", movie_reviews["Office Space"]);

// iterate over everything.
for elem in &movie_reviews {
    let (movie, review) = elem.as_pair();
    println!("{}: \"{}\"", movie, review);
}
```

### Shared storage

To create multiple b-trees which use the same storage, simply use a compatible storage and provide a shared reference to each b-tree.

```rust
#![cfg(feature = "shareable-slab")]
use btree_store::shareable_slab::{BTreeSet, ShareableSlab};

let shared_store = ShareableSlab::new();
let mut foo_bars: BTreeSet<'_, &'static str> = BTreeSet::new_in(&shared_store);
let mut alphabeticals: BTreeSet<'_, &'static str> = BTreeSet::new_in(&shared_store);
foo_bars.insert("foo");
alphabeticals.insert("abc");
foo_bars.insert("bar");
alphabeticals.insert("def");
foo_bars.insert("baz");
foo_bars.insert("qux");
alphabeticals.insert("xyz");
for elem in &foo_bars {
    println!("{}", elem);
}
for elem in &alphabeticals {
    println!("{}", elem);
}
```

Just make sure that, with `shareable_slab`, you don't hold a reference to an element in one of the sets (including iterating) while mutating another set. If you need this, use `shareable_slab_simultaneous_mutation` or `shareable_slab_arena` instead.

### Custom node allocation

One can use `btree_store::generic::BTreeMap` to use a custom slab type to handle nodes allocation. For example, here is an implementation for [`thunderdome`](https://docs.rs/thunderdome)

```rust
#![cfg(feature = "_thunderdome_example")]

use std::fmt::Formatter;
use btree_store::generic::{BTreeMap, Node};
use btree_store::generic::{OwnedSlab, Slab, SlabView};
use btree_store::generic::slab::{Ref, RefMut};

// region Thunderdome store impl
// We have to create wrapper types because orphan instances aren't allowed
struct Arena<T>(thunderdome::Arena<T>);

#[derive(Clone, Copy, PartialEq, Eq)]
struct Index(thunderdome::Index);

impl std::fmt::Display for Index {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self.0)
  }
}

impl btree_store::generic::slab::Index for Index {
  #[inline]
  fn nowhere() -> Self {
    Index(thunderdome::Index::from_bits(u64::MAX).unwrap())
  }

  #[inline]
  fn is_nowhere(&self) -> bool {
    self.0.to_bits() == u64::MAX
  }
}


impl<T> SlabView<T> for Arena<T> {
  type Index = Index;
  type Ref<'a, U: ?Sized + 'a> = &'a U where T: 'a;

  #[inline]
  fn get(&self, index: Self::Index) -> Option<Self::Ref<'_, T>> {
    self.0.get(index.0)
  }
}

impl<T> Slab<T> for Arena<T> {
  type RefMut<'a, U: ?Sized + 'a> = &'a mut U where T: 'a;

  #[inline]
  fn insert(&mut self, value: T) -> Self::Index {
    Index(self.0.insert(value))
  }

  #[inline]
  fn remove(&mut self, index: Self::Index) -> Option<T> {
    self.0.remove(index.0)
  }

  #[inline]
  fn get_mut(&mut self, index: Self::Index) -> Option<Self::RefMut<'_, T>> {
    self.0.get_mut(index.0)
  }

  #[inline]
  fn clear_fast(&mut self) -> bool {
    // Is owned
    self.clear();
    true
  }
}

impl<T> OwnedSlab<T> for Arena<T> {
  #[inline]
  fn clear(&mut self) {
    self.0.clear();
  }
}
// endregion

// Usage
let arena = Arena(thunderdome::Arena::with_capacity(12));
let mut heights_in_cm: BTreeMap< & 'static str, f64, Index, Arena<Node< & 'static str, f64, Index> > > = BTreeMap::new_in(arena);
heights_in_cm.insert("Bob", 177.3);
heights_in_cm.insert("Tom", 184.7);
```

In this example, we also initialize the slab with a fixed capacity.

## Extended API & Addressing

In this implementation of B-Trees, each node of a tree is addressed by the `Address` type. The extended API, visible through the `BTreeExt` trait, allows the caller to explore, access and modify the internal structure of the tree using this addressing system. This can be used to further extend the functionalities of the `BTreeMap` collection, for instance see what [`btree-range-map`](https://crates.io/crates/btree-range-map) does with [`btree-slab`](https://crates.io/crates/btree-slab) (which provides a similar extended API, because this crate was derived from it).

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Forked from [btree-slab](https://github.com/timothee-haudebourg/btree-slab), which is also dual licensed under Apache 2.0 "or" MIT.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
