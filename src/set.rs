use crate::BTreeMap;

/// A b-tree set.
///
/// See [std::collections::BTreeSet] for more info.
pub struct BTreeSet<'a, T>(BTreeMap<'a, T, ()>);