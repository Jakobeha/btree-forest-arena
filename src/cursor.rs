use crate::node::{Node, NodePtr};
use std::marker::PhantomData;

/// Iterates a node's keys and values forwards or backwards.
pub struct Cursor<'a, K, V> {
    /// Current node
    node: Option<NodePtr<K, V>>,
    /// Current index in the node, not counting child nodes.
    index: u16,
    /// Phantom data
    _p: PhantomData<(&'a K, &'a V)>,
}

impl<'a, K, V> Cursor<'a, K, V> {
    #[inline]
    pub fn new_detached() -> Self {
        Self {
            node: None,
            index: 0,
            _p: PhantomData,
        }
    }

    /// # Safety
    /// Node and connected pointers must be alive for `'a`, and the node must be a leaf.
    #[inline]
    pub unsafe fn new(node: Option<NodePtr<K, V>>, index: u16) -> Self {
        let cursor = Self {
            node,
            index,
            _p: PhantomData,
        };
        cursor.validate();
        cursor
    }

    /// # Safety
    /// Node and connected pointers must be alive for `'a`, and the node must be a leaf.
    #[inline]
    pub unsafe fn new_at_end(node: Option<NodePtr<K, V>>) -> Self {
        let idx = match node {
            None => 0,
            Some(node) => node.as_ref().len - 1,
        };
        Self::new(node, idx)
    }

    /// Move to the next entry
    #[inline]
    pub fn advance(&mut self) {
        let Some(node) = self.node() else {
            panic!("Cursor::advance called on empty cursor");
        };
        if self.index < node.len - 1 {
            self.index += 1;
        } else {
            self.node = unsafe { node.next() };
            self.index = 0;
        }
    }

    /// Move to the previous entry
    #[inline]
    pub fn advance_back(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        } else {
            self.node = match self.node() {
                None => panic!("Cursor::advance_back called on empty cursor"),
                Some(node) => unsafe { node.prev() },
            };
            self.index = match self.node() {
                None => 0,
                Some(node) => node.len - 1,
            };
        }
    }

    /// Whether the cursor has an entry
    #[inline]
    pub fn is_attached(&self) -> bool {
        self.node.is_some()
    }

    /// Make the cursor no longer have a current entry.
    #[inline]
    pub fn detach(&mut self) {
        self.node = None;
    }

    #[inline]
    pub fn address(&self) -> Option<(NodePtr<K, V>, u16)> {
        let node = self.node?;
        Some((node, self.index))
    }

    #[inline]
    pub fn key_value(&self) -> Option<(&'a K, &'a V)> {
        let node = self.node()?;
        Some(unsafe { node.key_val(self.index) })
    }

    /// # Safety
    /// Must have exclusive access to the current node
    #[inline]
    pub unsafe fn key_value_mut(&mut self) -> Option<(&'a K, &'a mut V)> {
        let node = self.node_mut()?;
        Some(unsafe { node.key_val_mut(self.index) })
    }

    /// # Safety
    /// This effectively copies the key and value, so you must ensure it isn't dropped or read again
    #[inline]
    pub unsafe fn read_key_value(&self) -> Option<(K, V)> {
        let node = self.node()?;
        Some(node.read_key_val(self.index))
    }

    #[inline]
    fn node(&self) -> Option<&'a Node<K, V>> {
        self.node.as_ref().map(|node| unsafe { node.as_ref() })
    }

    /// # Safety
    /// Must have exclusive access to the current node
    #[inline]
    unsafe fn node_mut(&mut self) -> Option<&'a mut Node<K, V>> {
        self.node.as_mut().map(|node| node.as_mut())
    }

    /// Check that the cursor index is within the node
    #[inline]
    pub fn validate(&self) {
        assert!(
            self.node().map_or(true, |node| self.index < node.len),
            "Cursor index out of bounds"
        );
    }
}
