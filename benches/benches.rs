extern crate rand;

use std::collections::BTreeMap as StdBTreeMap;
use std::ops::Deref;

use rand::{Rng, rngs::SmallRng, SeedableRng};

#[cfg(feature = "concurrent-shareable-slab")]
use btree_store::concurrent_shareable_slab::BTreeMap as ConcurrentShareableSlabBTreeMap;
use btree_store::generic::{Node, StoreView};
use btree_store::generic::map::KeyValueRef;
use btree_store::generic::store::Index;
#[cfg(feature = "shareable-slab")]
use btree_store::shareable_slab::BTreeMap as ShareableSlabBTreeMap;
#[cfg(feature = "shareable-slab-arena")]
use btree_store::shareable_slab_arena::BTreeMap as ShareableSlabArenaBTreeMap;
#[cfg(feature = "shareable-slab-simultaneous-mutation")]
use btree_store::shareable_slab_simultaneous_mutation::BTreeMap as ShareableSlabSimultaneousMutationBTreeMap;
#[cfg(feature = "slab")]
use btree_store::slab::BTreeMap as SlabBTreeMap;

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
trait Entry<'a, K, V> {
    fn key(&self) -> &K;
    fn value(&self) -> &V;
}

impl<'a, K, V> Entry<'a, K, V> for (&'a K, &'a V) {
    fn key(&self) -> &K {
        self.0
    }

    fn value(&self) -> &V {
        self.1
    }
}

impl<'a, K, V, I: Index, C: StoreView<Node<K, V, I>, Index=I>> Entry<'a, K, V> for KeyValueRef<'a, K, V, I, C> {
    fn key(&self) -> &K {
        self.key()
    }

    fn value(&self) -> &V {
        self.value()
    }
}

trait BTreeMap<'store, K: Ord + 'store, V: 'store>: 'store {
    /// `()` if owned
    type SharedStore: Default;
    type Entry<'a>: Entry<'a, K, V> where 'store: 'a;
    type Ref<'a>: Deref<Target=V> where 'store: 'a;
    type Iter<'a>: Iterator<Item = Self::Entry<'a>> where 'store: 'a;
    type Range<'a>: Iterator<Item = Self::Entry<'a>> where 'store: 'a;

    fn new_in(store: &'store Self::SharedStore) -> Self;
    fn insert(&mut self, key: K, value: V) -> Option<V>;
    fn remove(&mut self, key: &K) -> Option<V>;
    fn remove_first(&mut self) -> Option<(K, V)>;
    fn is_empty(&self) -> bool;
    fn first<'a>(&'a self) -> Option<Self::Entry<'a>> where 'store: 'a;
    fn get<'a>(&'a self, key: &K) -> Option<Self::Ref<'a>> where 'store: 'a;
    fn iter<'a>(&'a self) -> Self::Iter<'a> where 'store: 'a;
    fn range<'a>(&'a self, range: std::ops::Range<K>) -> Self::Range<'a> where 'store: 'a;
}
// endregion

// region map implementation
macro_rules! impl_b_tree_map_new_in {
    ($Ident:ident) => {
        fn new_in(_: &Self::SharedStore) -> Self {
            $Ident::new()
        }
    };
    ($Ident:ident<$store:lifetime>) => {
        fn new_in(store: &$store Self::SharedStore) -> Self {
            $Ident::new_in(store)
        }
    };
}

macro_rules! impl_b_tree_map_ref {
    (Entry, $a:lifetime, $K:ident, $V:ident, $I:ty, $C:ty) => {
        btree_store::generic::map::KeyValueRef<$a, $K, $V, $I, $C>
    };
    (Ref, $a:lifetime, $K:ident, $V:ident, $I:ty, $C:ty) => {
        btree_store::generic::map::ValueRef<$a, $K, $V, $I, $C>
    };
    (Entry, $a:lifetime, $K:ident, $V:ident) => {
        (&$a $K, &$a $V)
    };
    (Ref, $a:lifetime, $K:ident, $V:ident) => {
        &$a $V
    };
}

macro_rules! impl_b_tree_map {
    ($Ident:ident, $($package:ident)::+) => {
        impl_b_tree_map!(<'store, K, V> $Ident, $($package)::+, ());
    };
    ($Ident:ident, $($package:ident)::+, $I:ty, $($C:ident)::+) => {
        impl_b_tree_map!(<'store, K, V> $Ident, $($package)::+, (), $I, $($C)::+<Node<K, V, $I>>);
    };
    ($Ident:ident, $($package:ident)::+, $I:ty, &'store $($C:ident)::+) => {
        impl_b_tree_map!(<'store, K, V> $Ident<'store>, $($package)::+, $($C)::+<Node<K, V, $I>>, $I, &'store $($C)::+<Node<K, V, $I>>);
    };
    (<$store:lifetime, $K:ident, $V:ident> $Ident:ident $(<$store2:lifetime>)?, $($package:ident)::+, $SharedStore:ty $(, $I:ty, $C:ty)?) => {
        impl<$store, $K: Ord + $store, $V: $store> BTreeMap<$store, $K, $V> for $Ident<$($store2,)? $K, $V> {
            type SharedStore = $SharedStore;
            type Entry<'a> = impl_b_tree_map_ref!(Entry, 'a, $K, $V $(, $I, $C)?) where $store: 'a;
            type Ref<'a> = impl_b_tree_map_ref!(Ref, 'a, $K, $V $(, $I, $C)?) where $store: 'a;
            type Iter<'a> = $($package)::+::Iter<'a, $K, $V $(, $I, $C)?> where $store: 'a;
            type Range<'a> = $($package)::+::Range<'a, $K, $V $(, $I, $C)?> where $store: 'a;

            impl_b_tree_map_new_in!($Ident$(<$store2>)?);

            fn insert(&mut self, key: $K, value: $V) -> Option<$V> {
                $Ident::insert(self, key, value)
            }

            fn remove_first(&mut self) -> Option<($K, $V)> {
                $Ident::pop_first(self)
            }

            fn remove(&mut self, key: &$K) -> Option<$V> {
                $Ident::remove(self, key)
            }

            fn is_empty(&self) -> bool {
                $Ident::is_empty(self)
            }

            fn first<'a>(&'a self) -> Option<Self::Entry<'a>> where $store: 'a {
                $Ident::first_key_value(self)
            }

            fn get<'a>(&'a self, key: &$K) -> Option<Self::Ref<'a>> where $store: 'a {
                $Ident::get(self, key)
            }

            fn iter<'a>(&'a self) -> Self::Iter<'a> where $store: 'a {
                $Ident::iter(self)
            }

            fn range<'a>(&'a self, range: std::ops::Range<$K>) -> Self::Range<'a> where $store: 'a {
                $Ident::range(self, range)
            }
        }
    }
}

impl_b_tree_map!(StdBTreeMap, std::collections::btree_map);
#[cfg(feature = "slab")]
impl_b_tree_map!(SlabBTreeMap, btree_store::generic::map, usize, slab::Slab);
#[cfg(feature = "shareable-slab")]
impl_b_tree_map!(ShareableSlabBTreeMap, btree_store::generic::map, usize, &'store btree_store::shareable_slab::Store);
#[cfg(feature = "concurrent-shareable-slab")]
impl_b_tree_map!(ConcurrentShareableSlabBTreeMap, btree_store::generic::map, usize, &'store btree_store::concurrent_shareable_slab::Store);
#[cfg(feature = "shareable-slab-simultaneous-mutation")]
impl_b_tree_map!(ShareableSlabSimultaneousMutationBTreeMap, btree_store::generic::map, usize, &'store btree_store::shareable_slab_simultaneous_mutation::Store);
#[cfg(feature = "shareable-slab-arena")]
impl_b_tree_map!(ShareableSlabArenaBTreeMap, btree_store::generic::map, btree_store::shareable_slab_arena::Index, &'store btree_store::shareable_slab_arena::Store);
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
            for kv in map.iter() {
                B::black_box((kv.key(), kv.value()));
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
            for kv in map.range(range) {
                B::black_box((kv.key(), kv.value()));
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
                #[cfg(feature = "slab")]
                slab_b_tree_map: SlabBTreeMap<usize, usize>,
                #[cfg(feature = "shareable-slab")]
                shareable_slab_b_tree_map: ShareableSlabBTreeMap<'_, usize, usize>,
                #[cfg(feature = "concurrent-shareable-slab")]
                concurrent_shareable_slab_b_tree_map: ConcurrentShareableSlabBTreeMap<'_, usize, usize>,
                #[cfg(feature = "shareable-slab-simultaneous-mutation")]
                shareable_slab_simultaneous_mutation_b_tree_map: ShareableSlabSimultaneousMutationBTreeMap<'_, usize, usize>,
                #[cfg(feature = "shareable-slab-arena")]
                shareable_slab_arena_b_tree_map: ShareableSlabArenaBTreeMap<'_, usize, usize>,
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