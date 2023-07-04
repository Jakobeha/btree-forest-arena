use std::mem::{ManuallyDrop, MaybeUninit, swap};
use std::ops::{Bound, RangeBounds};
use std::ptr::{copy, copy_nonoverlapping};

use rustc_arena_modified::slab_arena::UnsafeRef;

use crate::utils::{maybe_uninit_array, PtrEq};

/// \# of keys and values in a leaf node
pub const M: usize = 8;

/// A node in the b+tree. This can be either leaf node or internal node depending on the implicit
/// height.
pub struct Node<K, V> {
    /// Parent node. We use [NonNull] in part because [LeafNode] must be covariant in `K` and `V`.
    pub parent: Option<NodePtr<K, V>>,
    /// This node's index into the parent node's `edges` array.
    /// `*node.parent.d.internal().edges[node.parent_idx]` should be the same thing as `node`.
    /// This is only guaranteed to be initialized when `parent` is non-null.
    pub parent_idx: MaybeUninit<u16>,
    /// Total # Of keys and values, not including children.
    pub len: u16,
    /// Keys storage. The first `len` are initialized.
    pub keys: [MaybeUninit<K>; M],
    /// Values or children depending on the implicit height.
    pub d: NodeData<K, V>,
}

/// Contains leaf/internal-specific data. An untagged union, whether it contains leaf or internal
/// node data is determined by the implicit height.
pub union NodeData<K, V> {
    /// Leaf data. Only exists if the implicit height is 0.
    pub leaf: ManuallyDrop<LeafData<K, V>>,
    /// Internal data. Only exists if the implicit height is positive.
    pub internal: ManuallyDrop<InternalData<K, V>>,
}

/// Leaf data. Only exists if the implicit height is 0.
pub struct LeafData<K, V> {
    /// Vals storage. The first `len` are initialized.
    pub vals: [MaybeUninit<V>; M],
    /// Previous leaf node in the linked list.
    pub prev: Option<NodePtr<K, V>>,
    /// Next leaf node in the linked list.
    pub next: Option<NodePtr<K, V>>,
}

/// Internal data. Only exists if the implicit height is positive.
pub struct InternalData<K, V> {
    /// Pointers to the node's children. `edges[i]` is the child whose keys are between
    /// `keys[i - 1]` and `keys[i]` (if either doesn't exist, just before or after the other). The
    /// first `len + 1` are initialized.
    pub edges: [MaybeUninit<NodePtr<K, V>>; M + 1],
}

/// A managed, non-null pointer to a node. This is either a pointer to a leaf node or internal node,
/// depending on the implicit height.
pub type NodePtr<K, V> = UnsafeRef<Node<K, V>>;

impl<K, V> Node<K, V> {
    #[inline]
    pub fn leaf() -> Self {
        Node {
            parent: None,
            parent_idx: MaybeUninit::uninit(),
            len: 0,
            keys: maybe_uninit_array(),
            d: NodeData {
                leaf: ManuallyDrop::new(LeafData {
                    vals: maybe_uninit_array(),
                    prev: None,
                    next: None,
                })
            }
        }
    }

    #[inline]
    pub fn internal() -> Self {
        Node {
            parent: None,
            parent_idx: MaybeUninit::uninit(),
            len: 0,
            keys: maybe_uninit_array(),
            d: NodeData {
                internal: ManuallyDrop::new(InternalData {
                    edges: maybe_uninit_array()
                })
            }
        }
    }

    #[inline]
    pub fn parent(&self) -> Option<(NodePtr<K, V>, u16)> {
        self.parent.map(|p| (p, unsafe { self.parent_idx.assume_init() }))
    }

    #[inline]
    pub fn parent_idx(&self) -> Option<u16> {
        self.parent.map(|_| unsafe { self.parent_idx.assume_init() })
    }

    #[inline]
    pub fn set_parent(&mut self, parent: NodePtr<K, V>, parent_idx: u16) {
        self.parent = Some(parent);
        self.parent_idx.write(parent_idx);
    }

    #[inline]
    pub fn clear_parent(&mut self) {
        self.parent = None;
        self.parent_idx = MaybeUninit::uninit();
    }

    #[inline]
    pub unsafe fn prev(&self) -> Option<NodePtr<K, V>> {
        self.d.leaf().prev
    }

    #[inline]
    pub unsafe fn set_prev(&mut self, prev: Option<NodePtr<K, V>>) {
        self.d.leaf_mut().prev = prev;
    }

    #[inline]
    pub unsafe fn next(&self) -> Option<NodePtr<K, V>> {
        self.d.leaf().next
    }

    #[inline]
    pub unsafe fn set_next(&mut self, next: Option<NodePtr<K, V>>) {
        self.d.leaf_mut().next = next;
    }

    #[inline]
    pub unsafe fn key(&self, idx: u16) -> &K {
        debug_assert!(idx < self.len);
        self.keys.get_unchecked(idx as usize).assume_init_ref()
    }

    #[inline]
    pub unsafe fn key_mut(&mut self, idx: u16) -> &mut K {
        debug_assert!(idx < self.len);
        self.keys.get_unchecked_mut(idx as usize).assume_init_mut()
    }

    #[inline]
    pub unsafe fn val(&self, idx: u16) -> &V {
        debug_assert!(idx < self.len);
        self.d.leaf().vals.get_unchecked(idx as usize).assume_init_ref()
    }

    #[inline]
    pub unsafe fn val_mut(&mut self, idx: u16) -> &mut V {
        debug_assert!(idx < self.len);
        self.d.leaf_mut().vals.get_unchecked_mut(idx as usize).assume_init_mut()
    }

    /// Copies the value, you must call `write_val` or `remove` and then `forget`.
    #[inline]
    pub unsafe fn read_val(&mut self, idx: u16) -> V {
        debug_assert!(idx < self.len);
        self.d.leaf().vals.get_unchecked(idx as usize).assume_init_read()
    }

    #[inline]
    pub unsafe fn write_val(&mut self, idx: u16, val: V) {
        debug_assert!(idx < self.len);
        self.d.leaf_mut().vals.get_unchecked_mut(idx as usize).write(val);
    }

    #[inline]
    pub unsafe fn key_val(&self, idx: u16) -> (&K, &V) {
        debug_assert!(idx < self.len);
        (
            self.keys.get_unchecked(idx as usize).assume_init_ref(),
            self.d.leaf().vals.get_unchecked(idx as usize).assume_init_ref(),
        )
    }

    #[inline]
    pub unsafe fn key_val_mut(&mut self, idx: u16) -> (&K, &mut V) {
        debug_assert!(idx < self.len);
        (
            self.keys.get_unchecked(idx as usize).assume_init_ref(),
            self.d.leaf_mut().vals.get_unchecked_mut(idx as usize).assume_init_mut(),
        )
    }

    #[inline]
    pub unsafe fn read_key_val(&self, idx: u16) -> (K, V) {
        debug_assert!(idx < self.len);
        (
            self.keys.get_unchecked(idx as usize).assume_init_read(),
            self.d.leaf().vals.get_unchecked(idx as usize).assume_init_read(),
        )
    }

    #[inline]
    pub unsafe fn edge(&self, idx: u16) -> NodePtr<K, V> {
        debug_assert!(idx < self.len + 1);
        self.d.internal().edges.get_unchecked(idx as usize).assume_init()
    }

    #[inline]
    pub unsafe fn edge_mut(&mut self, idx: u16) -> &mut NodePtr<K, V> {
        debug_assert!(idx < self.len + 1);
        self.d.internal_mut().edges.get_unchecked_mut(idx as usize).assume_init_mut()
    }

    #[inline]
    pub unsafe fn keys(&self) -> &[K] {
        &*(&self.keys[..self.len as usize] as *const [MaybeUninit<K>] as *const [K])
    }

    #[inline]
    pub unsafe fn keys_mut(&mut self) -> &mut [K] {
        &mut *(&mut self.keys[..self.len as usize] as *mut [MaybeUninit<K>] as *mut [K])
    }

    #[allow(unused)]
    #[inline]
    pub unsafe fn vals(&self) -> &[V] {
        &*(&self.d.leaf().vals[..self.len as usize] as *const [MaybeUninit<V>] as *const [V])
    }

    #[inline]
    pub unsafe fn vals_mut(&mut self) -> &mut [V] {
        &mut *(&mut self.d.leaf_mut().vals[..self.len as usize] as *mut [MaybeUninit<V>] as *mut [V])
    }

    #[inline]
    pub unsafe fn edges(&self) -> &[NodePtr<K, V>] {
        &*(&self.d.internal().edges[..(self.len + 1) as usize] as *const [MaybeUninit<NodePtr<K, V>>]
            as *const [NodePtr<K, V>])
    }

    #[allow(unused)]
    #[inline]
    pub unsafe fn edges_mut(&mut self) -> &mut [NodePtr<K, V>] {
        &mut *(&mut self.d.internal_mut().edges[..(self.len + 1) as usize] as *mut [MaybeUninit<NodePtr<K, V>>]
            as *mut [NodePtr<K, V>])
    }

    #[inline]
    pub unsafe fn first_key_value(&self) -> (&K, &V) {
        debug_assert!(self.len > 0);
        let key = self.keys.get_unchecked(0).assume_init_ref();
        let val = self.d.leaf().vals.get_unchecked(0).assume_init_ref();
        (key, val)
    }

    #[inline]
    pub unsafe fn first_key_value_mut(&mut self) -> (&K, &mut V) {
        debug_assert!(self.len > 0);
        let key = self.keys.get_unchecked(0).assume_init_ref();
        let val = self.d.leaf_mut().vals.get_unchecked_mut(0).assume_init_mut();
        (key, val)
    }

    #[inline]
    pub unsafe fn last_key_value(&self) -> (&K, &V) {
        debug_assert!(self.len > 0);
        let key = self.keys.get_unchecked(self.len as usize - 1).assume_init_ref();
        let val = self.d.leaf().vals.get_unchecked(self.len as usize - 1).assume_init_ref();
        (key, val)
    }

    #[inline]
    pub unsafe fn last_key_value_mut(&mut self) -> (&K, &mut V) {
        debug_assert!(self.len > 0);
        let key = self.keys.get_unchecked(self.len as usize - 1).assume_init_ref();
        let val = self.d.leaf_mut().vals.get_unchecked_mut(self.len as usize - 1).assume_init_mut();
        (key, val)
    }

    /// Doesn't rebalance
    #[inline]
    pub unsafe fn insert_val(&mut self, idx: u16, key: K, val: V) {
        debug_assert!(idx <= self.len);
        debug_assert!((self.len as usize) < M, "LeafNode::insert would overflow");

        // Shift later keys and values
        if self.len > idx {
            unsafe_copy_slice_overlapping(
                &mut self.keys,
                idx as usize + 1..self.len as usize + 1,
                idx as usize..self.len as usize
            );
            unsafe_copy_slice_overlapping(
                &mut self.d.leaf_mut().vals,
                idx as usize + 1..self.len as usize + 1,
                idx as usize..self.len as usize
            );
        }

        // Do insert
        self.keys[idx as usize].write(key);
        self.d.leaf_mut().vals[idx as usize].write(val);

        self.len += 1;
    }

    /// Doesn't rebalance. You must call `set_parent` on the edge beforehand.
    #[inline]
    pub unsafe fn insert_edge(&mut self, idx: u16, after_key: bool, key: K, edge: NodePtr<K, V>) {
        debug_assert!(idx <= self.len);
        debug_assert!((self.len as usize) < M, "InternalNode::insert_edge would overflow");
        debug_assert_eq!(
            edge.as_ref().parent_idx(),
            Some(match after_key {
                false => idx,
                true => idx + 1,
            }),
            "InternalNode::insert_edge edge's parent_idx must be set before insertion (idx is redundant)"
        );

        // Shift later keys and edges
        if idx < self.len {
            unsafe_copy_slice_overlapping(
                &mut self.keys,
                idx as usize + 1..self.len as usize + 1,
                idx as usize..self.len as usize
            );
        }
        let after_edge_idx = match after_key {
            false => idx,
            true => idx + 1
        };
        if after_edge_idx < self.len + 1 {
            unsafe_copy_slice_overlapping(
                &mut self.d.internal_mut().edges,
                after_edge_idx as usize + 1..self.len as usize + 2,
                after_edge_idx as usize..self.len as usize + 1
            );
            // Update later edge parent idxs
            for edge in self.d.internal_mut().edges[after_edge_idx as usize + 1..self.len as usize + 2].iter_mut().map(|e| e.assume_init_mut()) {
                *edge.as_mut().parent_idx.assume_init_mut() += 1;
            }
        }

        // Do insert
        self.keys[idx as usize].write(key);
        if after_key {
            self.d.internal_mut().edges[idx as usize + 1].write(edge);
        } else {
            self.d.internal_mut().edges[idx as usize].write(edge);
        }

        self.len += 1;
    }

    /// You must call `set_parent` on the edge beforehand.
    #[inline]
    pub unsafe fn set_last_edge(&mut self, edge: NodePtr<K, V>) {
        debug_assert_eq!(
            edge.as_ref().parent_idx(),
            Some(self.len),
            "InternalNode::set_last_edge edge's parent_idx must be set before insertion (idx is redundant)"
        );
        self.d.internal_mut().edges[self.len as usize].write(edge);
    }

    /// Doesn't rebalance
    #[inline]
    pub unsafe fn remove_val(&mut self, idx: u16) -> (K, V) {
        debug_assert!(idx < self.len);
        debug_assert!(self.len > 0);

        // Read removed key and value (safe because we either overwrite or decrease len past memory)
        let key = self.keys[idx as usize].assume_init_read();
        let val = self.d.leaf().vals[idx as usize].assume_init_read();

        // Shift later keys and values
        if idx + 1 < self.len {
            unsafe_copy_slice_overlapping(
                &mut self.keys,
                idx as usize..self.len as usize - 1,
                idx as usize + 1..self.len as usize
            );
            unsafe_copy_slice_overlapping(
                &mut self.d.leaf_mut().vals,
                idx as usize..self.len as usize - 1,
                idx as usize + 1..self.len as usize
            );
        }

        self.len -= 1;
        (key, val)
    }

    /// Doesn't rebalance.
    #[inline]
    pub unsafe fn remove_edge(&mut self, idx: u16, after_key: bool) -> (K, NodePtr<K, V>) {
        debug_assert!(idx < self.len);
        debug_assert!(self.len > 0);
        let edge_idx = match after_key {
            false => idx,
            true => idx + 1
        };
        debug_assert_eq!(
            self.edge(edge_idx).as_ref().parent_idx(),
            Some(edge_idx),
            "Sanity check failed: InternalNode::remove_edge edge's parent_idx is wrong"
        );

        // Read removed key and edge (safe because we either overwrite or decrease len past memory)
        let key = self.keys[idx as usize].assume_init_read();
        let edge = self.d.internal().edges[edge_idx as usize].assume_init();

        // Shift later keys and edges
        if idx + 1 < self.len {
            unsafe_copy_slice_overlapping(
                &mut self.keys,
                idx as usize..self.len as usize - 1,
                idx as usize + 1..self.len as usize
            );
        }
        if edge_idx < self.len {
            unsafe_copy_slice_overlapping(
                &mut self.d.internal_mut().edges,
                edge_idx as usize..self.len as usize,
                edge_idx as usize + 1..self.len as usize + 1
            );
            // Update later edge parent idxs
            for edge in self.d.internal_mut().edges[edge_idx as usize..self.len as usize].iter_mut().map(|e| e.assume_init_mut()) {
                *edge.as_mut().parent_idx.assume_init_mut() -= 1;
            }
        }

        self.len -= 1;
        (key, edge)
    }

    /// Doesn't rebalance, removes edge after key
    #[inline]
    pub unsafe fn remove_last_edge(&mut self) -> (K, NodePtr<K, V>) {
        debug_assert!(self.len > 0);
        debug_assert_eq!(
            self.edge(self.len).as_ref().parent_idx(),
            Some(self.len),
            "Sanity check failed: InternalNode::remove_last_edge edge's parent_idx is wrong"
        );

        // Read removed key and edge (safe because we decrease len past memory)
        let key = self.keys[self.len as usize - 1].assume_init_read();
        let edge = self.d.internal().edges[self.len as usize].assume_init();

        self.len -= 1;
        (key, edge)
    }

    /// Replaces key but not value or edge at the given index
    #[inline]
    pub unsafe fn replace_key(&mut self, idx: u16, key: K) -> K {
        debug_assert!(idx < self.len);
        let old_key = self.keys[idx as usize].assume_init_read();
        self.keys[idx as usize].write(key);
        old_key
    }

    /// Replace value but not key at the given index
    #[inline]
    pub unsafe fn replace_val(&mut self, idx: u16, val: V) -> V {
        debug_assert!(idx < self.len);
        let old_val = self.d.leaf().vals[idx as usize].assume_init_read();
        self.d.leaf_mut().vals[idx as usize].write(val);
        old_val
    }

    /// This becomes the left node, returns the right node and replaces the key with the median key
    /// ("split key").
    ///
    /// `self.d.leaf().prev`, `right.d.leaf().next`, and `self.d.leaf().prev.next` are set, but you need to set
    /// `self.d.leaf().next`, `right.d.leaf().prev`, and `right.d.leaf().next.prev`.
    #[inline]
    pub unsafe fn split_leaf(&mut self, mut idx: u16, key: &mut K, mut val: V) -> Node<K, V> where K: Clone {
        debug_assert!(idx <= self.len);
        debug_assert!(self.len as usize >= M / 2, "LeafNode::split_leaf would underflow");

        let median = self.len / 2;
        let mut right = Node::leaf();

        // Insert so that idx is median, and key and val point to the median val
        while idx < median {
            swap(self.key_mut(idx), key);
            swap(self.val_mut(idx), &mut val);
            idx += 1;
        }
        while idx > median {
            idx -= 1;
            swap(self.key_mut(idx), key);
            swap(self.val_mut(idx), &mut val);
        }

        // Now we just split and insert the middle into one of the nodes
        unsafe_copy_slice_nonoverlapping(&mut right.keys[1..median as usize + 1], &self.keys[median as usize..self.len as usize]);
        unsafe_copy_slice_nonoverlapping(&mut right.d.leaf_mut().vals[1..median as usize + 1], &self.d.leaf().vals[median as usize..self.len as usize]);
        // Remember: this is a B+ tree, so we copy the key in the leaf node, and write the val
        // instead of propagating it to the internal.
        right.keys[0].write(key.clone());
        right.d.leaf_mut().vals[0].write(val);
        right.len = self.len - median + 1;
        self.len = median;
        right.d.leaf_mut().next = self.d.leaf().next;
        right
    }

    /// This becomes the left node, returns the right node and replaces the key with the median key
    /// ("split key"). The edge is inserted after the key.
    ///
    /// `idx` is actually redundant here, you must call `set_parent` on `edge` before. You must also
    /// set the parent node on all nodes in `right` (the returned node).
    #[inline]
    pub unsafe fn split_internal(&mut self, mut idx: u16, key: &mut K, mut edge: NodePtr<K, V>) -> Node<K, V> {
        debug_assert!(idx <= self.len);
        debug_assert!(self.len as usize >= M / 2, "InternalNode::split_internal would underflow");
        debug_assert_eq!(edge.as_ref().parent_idx(), Some(idx + 1), "InternalNode::split_internal idx is redundant and should be edge.parent_idx - 1");

        let median = self.len / 2;
        let mut right = Node::internal();

        // Insert so that idx is median, and key and val point to the median val
        while idx < median {
            swap(self.key_mut(idx), key);
            swap(self.edge_mut(idx + 1), &mut edge);
            idx += 1;

            // old edge's parent_idx is already idx + 1
            // new edge's parent_idx is idx before increment, we need to update it
            debug_assert_eq!(edge.as_ref().parent_idx(), Some(idx));
            edge.as_mut().parent_idx = MaybeUninit::new(idx + 1);
        }
        while idx > median {
            idx -= 1;
            swap(self.key_mut(idx), key);
            swap(self.edge_mut(idx + 1), &mut edge);

            // old edge's parent idx will be changed after split
            // new edge's parent_idx is already idx after decrement
            debug_assert_eq!(edge.as_ref().parent_idx(), Some(idx + 1));
        }

        // Now we just split and insert the middle into one of the nodes
        unsafe_copy_slice_nonoverlapping(&mut right.keys[..median as usize], &self.keys[median as usize..self.len as usize]);
        unsafe_copy_slice_nonoverlapping(&mut right.d.internal_mut().edges[1..median as usize + 1], &self.d.internal().edges[median as usize + 1..self.len as usize + 1]);
        // Put the edge in index 0 in right, so that it's after the split key
        right.d.internal_mut().edges[0].write(edge);
        // Update parent_idxs in right (including the edge we just inserted)
        for (idx, mut edge) in right.d.internal_mut().edges[..median as usize + 1].iter_mut().enumerate().map(|(idx, e)| (idx as u16, e.assume_init())) {
            *edge.as_mut().parent_idx.assume_init_mut() = idx;
        }
        right.len = self.len - median;
        self.len = median;
        right
    }

    /// Absorbs all of `prev`'s keys and values and also its `prev`. Afterwards `prev` should be
    /// removed from the parent and discarded, and `self.prev.next` should be set to `self`.
    #[inline]
    pub unsafe fn merge_prev_leaf(&mut self, prev: &mut Node<K, V>) {
        debug_assert!(self.prev().ptr_eq(&Some(NodePtr::from_ref(prev))));
        debug_assert!(
            prev.parent.ptr_eq(&self.parent),
            "sanity check failed: prev.parent != self.parent (the failure happened before this function call, it was only detected now)"
        );
        debug_assert_eq!(
            prev.parent_idx().expect("sanity check failed") + 1, self.parent_idx().expect("sanity check failed"),
            "sanity check failed: prev.parent_idx + 1 != self.parent_idx (the failure happened before this function call, it was only detected now)"
        );
        debug_assert!((prev.len + self.len) as usize <= M, "nodes are too big to merge");

        let new_len = prev.len + self.len;
        unsafe_copy_slice_overlapping(&mut self.keys, prev.len as usize..new_len as usize, ..self.len as usize);
        unsafe_copy_slice_overlapping(&mut self.d.leaf_mut().vals, prev.len as usize..new_len as usize, ..self.len as usize);
        unsafe_copy_slice_nonoverlapping(&mut self.keys[..prev.len as usize], &prev.keys[..prev.len as usize]);
        unsafe_copy_slice_nonoverlapping(&mut self.d.leaf_mut().vals[..prev.len as usize], &prev.d.leaf().vals[..prev.len as usize]);
        self.len = new_len;
        self.set_prev(prev.prev());
    }

    /// Absorbs all of `next`'s keys and values and also its `next`. Afterwards `next` should be
    /// discarded and removed from the parent, and `self.next.prev` should be set to `self`.
    #[inline]
    pub unsafe fn merge_next_leaf(&mut self, next: &mut Node<K, V>) {
        debug_assert!(self.next().ptr_eq(&Some(NodePtr::from_ref(next))));
        debug_assert!(
            self.parent.ptr_eq(&next.parent),
            "sanity check failed: self.parent != next.parent (the failure happened before this function call, it was only detected now)"
        );
        debug_assert_eq!(
            self.parent_idx().expect("sanity check failed") + 1, next.parent_idx().expect("sanity check failed"),
            "sanity check failed: self.parent_idx + 1 != next.parent_idx (the failure happened before this function call, it was only detected now)"
        );
        debug_assert!((self.len + next.len) as usize <= M, "nodes are too big to merge");

        let new_len = self.len + next.len;
        unsafe_copy_slice_nonoverlapping(&mut self.keys[self.len as usize..new_len as usize], &next.keys[..next.len as usize]);
        unsafe_copy_slice_nonoverlapping(&mut self.d.leaf_mut().vals[self.len as usize..new_len as usize], &next.d.leaf().vals[..next.len as usize]);
        self.len = new_len;
        self.set_next(next.next());
    }

    /// Absorbs all of `prev`'s key and edges. Beforehand `prev`'s edges' parent nodes should be
    /// updated to `self`, and afterwards `prev` should be removed from the parent and discarded.
    #[inline]
    pub unsafe fn merge_prev_internal(&mut self, middle_key: K, prev: &mut Node<K, V>) {
        debug_assert!(
            prev.parent.ptr_eq(&self.parent),
            "sanity check failed: prev.parent != self.parent (the failure happened before this function call, it was only detected now)"
        );
        debug_assert_eq!(
            prev.parent_idx().expect("sanity check failed") + 1, self.parent_idx().expect("sanity check failed"),
            "sanity check failed: prev.parent_idx + 1 != self.parent_idx (the failure happened before this function call, it was only detected now)"
        );
        debug_assert!(((prev.len + self.len) as usize) < M, "nodes are too big to merge");

        let new_len = prev.len + self.len + 1;
        unsafe_copy_slice_overlapping(&mut self.keys, prev.len as usize + 1..new_len as usize, ..self.len as usize);
        unsafe_copy_slice_overlapping(&mut self.d.internal_mut().edges, prev.len as usize + 1..new_len as usize + 1, ..self.len as usize + 1);
        // Update edge parent indices
        for edge in self.d.internal_mut().edges[prev.len as usize + 1..new_len as usize + 1].iter_mut().map(|e| e.assume_init_mut()) {
            *edge.as_mut().parent_idx.assume_init_mut() += prev.len + 1;
        }
        unsafe_copy_slice_nonoverlapping(&mut self.keys[..prev.len as usize], &prev.keys[..prev.len as usize]);
        unsafe_copy_slice_nonoverlapping(&mut self.d.internal_mut().edges[..prev.len as usize + 1], &prev.d.internal().edges[..prev.len as usize + 1]);
        self.keys[prev.len as usize].write(middle_key);
        self.len = new_len;
    }

    /// Absorbs all of `next`'s key and edges. Beforehand `next`'s edges' parent nodes should be
    /// updated to `self`, and afterwards `next` should be removed from the parent and discarded.
    #[inline]
    pub unsafe fn merge_next_internal(&mut self, middle_key: K, next: &mut Node<K, V>) {
        debug_assert!(
            self.parent.ptr_eq(&next.parent),
            "sanity check failed: self.parent != next.parent (the failure happened before this function call, it was only detected now)"
        );
        debug_assert_eq!(
            self.parent_idx().expect("sanity check failed") + 1, next.parent_idx().expect("sanity check failed"),
            "sanity check failed: self.parent_idx + 1 != next.parent_idx (the failure happened before this function call, it was only detected now)"
        );
        debug_assert!(((self.len + next.len) as usize) < M, "nodes are too big to merge");
        let new_len = self.len + next.len + 1;
        self.keys[self.len as usize].write(middle_key);
        unsafe_copy_slice_nonoverlapping(&mut self.keys[self.len as usize + 1..new_len as usize], &next.keys[..next.len as usize]);
        unsafe_copy_slice_nonoverlapping(&mut self.d.internal_mut().edges[self.len as usize + 1..new_len as usize + 1], &next.d.internal().edges[..next.len as usize + 1]);
        // Update edge parent indices
        for edge in self.d.internal_mut().edges[self.len as usize + 1..new_len as usize + 1].iter_mut().map(|e| e.assume_init_mut()) {
            *edge.as_mut().parent_idx.assume_init_mut() += self.len + 1;
        }
        self.len = new_len;
    }
}

impl<K, V> NodeData<K, V> {
    pub unsafe fn leaf(&self) -> &LeafData<K, V> {
        &*self.leaf
    }

    pub unsafe fn leaf_mut(&mut self) -> &mut LeafData<K, V> {
        &mut *self.leaf
    }

    pub unsafe fn internal(&self) -> &InternalData<K, V> {
        &*self.internal
    }

    pub unsafe fn internal_mut(&mut self) -> &mut InternalData<K, V> {
        &mut *self.internal
    }
}

#[inline]
pub unsafe fn normalize_address<K, V>(node: NodePtr<K, V>, idx: u16) -> Option<(NodePtr<K, V>, u16)> {
    let node_ref = node.as_ref();
    if idx < node_ref.len {
        Some((node, idx))
    } else {
        debug_assert_eq!(idx, node_ref.len);
        node_ref.next().map(|node| (node, 0))
    }
}

#[inline]
pub unsafe fn address_before<K, V>(node: NodePtr<K, V>, idx: u16) -> Option<(NodePtr<K, V>, u16)> {
    let node_ref = node.as_ref();
    if idx > 0 {
        Some((node, idx - 1))
    } else {
        node_ref.prev().map(|node| (node, node.as_ref().len - 1))
    }
}

#[inline]
pub unsafe fn address_after<K, V>(node: NodePtr<K, V>, idx: u16) -> Option<(NodePtr<K, V>, u16)> {
    let node_ref = node.as_ref();
    if idx < node_ref.len - 1 {
        Some((node, idx + 1))
    } else if idx == node_ref.len - 1 {
        node_ref.next().map(|node| (node, 0))
    } else {
        // Not normalize AND we want the address after anyways. Currently this branch is never
        // actually reached, but if it was this is what we would do
        debug_assert_eq!(idx, node_ref.len);
        node_ref.next().map(|node| (node, 1))
    }
}

#[inline]
unsafe fn unsafe_copy_slice_overlapping<T>(
    data: &mut [T],
    dst: impl RangeBounds<usize>,
    src: impl RangeBounds<usize>
) {
    let src_start = match src.start_bound() {
        Bound::Included(&n) => n,
        Bound::Excluded(&n) => n + 1,
        Bound::Unbounded => 0,
    };
    let src_end = match src.end_bound() {
        Bound::Included(&n) => n + 1,
        Bound::Excluded(&n) => n,
        Bound::Unbounded => data.len(),
    };
    let dst_start = match dst.start_bound() {
        Bound::Included(&n) => n,
        Bound::Excluded(&n) => n + 1,
        Bound::Unbounded => 0,
    };
    let dst_end = match dst.end_bound() {
        Bound::Included(&n) => n + 1,
        Bound::Excluded(&n) => n,
        Bound::Unbounded => data.len(),
    };
    let src_len = src_end - src_start;
    let dst_len = dst_end - dst_start;
    debug_assert_eq!(src_len, dst_len);
    let ptr = data.as_mut_ptr();
    let src = ptr.add(src_start);
    let dst = ptr.add(dst_start);
    copy(src, dst, src_len);
}

#[inline]
unsafe fn unsafe_copy_slice_nonoverlapping<T>(dst: &mut [T], src: &[T]) {
    debug_assert_eq!(dst.len(), src.len());
    copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), src.len());
}