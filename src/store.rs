use std::ptr::NonNull;
use rustc_arena_modified::slab_arena::UnsafeRef;
use rustc_arena_modified::SlabArena;
use crate::node::{NodePtr, Node};

/// Arena to store nodes from multiple b-trees.
pub struct BTreeStore<K, V> {
    nodes: SlabArena<Node<K, V>>,
}

impl<K, V> BTreeStore<K, V> {
    #[inline]
    pub fn new() -> Self {
        Self {
            nodes: SlabArena::new(),
        }
    }

    #[inline]
    pub(crate) fn alloc(&self, node: Node<K, V>) -> NodePtr<K, V> {
        NonNull::from(self.nodes.alloc(node).leak())
    }

    #[inline]
    pub(crate) fn dealloc(&self, node: NodePtr<K, V>) {
        unsafe { UnsafeRef::from_ptr(node).discard(&self.nodes) }
    }

    #[allow(unused)]
    #[inline]
    pub(crate) fn dealloc_and_return(&self, node: NodePtr<K, V>) -> Node<K, V> {
        unsafe { UnsafeRef::from_ptr(node).take(&self.nodes) }
    }
}

impl<K, V> Default for BTreeStore<K, V> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}