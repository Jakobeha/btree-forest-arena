#![doc = include_str!("../README.md")]

pub use map::BTreeMap;
pub use set::BTreeSet;
pub use store::BTreeStore;

/// Immutable map and set which implement [Copy] but don't drop or deallocate its contents; instead,
/// the store has a new helper which performs a special variant of
/// [tracing garbage collection](https://en.wikipedia.org/wiki/Tracing_garbage_collection)
#[cfg(feature = "copyable")]
pub mod copyable;
mod cursor;
pub mod map;
mod node;
pub mod set;
mod store;
/// Misc utility functions
mod utils;
