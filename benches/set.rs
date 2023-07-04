extern crate rand;

use std::collections::BTreeSet as StdBTreeSet;
use btree_forest_arena::{BTreeStore, BTreeSet as MyBTreeSet};

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

// region set abstraction
trait BTreeSet<'store, T: Ord + 'store>: 'store {
    /// `()` if the store is owned
    type SharedStore: Default;
    type Iter<'a>: Iterator<Item = &'a T> where 'store: 'a;
    type Range<'a>: Iterator<Item = &'a T> where 'store: 'a;

    fn new_in(store: &'store Self::SharedStore) -> Self;
    fn insert(&mut self, elem: T) -> bool;
    fn remove(&mut self, elem: &T) -> bool;
    fn remove_first(&mut self) -> Option<T>;
    fn is_empty(&self) -> bool;
    fn first<'a>(&'a self) -> Option<&'a T> where 'store: 'a;
    fn contains<'a>(&'a self, elem: &T) -> bool where 'store: 'a;
    fn iter<'a>(&'a self) -> Self::Iter<'a> where 'store: 'a;
    fn range<'a>(&'a self, range: std::ops::Range<T>) -> Self::Range<'a> where 'store: 'a;
}
// endregion

// region set implementation
macro_rules! impl_b_tree_set_common {
    ($store:lifetime, $T:ident) => {
        fn insert(&mut self, elem: $T) -> bool {
            self.insert(elem)
        }

        fn remove_first(&mut self) -> Option<$T> {
            self.pop_first()
        }

        fn remove(&mut self, elem: &$T) -> bool {
            self.remove(elem)
        }

        fn is_empty(&self) -> bool {
            self.is_empty()
        }

        fn first<'a>(&'a self) -> Option<&'a $T> where $store: 'a {
            self.first()
        }

        fn contains<'a>(&'a self, elem: &$T) -> bool where $store: 'a {
            self.contains(elem)
        }

        fn iter<'a>(&'a self) -> Self::Iter<'a> where $store: 'a {
            self.iter()
        }

        fn range<'a>(&'a self, range: std::ops::Range<$T>) -> Self::Range<'a> where $store: 'a {
            self.range(range)
        }
    }
}

impl<'store, T: Ord + 'store> BTreeSet<'store, T> for StdBTreeSet<T> {
    type SharedStore = ();
    type Iter<'a> = std::collections::btree_set::Iter<'a, T> where 'store: 'a;
    type Range<'a> = std::collections::btree_set::Range<'a, T> where 'store: 'a;

    fn new_in(&(): &'store Self::SharedStore) -> Self {
        Self::new()
    }

    impl_b_tree_set_common!('store, T);
}

impl<'store, T: Clone + Ord + 'store> BTreeSet<'store, T> for MyBTreeSet<'store, T> {
    type SharedStore = BTreeStore<T, ()>;
    type Iter<'a> = btree_forest_arena::set::Iter<'a, T> where 'store: 'a;
    type Range<'a> = btree_forest_arena::set::Range<'a, T> where 'store: 'a;

    fn new_in(store: &'store Self::SharedStore) -> Self {
        Self::new_in(store)
    }

    impl_b_tree_set_common!('store, T);
}
// endregion

//noinspection RsUnnecessaryQualifications (IntelliJ is bugged)
fn bench_operations<'store, T: BTreeSet<'store, usize>, B: Bencher>(
    store: &'store T::SharedStore,
    b: &mut B,
    n_sets: usize,
    n_operations: usize
) {
    let mut rng = SmallRng::seed_from_u64(42);
    let mut sets = Vec::with_capacity(n_sets);

    b.iter(|| {
        // Create
        for _ in 0..n_sets {
            sets.push(T::new_in(store));
        }

        // Insert
        for set in &mut sets {
            for _ in 0..n_operations {
                B::black_box(set.insert(rng.gen()));
            }
        }

        // Remove first
        for set in &mut sets {
            while !set.is_empty() {
                B::black_box(set.remove_first());
            }
        }

        // Insert (again)
        for set in &mut sets {
            for _ in 0..n_operations {
                B::black_box(set.insert(rng.gen_range(0..n_operations)));
            }
        }

        // Retrieve at key
        for set in &mut sets {
            for _ in 0..n_operations {
                B::black_box(set.contains(&rng.gen_range(0..n_operations)));
            }
        }

        // Iterate all
        for set in &mut sets {
            for &elem in set.iter() {
                B::black_box(elem);
            }
        }

        // Iterate range
        for set in &mut sets {
            let key0 = rng.gen_range(0..n_operations);
            let key1 = rng.gen_range(0..n_operations);
            let range = match key0 < key1 {
                false => key1..key0,
                true => key0..key1,
            };
            for &elem in set.range(range) {
                B::black_box(elem);
            }
        }

        // Remove at key
        for set in &mut sets {
            for _ in 0..n_operations {
                B::black_box(set.remove(&rng.gen_range(0..n_operations)));
            }
        }

        // Destroy
        sets.clear();
    });
}

macro_rules! generate_bench_group {
    ($bench_name:ident: ($n_sets:literal, $n_operations:literal), {
        $($(#[$attr:meta])? $btree_set_name:ident: $btree_set_type:ty),* $(,)?
    }) => {
        #[cfg(feature = "bench")]
        fn $bench_name(c: &mut criterion::Criterion) {
            #[allow(unused_mut)]
            let mut group = c.benchmark_group(stringify!($bench_name));
            $(
                $(#[$attr])?
                group.bench_function(
                    stringify!($btree_set_name),
                    |b| bench_operations::<$btree_set_type, _>(&Default::default(), b, $n_sets, $n_operations)
                );
            )*
            group.finish();
        }

        #[cfg(not(feature = "bench"))]
        mod $bench_name {
            use super::*;

            $(
                #[test]
                fn $btree_set_name() {
                    bench_operations::<$btree_set_type, _>(&Default::default(), &mut MockBencher, $n_sets, $n_operations);
                }
            )*
        }
    }
}

macro_rules! generate_benches {
    ($($bench_name:ident: ($n_sets:literal, $n_operations:literal)),* $(,)?) => {
        #[cfg(feature = "bench")]
        criterion::criterion_group! {
            name = benches;
            config = criterion::Criterion::default().sample_size(sample_size());
            targets = $($bench_name),*
        }

        $(
            generate_bench_group!($bench_name: ($n_sets, $n_operations), {
                std_b_tree_set: StdBTreeSet<usize>,
                my_b_tree_set: MyBTreeSet<usize>,
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
    bench_1_set_3000_operations: (1, 3000),
    bench_10_sets_300_operations: (10, 300),
    bench_100_sets_30_operations: (100, 30),
    bench_1000_sets_3_operations: (1000, 3),
}