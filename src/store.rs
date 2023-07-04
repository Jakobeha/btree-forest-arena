use crate::node::{Node, NodePtr};
use rustc_arena_modified::SlabArena;

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
        self.nodes.alloc(node).into_unsafe()
    }

    #[inline]
    pub(crate) fn dealloc(&self, node: NodePtr<K, V>) {
        unsafe { node.discard(&self.nodes) }
    }

    #[allow(unused)]
    #[inline]
    pub(crate) fn dealloc_and_return(&self, node: NodePtr<K, V>) -> Node<K, V> {
        unsafe { node.take(&self.nodes) }
    }
}

impl<K, V> Default for BTreeStore<K, V> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
