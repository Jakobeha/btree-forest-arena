use std::cmp::{
	PartialOrd,
	Ord,
	Ordering
};
use staticvec::StaticVec;
use crate::{
	Item,
	Children,
	Balance,
	WouldUnderflow,
	utils::binary_search_min
};

pub struct Branch<K, V> {
	pub item: Item<K, V>,
	pub child: usize
}

impl<K: PartialEq, V> PartialEq<K> for Branch<K, V> {
	fn eq(&self, key: &K) -> bool {
		self.item.key.eq(key)
	}
}

impl<K: Ord + PartialEq, V> PartialOrd<K> for Branch<K, V> {
	fn partial_cmp(&self, key: &K) -> Option<Ordering> {
		Some(self.item.key.cmp(key))
	}
}

impl<K: PartialEq, V> PartialEq for Branch<K, V> {
	fn eq(&self, other: &Branch<K, V>) -> bool {
		self.item.key.eq(&other.item.key)
	}
}

impl<K: Ord + PartialEq, V> PartialOrd for Branch<K, V> {
	fn partial_cmp(&self, other: &Branch<K, V>) -> Option<Ordering> {
		Some(self.item.key.cmp(&other.item.key))
	}
}

pub struct Internal<K, V, const M: usize> {
	first_child: usize,
	other_children: StaticVec<Branch<K, V>, M /* should be M-1, but it's not supported by Rust. */>
}

impl<K, V, const M: usize> Internal<K, V, M> {
	#[inline]
	pub fn binary(left_id: usize, median: Item<K, V>, right_id: usize) -> Internal<K, V, M> {
		let mut other_children = StaticVec::new();
		other_children.push(Branch {
			item: median,
			child: right_id
		});

		Internal {
			first_child: left_id,
			other_children
		}
	}

	#[inline]
	pub fn item_count(&self) -> usize {
		self.other_children.len()
	}

	#[inline]
	pub fn child_count(&self) -> usize {
		1usize + self.item_count()
	}

	#[inline]
	pub fn child_id(&self, index: usize) -> usize {
		if index == 0 {
			self.first_child
		} else {
			self.other_children[index - 1].child
		}
	}

	#[inline]
	pub fn child_id_opt(&self, index: usize) -> Option<usize> {
		if index == 0 {
			Some(self.first_child)
		} else {
			self.other_children.get(index - 1).map(|b| b.child)
		}
	}

	#[inline]
	pub fn separators(&self, index: usize) -> (Option<&K>, Option<&K>) {
		let min = if index > 0 {
			Some(&self.other_children[index - 1].item.key)
		} else {
			None
		};

		let max = if index < self.other_children.len() {
			Some(&self.other_children[index].item.key)
		} else {
			None
		};

		(min, max)
	}

	#[inline]
	pub fn get(&self, key: &K) -> Result<&V, usize> where K: Ord {
		match binary_search_min(&self.other_children, key) {
			Some(offset) => {
				let b = &self.other_children[offset];
				if &b.item.key == key {
					Ok(&b.item.value)
				} else {
					Err(b.child)
				}
			},
			None => Err(self.first_child)
		}
	}

	#[inline]
	pub fn get_mut(&mut self, key: &K) -> Result<&mut V, usize> where K: Ord {
		match binary_search_min(&self.other_children, key) {
			Some(offset) => {
				let b = &mut self.other_children[offset];
				if &b.item.key == key {
					Ok(&mut b.item.value)
				} else {
					Err(b.child)
				}
			},
			None => Err(self.first_child)
		}
	}

	/// Find the offset of the item matching the given key.
	///
	/// If the key matches no item in this node,
	/// this funtion returns the index and id of the child that may match the key.
	#[inline]
	pub fn offset_of(&self, key: &K) -> Result<usize, (usize, usize)> where K: Ord {
		match binary_search_min(&self.other_children, key) {
			Some(offset) => {
				if &self.other_children[offset].item.key == key {
					Ok(offset)
				} else {
					let id = self.other_children[offset].child;
					Err((offset + 1, id))
				}
			},
			None => Err((0, self.first_child))
		}
	}

	#[inline]
	pub fn children(&self) -> Children<K, V> {
		Children::Internal(Some(self.first_child), self.other_children.as_ref().iter())
	}

	#[inline]
	pub fn item_at_mut(&mut self, offset: usize) -> &mut Item<K, V> {
		&mut self.other_children[offset].item
	}

	#[inline]
	pub fn insert(&mut self, key: K, mut value: V) -> Result<V, (K, V, usize, usize)> where K: Ord {
		match binary_search_min(&self.other_children, &key) {
			Some(i) => {
				if self.other_children[i].item.key == key {
					std::mem::swap(&mut value, &mut self.other_children[i].item.value);
					Ok(value)
				} else {
					Err((key, value, i+1, self.other_children[i].child))
				}
			},
			None => {
				Err((key, value, 0, self.first_child))
			}
		}
	}

	#[inline]
	pub fn insert_node_after(&mut self, i: usize, median: Item<K, V>, node_id: usize) {
		self.other_children.insert(i, Branch {
			item: median,
			child: node_id
		});
	}

	#[inline]
	pub fn split(&mut self) -> Result<(Item<K, V>, Internal<K, V, M>), ()> {
		if self.other_children.len() < M - 1 {
			Err(()) // We don't need to split.
		} else {
			// Index of the median-key item in `other_children`.
			let median_i = (M - 1) / 2; // Since M is at least 4, `median_i` is at least 1.

			let right_other_children = self.other_children.drain(median_i+1..);
			let median = self.other_children.pop().unwrap();

			let right_node = Internal {
				first_child: median.child,
				other_children: right_other_children
			};

			Ok((median.item, right_node))
		}
	}

	#[inline]
	pub fn replace(&mut self, offset: usize, mut item: Item<K, V>) -> Item<K, V> {
		std::mem::swap(&mut item, &mut self.other_children[offset].item);
		item
	}

	/// Merge the children at the given indexes.
	///
	/// It is supposed that `left_index` is `right_index-1`.
	/// This method returns the identifier of the left node in the tree, the identifier of the right node,
	/// the item removed from this node to be merged with the merged children and
	/// the balance status of this node after the merging operation.
	#[inline]
	pub fn merge(&mut self, left_index: usize, right_index: usize) -> (usize, usize, Item<K, V>, Balance) {
		let left_id = self.child_id(left_index);
		let right_id = self.child_id(right_index);

		// We remove the right child (the one of index `right_index`).
		// Since left_index = right_index-1, it is indexed by `left_index` in `other_children`.
		let item = self.other_children.remove(left_index).item;

		let item_count = self.item_count();
		let balancing = if item_count >= M/2 {
			Balance::Balanced
		} else {
			Balance::Underflow(item_count == 0)
		};

		(left_id, right_id, item, balancing)
	}

	#[inline]
	pub fn push_left(&mut self, item: Item<K, V>, child_id: usize) {
		self.other_children.insert(0, Branch {
			item,
			child: self.first_child
		});
		self.first_child = child_id
	}

	#[inline]
	pub fn pop_left(&mut self) -> Result<(Item<K, V>, usize), WouldUnderflow> {
		if self.item_count() < M/2 {
			Err(WouldUnderflow)
		} else {
			let child_id = self.first_child;
			let first = self.other_children.remove(0);
			self.first_child = first.child;
			Ok((first.item, child_id))
		}
	}

	#[inline]
	pub fn push_right(&mut self, item: Item<K, V>, child_id: usize) {
		self.other_children.push(Branch {
			item,
			child: child_id
		})
	}

	#[inline]
	pub fn pop_right(&mut self) -> Result<(Item<K, V>, usize), WouldUnderflow> {
		if self.item_count() < M/2 {
			Err(WouldUnderflow)
		} else {
			let last = self.other_children.pop().unwrap();
			Ok((last.item, last.child))
		}
	}

	#[inline]
	pub fn take(&mut self, offset: usize) -> (usize, Item<K, V>, usize) {
		let left_child_id = self.child_id(offset);
		let b = self.other_children.remove(offset);
		(left_child_id, b.item, b.child)
	}

	#[inline]
	pub fn put(&mut self, offset: usize, item: Item<K, V>, right_child_id: usize) {
		self.other_children.insert(offset, Branch {
			item,
			child: right_child_id
		})
	}

	#[inline]
	pub fn append(&mut self, separator: Item<K, V>, mut other: Internal<K, V, M>) {
		self.other_children.push(Branch {
			item: separator,
			child: other.first_child
		});

		self.other_children.append(&mut other.other_children);
	}

	/// Write the label of the internal node in the DOT format.
	///
	/// Requires the `dot` feature.
	#[cfg(feature = "dot")]
	#[inline]
	pub fn dot_write_label<W: std::io::Write>(&self, f: &mut W) -> std::io::Result<()> where K: std::fmt::Display, V: std::fmt::Display {
		write!(f, "<c0> |")?;
		let mut i = 1;
		for branch in &self.other_children {
			write!(f, "{{{}|<c{}> {}}} |", branch.item.key, i, branch.item.value)?;
			i += 1;
		}

		Ok(())
	}

	#[cfg(debug_assertions)]
	pub fn validate(&self, min: Option<&K>, max: Option<&K>) where K: Ord {
		if min.is_some() || max.is_some() { // not root
			if self.item_count() < (M/2 - 1) {
				panic!("internal node is underflowing")
			}

			if self.item_count() >= M {
				panic!("internal node is overflowing")
			}
		} else {
			if self.item_count() == 0 {
				panic!("root node is empty")
			}
		}

		if !self.other_children.is_sorted() {
			panic!("internal node items are not sorted")
		}

		if let Some(min) = min {
			if let Some(b) = self.other_children.first() {
				if min >= &b.item.key {
					panic!("internal node item key is greater than right separator")
				}
			}
		}

		if let Some(max) = max {
			if let Some(b) = self.other_children.last() {
				if max <= &b.item.key {
					panic!("internal node item key is less than left separator")
				}
			}
		}
	}
}
