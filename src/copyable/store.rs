use std::collections::HashSet;

use crate::node::NodePtr;
use crate::BTreeStore;

/// Extension to tracing garbage-collect nodes in a store
pub trait BTreeStoreExt<K, V> {
    /// Remove all allocated nodes which are not reachable through `b_trees` iterator.
    ///
    /// # Safety
    /// `b_trees` *must* return b-trees containing all reachable nodes in the store, AKA there must
    /// not exist a b-tree with this store which is not in `b_trees`. Any nodes not reachable through
    /// `b_trees` will be dropped.
    unsafe fn tracing_gc<'a>(&self, btrees: impl IntoIterator<Item = impl BTree<'a, K, V>>)
    where
        K: 'a,
        V: 'a;

    // TODO: Async or background version which does [tri-color marking](https://en.wikipedia.org/wiki/Tracing_garbage_collection#Tri-color_marking)
}

/// Generic trait for different b-tree maps and sets, which returns reachable nodes.
///
/// This trait is [sealed](https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed)
pub trait BTree<'store, K, V>: crate::copyable::sealed::BTree<'store, K, V> {}

impl<K, V> BTreeStoreExt<K, V> for BTreeStore<K, V> {
    #[inline]
    unsafe fn tracing_gc<'a>(&self, b_trees: impl IntoIterator<Item = impl BTree<'a, K, V>>)
    where
        K: 'a,
        V: 'a,
    {
        let nodes = b_trees
            .into_iter()
            .flat_map(|b_tree| {
                b_tree.assert_store(self);
                b_tree.nodes()
            })
            .collect::<HashSet<_>>();
        self.retain_shared(|node| nodes.contains(&NodePtr::from_ref(node)));
    }
}
