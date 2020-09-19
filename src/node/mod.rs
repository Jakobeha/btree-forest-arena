use crate::{
	Item
};

mod leaf;
mod internal;

use leaf::Leaf as LeafNode;
use internal::Internal as InternalNode;

#[derive(Debug)]
pub enum Balance {
	Balanced,
	Underflow(bool) // true if the node is empty.
}

pub struct WouldUnderflow;

/// B-tree node.
pub enum Node<K, V, const M: usize> {
	/// Free node pointing to the previous and next free node if any.
	Free(Option<usize>, Option<usize>),

	/// Internal node.
	Internal(InternalNode<K, V, M>),

	/// Leaf node.
	Leaf(LeafNode<K, V, M>)
}

impl<K, V, const M: usize> Node<K, V, M> {
	#[inline]
	pub fn binary(left_id: usize, median: Item<K, V>, right_id: usize) -> Node<K, V, M> {
		Node::Internal(InternalNode::binary(left_id, median, right_id))
	}

	#[inline]
	pub fn leaf(item: Item<K, V>) -> Node<K, V, M> {
		Node::Leaf(LeafNode::new(item))
	}

	#[inline]
	pub fn child_count(&self) -> usize {
		match self {
			Node::Internal(node) => node.child_count(),
			Node::Leaf(_) => 0,
			_ => panic!("free node have no children")
		}
	}

	#[inline]
	pub fn child_id(&self, index: usize) -> usize {
		match self {
			Node::Internal(node) => node.child_id(index),
			_ => panic!("only internal nodes can be indexed")
		}
	}

	#[inline]
	pub fn child_id_opt(&self, index: usize) -> Option<usize> {
		match self {
			Node::Internal(node) => node.child_id_opt(index),
			Node::Leaf(_) => None,
			_ => panic!("free node")
		}
	}

	#[inline]
	pub fn as_free_node(&self) -> Result<(Option<usize>, Option<usize>), ()> {
		match self {
			Node::Free(prev_id, next_id) => Ok((*prev_id, *next_id)),
			_ => Err(())
		}
	}

	#[inline]
	pub fn get(&self, key: &K) -> Result<Option<&V>, usize> where K: Ord {
		match self {
			Node::Leaf(leaf) => Ok(leaf.get(key)),
			Node::Internal(node) => match node.get(key) {
				Ok(value) => Ok(Some(value)),
				Err(e) => Err(e)
			},
			_ => panic!("free node")
		}
	}

	#[inline]
	pub fn get_mut(&mut self, key: &K) -> Result<Option<&mut V>, usize> where K: Ord {
		match self {
			Node::Leaf(leaf) => Ok(leaf.get_mut(key)),
			Node::Internal(node) => match node.get_mut(key) {
				Ok(value) => Ok(Some(value)),
				Err(e) => Err(e)
			},
			_ => panic!("free node")
		}
	}

	/// Find the offset of the item matching the given key.
	///
	/// If the key matches no item in this node,
	/// this funtion returns the index and id of the child that may match the key,
	/// or `Err(None)` if it is a leaf.
	#[inline]
	pub fn offset_of(&self, key: &K) -> Result<usize, Option<(usize, usize)>> where K: Ord {
		match self {
			Node::Internal(node) => match node.offset_of(key) {
				Ok(i) => Ok(i),
				Err(e) => Err(Some(e))
			},
			Node::Leaf(leaf) => match leaf.offset_of(key) {
				Some(i) => Ok(i),
				None =>  Err(None)
			},
			_ => panic!("free nodes have no items")
		}
	}

	#[inline]
	pub fn item_at_mut(&mut self, offset: usize) -> &mut Item<K, V> {
		match self {
			Node::Internal(node) => node.item_at_mut(offset),
			Node::Leaf(leaf) => leaf.item_at_mut(offset),
			_ => panic!("free node have no items")
		}
	}

	/// Insert on a node.
	///
	/// It is assumed that the node is not free.
	/// If it is a leaf node, there must be a free space in it for the inserted value.
	#[inline]
	pub fn insert(&mut self, key: K, value: V) -> Result<Option<V>, (K, V, usize, usize)> where K: Ord {
		match self {
			Node::Internal(node) => match node.insert(key, value) {
				Ok(value) => Ok(Some(value)),
				Err(e) => Err(e)
			},
			Node::Leaf(leaf) => Ok(leaf.insert(key, value)),
			_ => panic!("cannot insert on free node")
		}
	}

	/// Split the node if it would overlow upon insertion.
	#[inline]
	pub fn split(&mut self) -> Result<(Item<K, V>, Node<K, V, M>), ()> {
		match self {
			Node::Internal(node) => match node.split() {
				Ok((v, node)) => Ok((v, Node::Internal(node))),
				Err(()) => Err(())
			},
			Node::Leaf(leaf) => match leaf.split() {
				Ok((v, leaf)) => Ok((v, Node::Leaf(leaf))),
				Err(()) => Err(())
			},
			_ => panic!("cannot split on free node")
		}
	}

	#[inline]
	pub fn merge(&mut self, left_index: usize, right_index: usize) -> (usize, usize, Item<K, V>, Balance) {
		match self {
			Node::Internal(node) => node.merge(left_index, right_index),
			_ => panic!("only internal nodes can merge children")
		}
	}

	#[inline]
	pub fn append(&mut self, separator: Item<K, V>, other: Node<K, V, M>) {
		match (self, other) {
			(Node::Internal(node), Node::Internal(other)) => node.append(separator, other),
			(Node::Leaf(leaf), Node::Leaf(other)) => leaf.append(separator, other),
			_ => panic!("incompatibles nodes")
		}
	}

	#[inline]
	pub fn push_left(&mut self, item: Item<K, V>, opt_child_id: Option<usize>) {
		match self {
			Node::Internal(node) => node.push_left(item, opt_child_id.unwrap()),
			Node::Leaf(leaf) => leaf.push_left(item),
			_ => panic!("free node")
		}
	}

	#[inline]
	pub fn pop_left(&mut self) -> Result<(Item<K, V>, Option<usize>), WouldUnderflow> {
		match self {
			Node::Internal(node) => {
				let (item, child_id) = node.pop_left()?;
				Ok((item, Some(child_id)))
			},
			Node::Leaf(leaf) => Ok((leaf.pop_left()?, None)),
			_ => panic!("free node")
		}
	}

	#[inline]
	pub fn push_right(&mut self, item: Item<K, V>, opt_child_id: Option<usize>) {
		match self {
			Node::Internal(node) => node.push_right(item, opt_child_id.unwrap()),
			Node::Leaf(leaf) => leaf.push_right(item),
			_ => panic!("free node")
		}
	}

	#[inline]
	pub fn pop_right(&mut self) -> Result<(Item<K, V>, Option<usize>), WouldUnderflow> {
		match self {
			Node::Internal(node) => {
				let (item, child_id) = node.pop_right()?;
				Ok((item, Some(child_id)))
			},
			Node::Leaf(leaf) => Ok((leaf.pop_right()?, None)),
			_ => panic!("free node")
		}
	}

	#[inline]
	pub fn take(&mut self, offset: usize) -> Result<(Item<K, V>, Balance), usize> {
		match self {
			Node::Internal(node) => {
				let left_child_index = offset;
				Err(node.child_id(left_child_index))
			},
			Node::Leaf(leaf) => Ok(leaf.take(offset)),
			_ => panic!("free node")
		}
	}

	#[inline]
	pub fn force_take(&mut self, offset: usize) -> Result<(Item<K, V>, Balance), (usize, Item<K, V>, usize)> {
		match self {
			Node::Internal(node) => Err(node.take(offset)),
			Node::Leaf(leaf) => Ok(leaf.take(offset)),
			_ => panic!("free node")
		}
	}

	#[inline]
	pub fn take_rightmost_leaf(&mut self) -> Result<(Item<K, V>, Balance), (usize, usize)> {
		match self {
			Node::Internal(node) => {
				let child_index = node.child_count() - 1;
				let child_id = node.child_id(child_index);
				Err((child_index, child_id))
			},
			Node::Leaf(leaf) => Ok(leaf.take_last()),
			_ => panic!("free node")
		}
	}

	/// Put an item in a node.
	///
	/// It is assumed that the node will not overflow.
	#[inline]
	pub fn put(&mut self, offset: usize, item: Item<K, V>, opt_right_child_id: Option<usize>) {
		match self {
			Node::Internal(node) => node.put(offset, item, opt_right_child_id.unwrap()),
			Node::Leaf(leaf) => leaf.put(offset, item),
			_ => panic!("free node")
		}
	}

	#[inline]
	pub fn replace(&mut self, offset: usize, item: Item<K, V>) -> Item<K, V> {
		match self {
			Node::Internal(node) => node.replace(offset, item),
			_ => panic!("can only replace in internal nodes")
		}
	}

	#[inline]
	pub fn separators(&self, i: usize) -> (Option<&K>, Option<&K>) {
		match self {
			Node::Leaf(_) => (None, None),
			Node::Internal(node) => node.separators(i),
			_ => panic!("free node")
		}
	}

	#[inline]
	pub fn children(&self) -> Children<K, V> {
		match self {
			Node::Leaf(_) => Children::Leaf,
			Node::Internal(node) => node.children(),
			_ => panic!("free node")
		}
	}

	/// Write the label of the node in the DOT format.
	///
	/// Requires the `dot` feature.
	#[cfg(feature = "dot")]
	#[inline]
	pub fn dot_write_label<W: std::io::Write>(&self, f: &mut W) -> std::io::Result<()> where K: std::fmt::Display, V: std::fmt::Display {
		match self {
			Node::Leaf(leaf) => leaf.dot_write_label(f),
			Node::Internal(node) => node.dot_write_label(f),
			_ => panic!("free node")
		}
	}

	#[cfg(debug_assertions)]
	pub fn validate(&self, min: Option<&K>, max: Option<&K>) where K: Ord {
		match self {
			Node::Leaf(leaf) => leaf.validate(min, max),
			Node::Internal(node) => node.validate(min, max),
			_ => panic!("free node")
		}
	}
}

pub enum Children<'a, K, V> {
	Leaf,
	Internal(Option<usize>, std::slice::Iter<'a, internal::Branch<K, V>>)
}

impl<'a, K, V> Iterator for Children<'a, K, V> {
	type Item = usize;

	fn next(&mut self) -> Option<usize> {
		match self {
			Children::Leaf => None,
			Children::Internal(first, rest) => {
				match first.take() {
					Some(child) => Some(child),
					None => {
						match rest.next() {
							Some(branch) => Some(branch.child),
							None => None
						}
					}
				}
			}
		}
	}
}
