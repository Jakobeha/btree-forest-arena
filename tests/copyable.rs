#![cfg(feature = "copyable")]

use btree_plus_store::{copyable, BTreeMap, BTreeStore};
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn test_copy() {
    let store = BTreeStore::new();

    let mut map = BTreeMap::new_in(&store);
    map.insert(1, 10);
    map.insert(2, 20);

    let map = copyable::BTreeMap::from(map);
    let map2 = map;

    assert_eq!(map.len(), 2);
    assert_eq!(map2.len(), 2);

    assert_eq!(map.get(&1), Some(&10));
    assert_eq!(map2.get(&2), Some(&20));
}

#[test]
fn test_large_map() {
    let store = BTreeStore::new();

    let mut map = BTreeMap::new_in(&store);
    for i in 0..1000 {
        map.insert(i, i * 10);
    }

    let map = copyable::BTreeMap::from(map);

    assert_eq!(map.len(), 1000);
    for i in 0..1000 {
        assert_eq!(map.get(&i), Some(&(i * 10)));
    }
}

#[test]
fn test_copy_contains() {
    let store = BTreeStore::new();

    let mut map = BTreeMap::new_in(&store);
    for i in 0..100 {
        map.insert(i * 10, i * 10);
    }

    let map = copyable::BTreeMap::from(map);
    let map2 = map;

    assert!(map.contains_key(&300));
    assert!(!map.contains_key(&301));
    assert!(map2.contains_key(&300));
    assert!(!map2.contains_key(&301));
}

#[test]
fn test_concurrent_iter() {
    let store = BTreeStore::new();

    let mut map = BTreeMap::new_in(&store);
    map.insert(2, 30);
    map.insert(1, 20);
    map.insert(3, 10);

    let map = copyable::BTreeMap::from(map);
    let map2 = map;

    let keys = map.keys();
    let keys2 = map2.keys().rev();
    let keys = keys.collect::<Vec<_>>();
    let keys2 = keys2.collect::<Vec<_>>();

    assert_eq!(keys, vec![&1, &2, &3]);
    assert_eq!(keys2, vec![&3, &2, &1]);
}

#[test]
fn test_create_after_drop() {
    let store = BTreeStore::new();

    {
        let mut map = BTreeMap::new_in(&store);
        map.insert(1, 10);
    }

    let map = copyable::BTreeMap::from(BTreeMap::new_in(&store));

    assert!(map.is_empty());
}

#[test]
fn test_drop_contents() {
    struct DropCounter {
        drop_count: Rc<Cell<usize>>,
    }

    impl DropCounter {
        fn new(drop_count: &Rc<Cell<usize>>) -> Self {
            Self {
                drop_count: drop_count.clone(),
            }
        }
    }

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.drop_count.set(self.drop_count.get() + 1);
        }
    }

    let store = BTreeStore::new();
    let drop_count = Rc::new(Cell::new(0));

    let mut map = BTreeMap::new_in(&store);
    map.insert(1, DropCounter::new(&drop_count));
    map.insert(2, DropCounter::new(&drop_count));

    assert_eq!(drop_count.get(), 0);
    assert!(map.contains_key(&1));
    assert!(map.contains_key(&2));

    let mut map2 = BTreeMap::new_in(&store);
    map2.insert(3, DropCounter::new(&drop_count));
    map2.insert(4, DropCounter::new(&drop_count));
    map2.insert(5, DropCounter::new(&drop_count));
    let map2 = copyable::BTreeMap::from(map2);
    let map3 = map2;

    assert_eq!(drop_count.get(), 0);
    assert!(map2.contains_key(&3));
    assert!(map2.contains_key(&4));
    assert!(map2.contains_key(&5));

    drop(map2);
    assert_eq!(drop_count.get(), 0);
    drop(map);
    assert_eq!(drop_count.get(), 2);
    drop(map3);
    assert_eq!(drop_count.get(), 2);
}
