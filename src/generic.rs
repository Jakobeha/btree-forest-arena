//! Generic B-Tree types.
//!
//! Types defined in this module are independent of the actual storage type.

pub mod node;
pub use node::Node;

pub mod map;
pub use map::BTreeMap;

pub mod set;
pub use set::BTreeSet;

pub mod slab;
pub use self::slab::{SlabView, Slab};
