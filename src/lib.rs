#![doc = include_str!("../README.md")]

/// Generic B-Tree types.
///
/// Types defined in this module are independent of the actual storage type.
pub mod generic;
/// Misc utility functions
#[cfg(any(doc, feature = "utils"))]
pub mod utils;
#[cfg(not(any(doc, feature = "utils")))]
mod utils;
/// b-tree which stores its data in an owned slab
#[cfg(any(doc, feature = "slab"))]
pub mod slab;
/// b-tree which stores its data in a reference to [shareable_slab::ShareableSlab], so its store can
/// be shared with other b-trees
#[cfg(any(doc, feature = "shareable-slab"))]
pub mod shareable_slab;
/// b-tree which stores its data in a reference to [concurrent_shareable_slab::ShareableSlab], which
/// can be shared across threads (implements `Sync`) via a read-write lock
#[cfg(any(doc, feature = "concurrent-shareable-slab"))]
pub mod concurrent_shareable_slab;
/// b-tree which stores its data in a reference to
/// [shareable_slab_simultaneous_mutation::ShareableSlab], which is faster and allows concurrent
/// access and mutation, but *panics* under insertion.
#[cfg(any(doc, feature = "shareable-slab-simultaneous-mutation"))]
pub mod shareable_slab_simultaneous_mutation;
/// b-tree which stores its data in a reference to [shareable_slab_arena::ShareableSlab], which uses
/// an arena allocator to allow simultaneous access, mutation, and insertion.
#[cfg(any(doc, feature = "shareable-slab-arena"))]
pub mod shareable_slab_arena;
