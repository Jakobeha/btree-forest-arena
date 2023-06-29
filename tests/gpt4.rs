/*//! These tests were generated with the help of GPT-4 (only gave other tests as input), although
//! they needed a lot of fixing
use std::thread;

use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;
use btree_forest_arena::{BTreeMap, BTreeSet, BTreeStore};

use btree_store::concurrent_shareable_slab::{BTreeMap, BTreeSet, Store};

const ELEMS: [usize; 10] = [
    12,
    23,
    34,
    45,
    56,
    67,
    78,
    89,
    910,
    1011,
];

const KV_ELEMS: [(usize, &'static str); 10] = [
    (12, "twelve"),
    (23, "twenty-three"),
    (34, "thirty-four"),
    (45, "forty-five"),
    (56, "fifty-six"),
    (67, "sixty-seven"),
    (78, "seventy-eight"),
    (89, "eighty-nine"),
    (910, "nine-hundred-ten"),
    (1011, "one-thousand-eleven"),
];

#[test]
fn shared_between_maps_and_sets_same_thread() {
    let store = BTreeStore::new();

    let mut map = BTreeMap::new_in(&store);
    let mut set = BTreeSet::new_in(&store);

    for &elem in ELEMS.iter() {
        map.insert(elem, ());
        set.insert(elem);
    }

    assert_eq!(map.len(), set.len());

    for (idx, key) in set.iter().enumerate() {
        // since ELEMS is sorted
        assert_eq!(ELEMS[idx], *key);
        assert!(map.contains_key(&*key));
    }

    let random_key = rand::thread_rng().gen_range(0..ELEMS.len());

    map.remove(&random_key);
    set.remove(&random_key);

    assert!(!set.contains(&random_key));

}

#[test]
fn shared_between_maps_and_sets_concurrent_threads() {
    let store = BTreeStore::new();

    thread::scope(|scope| {
        let map_thread = scope.spawn(|| {
            let mut map = BTreeMap::new_in(&store);

            for &elem in ELEMS.iter() {
                map.insert(elem, ());
            }

            assert_eq!(map.len(), ELEMS.len());

        });

        let set_thread = scope.spawn(|| {
            let mut set = BTreeSet::new_in(&store);

            for &elem in ELEMS.iter() {
                set.insert(elem);
            }

            assert_eq!(set.len(), ELEMS.len());

        });

        map_thread.join().expect("map thread crashed");
        set_thread.join().expect("set thread crashed");

    });
}

#[test]
fn shared_concurrent_maps_insertion_and_removal() {
    let store = BTreeStore::new();

    thread::scope(|scope| {
        let map1_thread = scope.spawn(|| {
            let mut rng = SmallRng::from_seed([66; 32]);
            let mut map1 = BTreeMap::new_in(&store);

            for (key, value) in KV_ELEMS.iter() {
                map1.insert(*key, *value);
            }

            for _ in 0..1000 {
                let random_key = (rng.gen_range(1..11), rng.gen_range(0..KV_ELEMS.len()));
                map1.insert(random_key.0, KV_ELEMS[random_key.1].1);
                map1.remove(&random_key.0);
            }

            assert_eq!(map1.len(), KV_ELEMS.len());

        });

        let map2_thread = scope.spawn(|| {
            let mut rng = SmallRng::from_seed([99; 32]);
            let mut map2 = BTreeMap::new_in(&store);

            for (key, value) in KV_ELEMS.iter() {
                map2.insert(*key, *value);
            }

            for _ in 0..1000 {
                let random_key_value = (rng.gen_range(1..11), rng.gen_range(0..KV_ELEMS.len()));
                map2.insert(random_key_value.0, KV_ELEMS[random_key_value.1].1);
                map2.remove(&random_key_value.0);
            }

            assert_eq!(map2.len(), KV_ELEMS.len());

        });

        map1_thread.join().expect("map1 thread crashed");
        map2_thread.join().expect("map2 thread crashed");

    });
}

#[test]
fn shared_maps_set_clear_same_thread() {
    let store = BTreeStore::new();

    let mut map1 = BTreeMap::new_in(&store);
    let mut map2 = BTreeMap::new_in(&store);
    let mut set = BTreeSet::new_in(&store);

    for &elem in ELEMS.iter() {
        map1.insert(elem, ());
        map2.insert(elem, ());
        set.insert(elem);
    }

    assert_eq!(map1.len(), map2.len());
    assert_eq!(map2.len(), set.len());

    map1.clear();
    assert!(map1.is_empty());
    assert!(!map2.is_empty());
    assert!(!set.is_empty());

    map2.clear();
    assert!(map1.is_empty());
    assert!(map2.is_empty());
    assert!(!set.is_empty());

    set.clear();
    assert!(map1.is_empty());
    assert!(map2.is_empty());
    assert!(set.is_empty())
}

#[test]
fn shared_maps_entry_api_same_thread() {
    let store = BTreeStore::new();

    let mut map1 = BTreeMap::new_in(&store);

    for (key, value) in KV_ELEMS.iter() {
        map1.insert(*key, *value);
    }

    // Test entry API
    const NON_EXISTENT_KEY: usize = 50;
    const DEFAULT_VALUE: &str = "default";

    let value_ref = map1.entry(NON_EXISTENT_KEY).or_insert(DEFAULT_VALUE);

    assert_eq!(*value_ref, DEFAULT_VALUE);
}

#[test]
fn shared_maps_entry_api_concurrent_threads() {
    let store = BTreeStore::new();

    thread::scope(|scope| {
        let map_thread1 = scope.spawn(|| {
            let mut map1 = BTreeMap::new_in(&store);

            for (key, value) in KV_ELEMS.iter() {
                map1.insert(*key, *value);
            }

            const NEW_KEY: usize = 25;
            const NEW_VALUE: &str = "twenty-five";

            let _ = map1.entry(NEW_KEY).or_insert(NEW_VALUE);

            assert_eq!(map1.get(&NEW_KEY).map(|x| *x), Some(NEW_VALUE));

            let value_ref = map1.entry(2).or_insert("two");
            assert_eq!(*value_ref, "two");
        });

        let map_thread2 = scope.spawn(|| {
            let mut map2 = BTreeMap::new_in(&store);

            for (key, value) in KV_ELEMS.iter() {
                map2.insert(*key, *value);
            }

            const NEW_KEY: usize = 50;
            const NEW_VALUE: &str = "fifty";

            let _ = map2.entry(NEW_KEY).or_insert(NEW_VALUE);

            assert_eq!(map2.get(&NEW_KEY).map(|x| *x), Some(NEW_VALUE));


            let value_ref = map2.entry(4).or_insert("four");
            assert_eq!(*value_ref, "four");
        });

        map_thread1.join().expect("map1 thread crashed");
        map_thread2.join().expect("map2 thread crashed");

    });
}

#[test]
fn shared_sets_operations_same_thread() {
    let store = BTreeStore::new();

    let mut set1 = BTreeSet::new_in(&store);
    let mut set2 = BTreeSet::new_in(&store);

    for &elem in ELEMS.iter() {
        set1.insert(elem);
    }

    for i in 20..50 {
        set2.insert(i);
    }

    let set_intersection_1_2 = set1.intersection(&set2).map(|x| *x);
    let set_union_1_2 = set1.union(&set2).map(|x| *x);
    let set_difference_1_2 = set1.difference(&set2).map(|x| *x);
    let set_difference_2_1 = set2.difference(&set1).map(|x| *x);
    let set_difference_symmetric = set2.symmetric_difference(&set1).map(|x| *x);

    // We can concurrently iterate over the sets, as long as there is no mutation
    assert_eq!(
        set_intersection_1_2.collect::<Vec<_>>(),
        [23, 34, 45]
    );
    assert_eq!(
        set_union_1_2.collect::<Vec<_>>(),
        [12].into_iter()
            .chain(20..50)
            .chain([56, 67, 78, 89, 910, 1011])
            .collect::<Vec<_>>()
    );
    assert_eq!(
        set_difference_1_2.collect::<Vec<_>>(),
        [12, 56, 67, 78, 89, 910, 1011]
    );
    assert_eq!(
        set_difference_2_1.collect::<Vec<_>>(),
        (20..50).filter(|&x| ![23, 34, 45].contains(&x)).collect::<Vec<_>>()
    );
    assert_eq!(
        set_difference_symmetric.collect::<Vec<_>>(),
        [12].into_iter()
            .chain((20..50).filter(|&x| ![23, 34, 45].contains(&x)))
            .chain([56, 67, 78, 89, 910, 1011])
            .collect::<Vec<_>>()
    )
}*/