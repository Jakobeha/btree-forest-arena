use std::cmp::Ordering;
use std::marker::PhantomData;

use crate::node::{Node, NodePtr};
use crate::BTreeStore;

#[doc(hidden)]
pub trait BTree<'store, K, V> {
    fn assert_store(&self, store: &BTreeStore<K, V>);
    fn nodes(&self) -> NodeIter<'store, K, V>;
}

/// Does a pre-order traversal of all nodes (*not* entries) in the tree.
#[doc(hidden)]
pub struct NodeIter<'store, K, V> {
    current: Option<NodePtr<K, V>>,
    current_height: usize,
    max_height: usize,
    _p: PhantomData<&'store Node<K, V>>,
}

impl<'store, K, V> NodeIter<'store, K, V> {
    #[inline]
    pub(crate) fn new(root: Option<NodePtr<K, V>>, height: usize) -> Self {
        Self {
            current: root,
            current_height: height,
            max_height: height,
            _p: PhantomData,
        }
    }
}

impl<'store, K, V> Iterator for NodeIter<'store, K, V> {
    type Item = NodePtr<K, V>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Some(next) = self.current.take() else {
            return None;
        };

        // Advance.
        // To get all nodes:
        // - If we're at an internal node, go to its first leaf
        // - If we're at a leaf: we've already iterated all this node's internal parents, but we
        //   haven't iterated this node's next sibling, or (if the node is the last sibling) its
        //   parent's next sibling, etc. Furthermore, these siblings and their children are *all*
        //   the nodes we haven't yet iterated (we've already iterated the parents as mentioned, and
        //   we've already iterated the previous siblings because we did "choose next-sibling" to
        //   get here), so if there is no next sibling, parent next sibling, etc. we're done. So, go
        //   up until we find this next "ancestor sibling", or if there is none, break.
        if self.current_height > 0 {
            self.current = Some(unsafe { next.as_ref().edge(0) });
            self.current_height -= 1;
        } else {
            let mut node = next;
            self.current = loop {
                match self.current_height.cmp(&self.max_height) {
                    Ordering::Less => {
                        self.current_height += 1;
                        node = unsafe { node.as_ref().parent.unwrap() };
                        let index = unsafe { node.as_ref().parent_idx.assume_init() };
                        if index < unsafe { node.as_ref() }.len {
                            // Remember: we've already traversed this node and its children at `index`s
                            // going down. But we haven't traversed its next child at `index + 1`...
                            break Some(unsafe { node.as_ref().edge(index + 1) });
                        }
                    }
                    Ordering::Equal => break None,
                    Ordering::Greater => unreachable!(),
                }
            }
        }

        Some(next)
    }
}
