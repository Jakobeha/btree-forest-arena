extern crate rand;

use std::collections::BTreeMap as StdBTreeMap;
use btree_plus_store::{BTreeStore, BTreeMap as MyBTreeMap};

use rand::{Rng, rngs::SmallRng, SeedableRng};

// region benchmark abstraction / implementation
trait Bencher {
    fn black_box<T>(x: T) -> T;
    fn iter<Return>(&mut self, f: impl FnMut() -> Return);
}

/// Doesn't actually bench, runs the benchmarks only to test and debug them.
#[cfg(not(feature = "bench"))]
struct MockBencher;

#[cfg(feature = "bench")]
impl<'a, M: criterion::measurement::Measurement> Bencher for criterion::Bencher<'a, M> {
    fn black_box<T>(x: T) -> T {
        criterion::black_box(x)
    }

    fn iter<Return>(&mut self, f: impl FnMut() -> Return) {
        self.iter(f)
    }
}

#[cfg(not(feature = "bench"))]
impl Bencher for MockBencher {
    fn black_box<T>(x: T) -> T {
        x
    }

    fn iter<Return>(&mut self, mut f: impl FnMut() -> Return) {
        f();
    }
}
// endregion

// region map abstraction
trait BTreeMap<'store, K: Ord + 'store, V: 'store>: 'store {
    /// `()` if the store is owned
    type SharedStore: Default;
    type Iter<'a>: Iterator<Item = (&'a K, &'a V)> where 'store: 'a;
    type Range<'a>: Iterator<Item = (&'a K, &'a V)> where 'store: 'a;

    fn new_in(store: &'store Self::SharedStore) -> Self;
    fn insert(&mut self, key: K, value: V) -> Option<V>;
    fn remove(&mut self, key: &K) -> Option<V>;
    fn remove_first(&mut self) -> Option<(K, V)>;
    fn is_empty(&self) -> bool;
    fn first<'a>(&'a self) -> Option<(&'a K, &'a V)> where 'store: 'a;
    fn get<'a>(&'a self, key: &K) -> Option<&'a V> where 'store: 'a;
    fn iter<'a>(&'a self) -> Self::Iter<'a> where 'store: 'a;
    fn range<'a>(&'a self, range: std::ops::Range<K>) -> Self::Range<'a> where 'store: 'a;
}
// endregion

// region map implementation
macro_rules! impl_b_tree_map_common {
    ($store:lifetime, $K:ident, $V:ident) => {
        fn insert(&mut self, key: $K, value: $V) -> Option<$V> {
            self.insert(key, value)
        }

        fn remove_first(&mut self) -> Option<($K, $V)> {
            self.pop_first()
        }

        fn remove(&mut self, key: &$K) -> Option<$V> {
            self.remove(key)
        }

        fn is_empty(&self) -> bool {
            self.is_empty()
        }

        fn first<'a>(&'a self) -> Option<(&'a K, &'a V)> where $store: 'a {
            self.first_key_value()
        }

        fn get<'a>(&'a self, key: &$K) -> Option<&'a V> where $store: 'a {
            self.get(key)
        }

        fn iter<'a>(&'a self) -> Self::Iter<'a> where $store: 'a {
            self.iter()
        }

        fn range<'a>(&'a self, range: std::ops::Range<$K>) -> Self::Range<'a> where $store: 'a {
            self.range(range)
        }
    }
}

impl<'store, K: Ord + 'store, V: 'store> BTreeMap<'store, K, V> for StdBTreeMap<K, V> {
    type SharedStore = ();
    type Iter<'a> = std::collections::btree_map::Iter<'a, K, V> where 'store: 'a;
    type Range<'a> = std::collections::btree_map::Range<'a, K, V> where 'store: 'a;

    fn new_in(&(): &'store Self::SharedStore) -> Self {
        Self::new()
    }

    impl_b_tree_map_common!('store, K, V);
}

impl<'store, K: Clone + Ord + 'store, V: 'store> BTreeMap<'store, K, V> for MyBTreeMap<'store, K, V> {
    type SharedStore = BTreeStore<K, V>;
    type Iter<'a> = btree_plus_store::map::Iter<'a, K, V> where 'store: 'a;
    type Range<'a> = btree_plus_store::map::Range<'a, K, V> where 'store: 'a;

    fn new_in(store: &'store Self::SharedStore) -> Self {
        Self::new_in(store)
    }

    impl_b_tree_map_common!('store, K, V);
}
// endregion

//noinspection RsUnnecessaryQualifications (IntelliJ is bugged)
fn bench_operations<'store, T: BTreeMap<'store, usize, usize>, B: Bencher>(
    store: &'store T::SharedStore,
    b: &mut B,
    n_maps: usize,
    n_operations: usize
) {
    let mut rng = SmallRng::seed_from_u64(42);
    let mut maps = Vec::with_capacity(n_maps);

    b.iter(|| {
        // Create
        for _ in 0..n_maps {
            maps.push(T::new_in(store));
        }

        // Insert
        for map in &mut maps {
            for _ in 0..n_operations {
                B::black_box(map.insert(rng.gen(), rng.gen()));
            }
        }

        // Remove first
        for map in &mut maps {
            while !map.is_empty() {
                B::black_box(map.remove_first());
            }
        }

        // Insert (again)
        for map in &mut maps {
            for _ in 0..n_operations {
                B::black_box(map.insert(rng.gen_range(0..n_operations), rng.gen()));
            }
        }

        // Retrieve at key
        for map in &mut maps {
            for _ in 0..n_operations {
                B::black_box(map.get(&rng.gen_range(0..n_operations)));
            }
        }

        // Iterate all
        for map in &mut maps {
            for (&key, &value) in map.iter() {
                B::black_box((key, value));
            }
        }

        // Iterate range
        for map in &mut maps {
            let key0 = rng.gen_range(0..n_operations);
            let key1 = rng.gen_range(0..n_operations);
            let range = match key0 < key1 {
                false => key1..key0,
                true => key0..key1,
            };
            for (&key, &value) in map.range(range) {
                B::black_box((key, value));
            }
        }

        // Remove at key
        for map in &mut maps {
            for _ in 0..n_operations {
                B::black_box(map.remove(&rng.gen_range(0..n_operations)));
            }
        }

        // Destroy
        maps.clear();
    });
}

macro_rules! generate_bench_group {
    ($bench_name:ident: ($n_maps:literal, $n_operations:literal), {
        $($(#[$attr:meta])? $btree_map_name:ident: $btree_map_type:ty),* $(,)?
    }) => {
        #[cfg(feature = "bench")]
        fn $bench_name(c: &mut criterion::Criterion) {
            #[allow(unused_mut)]
            let mut group = c.benchmark_group(stringify!($bench_name));
            $(
                $(#[$attr])?
                group.bench_function(
                    stringify!($btree_map_name),
                    |b| bench_operations::<$btree_map_type, _>(&Default::default(), b, $n_maps, $n_operations)
                );
            )*
            group.finish();
        }

        #[cfg(not(feature = "bench"))]
        mod $bench_name {
            use super::*;

            $(
                #[test]
                fn $btree_map_name() {
                    bench_operations::<$btree_map_type, _>(&Default::default(), &mut MockBencher, $n_maps, $n_operations);
                }
            )*
        }
    }
}

macro_rules! generate_benches {
    ($($bench_name:ident: ($n_maps:literal, $n_operations:literal)),* $(,)?) => {
        #[cfg(feature = "bench")]
        criterion::criterion_group! {
            name = benches;
            config = criterion::Criterion::default().sample_size(sample_size());
            targets = $($bench_name),*
        }

        $(
            generate_bench_group!($bench_name: ($n_maps, $n_operations), {
                std_b_tree_map: StdBTreeMap<usize, usize>,
                my_b_tree_map: MyBTreeMap<usize, usize>,
            });
        )*
    };
}

#[cfg(feature = "bench")]
fn sample_size() -> usize {
    std::env::var("SAMPLE_SIZE")
        .ok().filter(|s| !s.is_empty())
        .map_or(10, |s| s.parse().expect("SAMPLE_SIZE must be an integer or unset"))
}

#[cfg(feature = "bench")]
criterion::criterion_main!(benches);
generate_benches! {
    bench_1_map_3000_operations: (1, 3000),
    bench_10_maps_300_operations: (10, 300),
    bench_100_maps_30_operations: (100, 30),
    bench_1000_maps_3_operations: (1000, 3),
}