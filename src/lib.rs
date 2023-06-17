//! This crate provides an alternative implementation to the standard `BTreeMap`
//! and `BTreeSet` data structures based on a slab data-structure. In principle,
//! this implementation is more flexible and more memory efficient. It is more
//! flexible by providing an extended set of low-level operations on B-Trees
//! through the `BTreeExt` trait which can be used to further extend the
//! functionalities of the `BTreeMap` collection.
//!
//! In addition, the underlying node allocation scheme is abstracted by a type
//! parameter that can be instantiated by any data structure implementing
//! slab-like operations.
//!
//! ## Different slab implementations
//!
//! By default and under the `slab` feature, the `Slab` type (from the `slab` crate) is used, which
//! means that every node of the tree are allocated in a contiguous memory region, reducing the
//! number of allocations needed.
//!
//! The `shareable-slab` feature defines a new type `ShareableSlab`, which is actually a
//! `RefCell<Slab<T>>` (again from `slab`). A shared reference to `ShareableSlab` can be used by
//! multiple `SharingBTreeMap`s from this crate, so that they all ultimately share the same memory
//! buffer.
//!
//! You can define your own implementations of `generic::Slab` as well. For example, you could
//! create a version of `ShareableSlab` which uses a bump allocator from `bumpalo`, which can then
//! be shared by b-trees with different node types (remember all types in `bumpalo` can't have drop
//! code).
//!
//! ## Usage
//!
//! From the user point of view, the collection provided by this crate can be
//! used just like the standard `BTreeMap` and `BTreeSet` collections.
//! ```
//! use btree_slab::BTreeMap;
//!
//! // type inference lets us omit an explicit type signature (which
//! // would be `BTreeMap<&str, &str>` in this example).
//! let mut movie_reviews = BTreeMap::new();
//!
//! // review some movies.
//! movie_reviews.insert("Office Space",       "Deals with real issues in the workplace.");
//! movie_reviews.insert("Pulp Fiction",       "Masterpiece.");
//! movie_reviews.insert("The Godfather",      "Very enjoyable.");
//! movie_reviews.insert("The Blues Brothers", "Eye lyked it a lot.");
//!
//! // check for a specific one.
//! if !movie_reviews.contains_key("Les Misérables") {
//!     println!("We've got {} reviews, but Les Misérables ain't one.",
//!              movie_reviews.len());
//! }
//!
//! // oops, this review has a lot of spelling mistakes, let's delete it.
//! movie_reviews.remove("The Blues Brothers");
//!
//! // look up the values associated with some keys.
//! let to_find = ["Up!", "Office Space"];
//! for movie in &to_find {
//!     match movie_reviews.get(movie) {
//!        Some(review) => println!("{}: {}", movie, review),
//!        None => println!("{} is unreviewed.", movie)
//!     }
//! }
//!
//! // Look up the value for a key (will panic if the key is not found).
//! println!("Movie review: {}", movie_reviews["Office Space"]);
//!
//! // iterate over everything.
//! for elem in &movie_reviews {
//!     let movie = elem.key();
//!     let review = elem.value();
//!     println!("{}: \"{}\"", movie, review);
//! }
//! ```
//!
//! ### Custom node allocation
//!
//! One can use `btree_slab::generic::BTreeMap` to
//! use a custom slab type to handle nodes allocation.
//!
//! ```rust
//! use btree_slab::generic::{Node, BTreeMap};
//!
//! # type K = u32;
//! # type V = u32;
//! # type MySlab<T> = slab::Slab<T>;
//! let my_slab = MySlab::with_capacity(10);
//! let mut map: BTreeMap<K, V, usize, MySlab<Node<K, V, usize>>> = BTreeMap::new_in(my_slab);
//! ```
//!
//! In this example,
//! the `Slab<Node<_, _>>` type is a slab-like data structure responsible for the nodes allocation.
//! It must implement all the traits defining the `cc_traits::Slab` trait alias.
//!
//! ## Extended API & Addressing
//!
//! In this implementation of B-Trees, each node of a tree is addressed
//! by the `Address` type.
//! The extended API, visible through the `BTreeExt` trait,
//! allows the caller to explore, access and modify the
//! internal structure of the tree using this addressing system.
//! This can be used to further extend the functionalities of the `BTreeMap`
//! collection, for example in the
//! [`btree-range-map`](https://crates.io/crates/btree-range-map) crate.
pub mod generic;
pub mod utils;
#[cfg(any(doc, feature = "shareable-slab"))]
pub mod shareable_slab;

/// B-Tree map based on `Slab`.
#[cfg(any(doc, feature = "slab"))]
pub type BTreeMap<K, V> = generic::BTreeMap<K, V, usize, slab::Slab<generic::Node<K, V, usize>>>;

/// B-Tree set based on `Slab`.
#[cfg(any(doc, feature = "slab"))]
pub type BTreeSet<T> = generic::BTreeSet<T, usize, slab::Slab<generic::Node<T, (), usize>>>;

/// B-Tree map based on `ShareableSlab`.
#[cfg(any(doc, feature = "shareable-slab"))]
pub type SharingBTreeMap<'a, K, V> = generic::BTreeMap<K, V, usize, &'a shareable_slab::ShareableSlab<generic::Node<K, V, usize>>>;

/// B-Tree set based on `ShareableSlab`.
#[cfg(any(doc, feature = "shareable-slab"))]
pub type SharingBTreeSet<'a, T> = generic::BTreeSet<T, usize, &'a shareable_slab::ShareableSlab<generic::Node<T, (), usize>>>;
