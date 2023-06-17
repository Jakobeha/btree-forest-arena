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

- `slab`: Defines [`BTreeMap<K, V>`] and [`BTreeSet<T>`] which use a slab allocator

### Shareable

- `shareable-slab`: Defines [`BTreeMap<K, V>`] and [`BTreeSet<T>`] which use a reference to a shared slab allocator, [`ShareableSlab<T>`]. `ShareableSlab` is internally a slab within a `RefCell`, so it will *panic* at runtime if you hold a reference to one element while inserting or mutating another (this is because insertion can cause the underlying `Vec` to grow and reallocate; simultaneous mutation is theoretically safe when the store is only used by b-trees, but currently not supported).
- `concurrent-shareable-slab`: Defines [`BTreeMap<K, V>`] and [`BTreeSet<T>`] which use a reference to [`ShareableSlab<T>`], a slab within a `RwLock`. This allows b-trees on separate threads to use the same storage, but not at the same time. (TODO)
- `shareable-slab-simultaneous-mutation`: Defines [`BTreeMap<K, V>`], [`BTreeSet<T>`], and [`ShareableSlab<T>`] which uses `UnsafeCell` and pointer indices to allow concurrent access, mutation, and removal, but not insertion (still *panics* if there are active references on insertion). References to this `ShareableSlab<T>` must be created in an `unsafe` block, with the invariant that indices don't get simultaneously used in ways which violate Rust's borrowing rules (they won't if they're only passed to b-trees)
- `shareable-slab-arena`: Defines [`BTreeMap<K, V>`], [`BTreeSet<T>`], and [`ShareableSlabArena<T>`] which is a slab backed by an arena (`Vec<Vec<T>>`), so that it supports concurrent access, mutation, removal, and insertion (no way to make it panic). References to this `ShareableSlab<T>` must also be created `unsafe`ly.

### Alternate memory representation

- `array-slab`: Defines [`BTreeMap<K, V>`] and [`BTreeSet<T>`] which use an owned [`ArraySlab`], which is a slab allocator backed by an array instead of a vector. This means they can be stored on the stack and in fixed memory regions. *TODO: the current implementation of `BTreeMap` and `BTreeSet` will occasionally spill onto the heap instead of doing expensive rebalancing, so you still can't use it in no-std or rely on to be entirely stored in a fixed memory region e.g. for easy serialization and deep copying. Add an option to disable this.*
- TODO: `bumpalo` which can store multiple b-trees of different types, but requires every type to be dropless (have no `Drop` impl or nested field with a `Drop` impl)

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
for (movie, review) in &movie_reviews {
    println!("{}: \"{}\"", movie, review);
}
```

### Custom node allocation

One can use `btree_slab::generic::BTreeMap` to use a custom slab type to handle nodes allocation.

```rust
use my_slab::{MyIndex, MySlab};
use btree_store::generic::BTreeMap;

let my_slab = MySlab::with_capacity(12);
let mut heights_in_cm: BTreeMap<&'static str, f64, MyIndex, MySlab<Node<&'static str, f64>>> = BTreeMap::new_in(my_slab);
heights_in_cm.insert("Bob", 177.3);
heights_in_cm.insert("Tom", 184.7);
```

In this example, we use a different kind of slab (`MySlab`) instead of `slab::Slab`, and initialize it with a fixed capacity.

### Shared storage

To create multiple b-trees which use the same storage, simply use a compatible storage and provide a shared reference to each b-tree.

```rust
use btree_store::shared_slab::{BTreeSet, SharedSlab};

let shared_store = SharedSlab::new(12);
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

### Extended API & Addressing

In this implementation of B-Trees, each node of a tree is addressed by the `Address` type. The extended API, visible through the `BTreeExt` trait, allows the caller to explore, access and modify the internal structure of the tree using this addressing system. This can be used to further extend the functionalities of the `BTreeMap` collection, for instance see what [`btree-range-map`](https://crates.io/crates/btree-range-map) does with [`btree-slab`](https://crates.io/crates/btree-slab) (which provides a similar extended API, because this crate was derived from it).

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Forked from [btree-slab](https://github.com/timothee-haudebourg/btree-slab), which is also dual licensed under Apache 2.0 "or" MIT.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
