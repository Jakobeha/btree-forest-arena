# btree-forest-arena: B-trees backed by a slab/arena to reduce allocations and increase locality

[![CI](https://github.com/Jakobeha/btree-store/workflows/CI/badge.svg)](https://github.com/Jakobeha/btree-store/actions)
[![Crate informations](https://img.shields.io/crates/v/btree-store.svg?style=flat-square)](https://crates.io/crates/btree-store)
[![License](https://img.shields.io/crates/l/btree-store.svg?style=flat-square)](https://github.com/Jakobeha/btree-store#license)
[![Documentation](https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square)](https://docs.rs/btree-store)

Forked from [btree-slab](https://github.com/timothee-haudebourg/btree-slab).

## Why would you want this?

You have many b-trees, some of which are very tiny, and want to reduce allocations and increase localization by storing them all in the same memory region.

## What is it?

`BTreeMap` and `BTreeSet` with an interface almost identical to standard library (with some additional features), but constructed via `new_in(&'a BTreeStore)`.

`BTreeStore` is internally an arena which maintains a linked list in the allocated but discarded nodes like a slab. This means we can reuse nodes by dropped b-trees, although the memory won't get reclaimed until the arena is destroyed.

```rust
use btree_forest_arena::{BTreeSet, BTreeStore};

fn main() {
  let store = BTreeStore::new();
  let mut foo_bars: BTreeSet<'_, &'static str> = BTreeSet::new_in(&store);
  let mut alphabeticals: BTreeSet<'_, &'static str> = BTreeSet::new_in(&store);
  
  foo_bars.insert("foo");
  alphabeticals.insert("abc");
  foo_bars.insert("bar");
  alphabeticals.insert("def");
  foo_bars.insert("baz");
  foo_bars.insert("qux");
  alphabeticals.insert("xyz");
  foo_bars.remove("baz");
  alphabeticals.remove("def");
  for elem in &foo_bars {
      println!("Iterate {}", elem);
  }
  for elem in alphabeticals.drain_filter(|a| a.starts_with('a')) {
      println!("Drain {}", elem);
  }
  for elem in alphabeticals {
      println!("Consume {}", elem)
  }
}
```

## Benchmarks

Benchmarks are run for a sequence of operations including insertion, retrieval, iteration, and removal. We vary the # and size of maps.

[Full Report](criterion/report/index.html)

![1_map_3000_entries](criterion/1_map_3000_entries/report/violin.svg)
![10_maps_300_entries](criterion/10_maps_3000_entries/report/violin.svg)
![100_maps_30_entries](criterion/100_maps_3000_entries/report/violin.svg)
![1000_maps_3_entries](criterion/1000_maps_3000_entries/report/violin.svg)

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Forked from [btree-slab](https://github.com/timothee-haudebourg/btree-slab), which is also dual licensed under Apache 2.0 "or" MIT.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
