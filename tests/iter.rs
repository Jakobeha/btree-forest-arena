use btree_plus_store::{BTreeMap, BTreeSet, BTreeStore};
use std::fmt::{Debug, Formatter};
use std::{cell::Cell, rc::Rc};

#[test]
pub fn iter() {
    let store = BTreeStore::new();
    let mut map = BTreeMap::new_in(&store);
    for i in 0..10 {
        map.insert(i, i);
    }

    let mut i = 0;
    for (&key, &value) in &map {
        assert_eq!(key, i);
        assert_eq!(value, i);
        i += 1;
    }

    assert_eq!(i, 10)
}

#[test]
pub fn into_iter() {
    struct Element {
        /// Drop counter.
        counter: Rc<Cell<usize>>,
        value: i32,
    }

    impl Element {
        pub fn new(counter: &Rc<Cell<usize>>, value: i32) -> Self {
            Element {
                counter: counter.clone(),
                value,
            }
        }

        pub fn inner(&self) -> i32 {
            self.value
        }
    }

    impl Debug for Element {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.value)
        }
    }

    impl Drop for Element {
        fn drop(&mut self) {
            let c = self.counter.get();
            self.counter.set(c + 1);
        }
    }

    let counter = Rc::new(Cell::new(0));
    let store = BTreeStore::new();
    let mut map = BTreeMap::new_in(&store);
    for i in 0..100 {
        map.insert(i, Element::new(&counter, i));
    }

    println!("{:?}", map);

    for (key, value) in map {
        assert_eq!(key, value.inner());
    }

    assert_eq!(counter.get(), 100);
}

#[test]
pub fn into_iter_rev() {
    struct Element {
        /// Drop counter.
        counter: Rc<Cell<usize>>,
        value: i32,
    }

    impl Element {
        pub fn new(counter: &Rc<Cell<usize>>, value: i32) -> Self {
            Element {
                counter: counter.clone(),
                value,
            }
        }

        pub fn inner(&self) -> i32 {
            self.value
        }
    }

    impl Drop for Element {
        fn drop(&mut self) {
            let c = self.counter.get();
            self.counter.set(c + 1);
        }
    }

    let counter = Rc::new(Cell::new(0));
    let store = BTreeStore::new();
    let mut map = BTreeMap::new_in(&store);
    for i in 0..100 {
        map.insert(i, Element::new(&counter, i));
    }

    for (key, value) in map.into_iter().rev() {
        assert_eq!(key, value.inner());
    }

    assert_eq!(counter.get(), 100);
}

#[test]
pub fn into_iter_both_ends1() {
    struct Element {
        /// Drop counter.
        counter: Rc<Cell<usize>>,
        value: i32,
    }

    impl Element {
        pub fn new(counter: &Rc<Cell<usize>>, value: i32) -> Self {
            Element {
                counter: counter.clone(),
                value,
            }
        }

        pub fn inner(&self) -> i32 {
            self.value
        }
    }

    impl Drop for Element {
        fn drop(&mut self) {
            let c = self.counter.get();
            self.counter.set(c + 1);
        }
    }

    let counter = Rc::new(Cell::new(0));
    let store = BTreeStore::new();
    let mut map = BTreeMap::new_in(&store);
    for i in 0..100 {
        map.insert(i, Element::new(&counter, i));
    }

    let mut it = map.into_iter();
    while let Some((key, value)) = it.next() {
        assert_eq!(key, value.inner());

        let (key, value) = it.next_back().unwrap();
        assert_eq!(key, value.inner());
    }

    assert_eq!(counter.get(), 100);
}

#[test]
pub fn into_iter_both_ends2() {
    struct Element {
        /// Drop counter.
        counter: Rc<Cell<usize>>,
        value: i32,
    }

    impl Element {
        pub fn new(counter: &Rc<Cell<usize>>, value: i32) -> Self {
            Element {
                counter: counter.clone(),
                value,
            }
        }

        pub fn inner(&self) -> i32 {
            self.value
        }
    }

    impl Drop for Element {
        fn drop(&mut self) {
            let c = self.counter.get();
            self.counter.set(c + 1);
        }
    }

    let counter = Rc::new(Cell::new(0));
    let store = BTreeStore::new();
    let mut map = BTreeMap::new_in(&store);
    for i in 0..100 {
        map.insert(i, Element::new(&counter, i));
    }

    let mut it = map.into_iter();
    while let Some((key, value)) = it.next_back() {
        assert_eq!(key, value.inner());

        let (key, value) = it.next().unwrap();
        assert_eq!(key, value.inner());
    }

    assert_eq!(counter.get(), 100);
}

#[test]
fn iter_mut() {
    let store = BTreeStore::new();
    let mut map = BTreeMap::new_in(&store);

    for i in 0..10 {
        map.insert(i, i);
    }

    for (k, v) in map.iter_mut() {
        *v += *k;
    }

    for i in 0..10 {
        assert_eq!(map.get(&i).unwrap(), &(i * 2));
    }
}

#[test]
fn keys() {
    let store = BTreeStore::new();
    let mut map = BTreeMap::new_in(&store);

    for i in 0..10 {
        map.insert(i, i);
    }

    let mut keys = map.keys();

    for i in 0..10 {
        assert_eq!(keys.next(), Some(&i));
    }

    assert_eq!(keys.next(), None);
}

#[test]
fn values() {
    let store = BTreeStore::new();
    let mut map = BTreeMap::new_in(&store);

    for i in 0..10 {
        map.insert(i, i);
    }

    let mut values = map.values();

    for i in 0..10 {
        assert_eq!(values.next(), Some(&i));
    }

    assert_eq!(values.next(), None);
}

#[test]
fn set_iter() {
    let store = BTreeStore::new();
    let mut set = BTreeSet::new_in(&store);

    for i in 0..10 {
        set.insert(i);
    }

    let mut i = 0;
    for value in &set {
        assert_eq!(value, &i);
        i += 1;
    }

    assert_eq!(i, 10);
}

#[test]
fn set_into_iter() {
    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct Element {
        counter: Rc<Cell<usize>>,
        value: i32,
    }

    impl Drop for Element {
        fn drop(&mut self) {
            let c = self.counter.get();
            self.counter.set(c + 1);
        }
    }

    let counter = Rc::new(Cell::new(0));
    let store = BTreeStore::new();
    let mut set = BTreeSet::new_in(&store);

    for i in 0..100 {
        set.insert(Element {
            counter: counter.clone(),
            value: i,
        });
    }

    for value in set {
        assert!(value.value < 100);
    }

    assert_eq!(counter.get(), 100);
}
