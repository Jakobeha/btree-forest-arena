//! These tests were generated with the help of GPT-3.5 (only gave other tests as input), although
//! they needed a lot of fixing

use std::thread;

#[test]
#[cfg(feature = "shareable-slab")]
fn shared_between_3() {
    let slab = btree_store::shareable_slab::ShareableSlab::new();

    let mut map1 = btree_store::shareable_slab::BTreeMap::new_in(&slab);
    let mut map2 = btree_store::shareable_slab::BTreeMap::new_in(&slab);
    let mut map3 = btree_store::shareable_slab::BTreeMap::new_in(&slab);

    // Insert key-value pairs into map1
    map1.insert(1, "One");
    map1.insert(2, "Two");
    map1.insert(3, "Three");

    // Insert key-value pairs into map2
    map2.insert(1, "Uno");
    map2.insert(2, "Dos");
    map2.insert(3, "Tres");

    // Insert key-value pairs into map3
    map3.insert(1, "Eins");
    map3.insert(2, "Zwei");
    map3.insert(3, "Drei");

    // Verify that the maps have the correct values
    assert_eq!(map1.get(&1).map(|x| *x), Some("One"));
    assert_eq!(map2.get(&1).map(|x| *x), Some("Uno"));
    assert_eq!(map3.get(&1).map(|x| *x), Some("Eins"));

    assert_eq!(map1.get(&2).map(|x| *x), Some("Two"));
    assert_eq!(map2.get(&2).map(|x| *x), Some("Dos"));
    assert_eq!(map3.get(&2).map(|x| *x), Some("Zwei"));

    assert_eq!(map1.get(&3).map(|x| *x), Some("Three"));
    assert_eq!(map2.get(&3).map(|x| *x), Some("Tres"));
    assert_eq!(map3.get(&3).map(|x| *x), Some("Drei"));


}

#[test]
#[cfg(feature = "concurrent-shareable-slab")]
fn shared_concurrently() {
    // Create a shareable slab
    let slab = btree_store::concurrent_shareable_slab::ShareableSlab::new();

    // Create a vector to hold the maps
    let mut maps = Vec::new();

    // Spawn multiple threads, each working with their own map
    for _ in 0..5 {
        let map = btree_store::concurrent_shareable_slab::BTreeMap::new_in(&slab);
        maps.push(map);
    }

    thread::scope(|scope| {
        for map in &mut maps {
            // Insert key-value pairs into the map
            scope.spawn(|| {
                for i in 0..10 {
                    map.insert(i, i.to_string());
                }
            });
        }
    });

    // Verify that the maps have the correct values
    for map in &maps {
        for i in 0..10 {
            assert_eq!(map.get(&i).map(|x| x.clone()), Some(i.to_string()));
        }
    }
}