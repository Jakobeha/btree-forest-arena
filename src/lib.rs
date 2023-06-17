#![doc = include_str("../README.md")]
pub mod generic;
pub mod utils;
#[cfg(any(doc, feature = "shareable-slab"))]
pub mod shareable_slab;

/// B-Tree map based on `Slab`.
#[cfg(any(doc, feature = "slab"))]
pub type BTreeMap<K, V> = generic::BTreeMap<K, V, usize, slab::Slab<generic::Node<K, V, usize>>>;

/// B-Tree set based on `Slab`.
#[cfg(any(doc, feature = "slab"))]
pub type BTreeSet<T> = generic::BTreeSet<T, usize, slab::Slab<generic::Node<T, (), usize>>>;

/// B-Tree map based on `ShareableSlab`.
#[cfg(any(doc, feature = "shareable-slab"))]
pub type SharingBTreeMap<'a, K, V> = generic::BTreeMap<K, V, usize, &'a shareable_slab::ShareableSlab<generic::Node<K, V, usize>>>;

/// B-Tree set based on `ShareableSlab`.
#[cfg(any(doc, feature = "shareable-slab"))]
pub type SharingBTreeSet<'a, T> = generic::BTreeSet<T, usize, &'a shareable_slab::ShareableSlab<generic::Node<T, (), usize>>>;
