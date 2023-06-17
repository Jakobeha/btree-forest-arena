/// B-Tree map based on `Slab`.
pub type BTreeMap<K, V> = crate::generic::BTreeMap<K, V, usize, slab::Slab<crate::generic::Node<K, V, usize>>>;

/// B-Tree set based on `Slab`.
pub type BTreeSet<T> = crate::generic::BTreeSet<T, usize, slab::Slab<crate::generic::Node<T, (), usize>>>;