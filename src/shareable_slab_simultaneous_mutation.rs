use std::cell::UnsafeCell;
use std::fmt::{Debug, Formatter};
use std::mem::{forget, replace};
use std::ops::{Deref, DerefMut};
use std::ptr::{addr_of, addr_of_mut};
use crate::generic::StoreView;

/// B-Tree map based on [Store].
pub type BTreeMap<'a, K, V> = crate::generic::BTreeMap<K, V, usize, &'a Store<crate::generic::Node<K, V, usize>>>;

/// B-Tree set based on [Store].
pub type BTreeSet<'a, T> = crate::generic::BTreeSet<T, usize, &'a Store<crate::generic::Node<T, (), usize>>>;

/// Shareable storage implemented via `UnsafeCell`. Can be shared by multiple b-trees, and multiple
/// b-trees can simultaneously access, mutate, and delete entries, but *panics* if there is an
/// insertion while elements are being accessed or mutated.
pub struct Store<T>(UnsafeCell<_Store<T>>);

type VecRawParts<T> = (*mut T, usize, usize);

struct _Store<T> {
    /// Chunk of memory. We have to store the vector as raw parts so that we can get pointers to
    /// elements without entirely borrowing it to call `Vec::as_ptr`. When we need to perform `Vec`
    /// operations, we're in a situation where we can borrow the entire struct, so we call
    /// `Vec::from_raw_parts`, perform the operation, and (except when dropping) re-deconstruct it.
    entries: VecRawParts<Entry<T>>,
    /// Number of Filled elements currently in the slab
    len: usize,
    /// Offset of the next available slot in the slab. Set to the slab's
    /// capacity when the slab is full.
    next: usize,
    /// How many references to slab elements are there right now?
    num_active_refs: usize
}

#[derive(Clone)]
enum Entry<T> {
    /// A value is present
    Occupied { value: T },
    /// A value is not present
    Vacant { next_free: usize },
}

#[derive(Debug)]
pub struct Ref<'a, T: ?Sized> {
    /// `elem` is `Some` unless this gets consumed by `map` or `try_map`, in which case we don't
    /// want to decrease `num_active_refs` when this ref gets dropped, because the mapped ref will
    elem: Option<&'a T>,
    num_active_refs: *mut usize,
}

#[derive(Debug)]
pub struct RefMut<'a, T: ?Sized> {
    /// `elem` is `Some` unless this gets consumed by `map` or `try_map`, in which case we don't
    /// want to decrease `num_active_refs` when this ref gets dropped, because the mapped ref will
    elem: Option<&'a mut T>,
    num_active_refs: *mut usize,
}

impl<T> Store<T> {
    pub fn new() -> Self {
        Self(UnsafeCell::new(_Store {
            entries: Vec::new().into_raw_parts_stable(),
            len: 0,
            next: 0,
            num_active_refs: 0
        }))
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(UnsafeCell::new(_Store {
            entries: Vec::with_capacity(capacity).into_raw_parts_stable(),
            len: 0,
            next: 0,
            num_active_refs: 0
        }))
    }

    #[inline]
    fn get_ref_pointers_and_increment_num_refs(
        &self,
        index: usize
    ) -> Option<(*mut T, *mut usize)> {
        let ptr = self.0.get();
        // SAFETY: These operations are "atomic", and there's no parallel access since we're in
        // `UnsafeCell`
        let (entries_start, num_entries) = unsafe { (*addr_of_mut!((*ptr).entries.0), *addr_of!((*ptr).entries.1)) };
        if num_entries < index {
            // Note: This won't happen since this structure is only used by b-trees, and they
            // don't get empty elements, but we return None to comply with the actual trait
            // implementation
            return None
        }
        // SAFETY: Assuming this is only used by b-trees, there are no mutable references to
        // elem, because each b-tree effectively owns its elements and its borrow live ranges
        // are a superset. We also checked that index is in bounds
        let entry = unsafe { &*entries_start.add(index) };
        match entry {
            Entry::Occupied { value } => {
                // SAFETY: "Atomic", and no parallel access since we're in `UnsafeCell`
                unsafe { *addr_of_mut!((*ptr).num_active_refs) += 1 };
                Some((value as *const T as *mut T, unsafe { addr_of_mut!((*ptr).num_active_refs) }))
            }
            Entry::Vacant { .. } => {
                None
            }
        }
    }

    #[inline]
    fn assert_no_refs(&self) {
        let ptr = self.0.get();
        // SAFETY: "Atomic", and no parallel access since we're in `UnsafeCell`
        let num_active_refs = unsafe { *addr_of!((*ptr).num_active_refs) };
        assert_eq!(
            num_active_refs, 0,
            "Attempted to insert into a ShareableSlab while there are active references to its elements"
        );
    }
}

impl<T> Debug for Store<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let ptr = self.0.get();
        f.debug_struct("ShareableSlab")
            .field("entries", unsafe { &*addr_of!((*ptr).entries) })
            .field("len", unsafe { &*addr_of!((*ptr).len) })
            .field("next", unsafe { &*addr_of!((*ptr).next) })
            .field("num_active_refs", unsafe { &*addr_of!((*ptr).num_active_refs) })
            .finish()
    }
}

impl<T> Default for Store<T> {
    fn default() -> Self {
        Store::new()
    }
}

impl<T> _Store<T> {
    #[inline]
    fn with_entries<Return>(&self, op: impl FnOnce(&Vec<Entry<T>>) -> Return) -> Return {
        let vec = self.materialize_vec();
        let result = op(&vec);
        forget(vec);
        result
    }

    #[inline]
    fn with_entries_mut<Return>(&mut self, op: impl FnOnce(&mut Vec<Entry<T>>) -> Return) -> Return {
        let mut vec = self.materialize_vec();
        let result = op(&mut vec);
        self.entries = vec.into_raw_parts_stable();
        result
    }

    #[inline]
    fn materialize_vec(&self) -> Vec<Entry<T>> {
        let (ptr, length, capacity) = self.entries;
        // SAFETY: These are a `Vec`'s raw parts
        unsafe { Vec::from_raw_parts(ptr, length, capacity) }
    }
}

impl<T> Drop for _Store<T> {
    fn drop(&mut self) {
        assert_eq!(self.num_active_refs, 0, "Dropping a ShareableSlab with active references");
        drop(self.materialize_vec())
    }
}

impl<'a, T> StoreView<T> for &'a Store<T> {
    type Index = usize;
    type Ref<'b, U: ?Sized + 'b> = Ref<'b, U> where Self: 'b;

    #[inline]
    fn get(&self, index: Self::Index) -> Option<Self::Ref<'_, T>> {
        self.get_ref_pointers_and_increment_num_refs(index).map(|(elem, num_active_refs)| Ref {
            // SAFETY: Assuming this is only used by b-trees, there are no mutable references to
            // elem, because each b-tree effectively owns its elements and its borrow live ranges
            // are a superset
            elem: Some(unsafe { &*elem }),
            num_active_refs
        })
    }
}

impl<'a, T> crate::generic::store::Store<T> for &'a Store<T> {
    type RefMut<'b, U: ?Sized + 'b> = RefMut<'b, U> where Self: 'b;

    #[inline]
    fn insert(&mut self, value: T) -> Self::Index {
        self.assert_no_refs();
        // SAFETY: We just checked, there are no refs
        let this = unsafe { &mut *self.0.get() };
        let key = this.next;

        // Code from slab::Slab::insert_at
        this.len += 1;
        if key == this.with_entries(|e| e.len()) {
            this.with_entries_mut(|e| e.push(Entry::Occupied { value }));
            this.next = key + 1;
        } else {
            this.next = this.with_entries(|e| match e.get(key) {
                Some(&Entry::Vacant { next_free }) => next_free,
                _ => unreachable!(),
            });
            this.with_entries_mut(|e| e[key] = Entry::Occupied { value });
        }

        key
    }

    #[inline]
    fn remove(&mut self, index: Self::Index) -> Option<T> {
        // We can't borrow self entirely, because there may be active mutable references to some
        // parts
        let ptr = self.0.get();
        // SAFETY: However, we can assert that the entries start pointer and length aren't being
        // written to (no concurrency in `UnsafeCell`)
        let (entries_start, num_entries) = unsafe { (*addr_of_mut!((*ptr).entries.0), *addr_of!((*ptr).entries.1)) };
        // Just sanity checking the bounds, should always succeed when used by b-trees but we return
        // None to match the signature and prevent confusing bugs if we use extensively (see ???
        // in `Self::get_ref_pointers_and_increment_num_refs`)
        if index >= num_entries {
            return None;
        }
        // SAFETY: Moreover, we can assert that index is unoccupied because we have a mutable
        // reference to the b-tree which owns that particular element (same reason get_mut can take
        // a mutable reference) and we just checked bounds
        let entry = unsafe { &mut *entries_start.add(index) };
        // SAFETY: Here we do more "atomic" dereferences and assignments, which are safe since there's
        // no concurrency in `UnsafeCell`
        unsafe {
            // Swap the entry at the provided value
            let prev = replace(entry, Entry::Vacant { next_free: *addr_of!((*ptr).next) });
            match prev {
                Entry::Occupied { value } => {
                    *addr_of_mut!((*ptr).len) -= 1;
                    *addr_of_mut!((*ptr).next) = index;
                    Some(value)
                }
                _ => {
                    // Whoops, the entry is actually vacant, restore the state
                    // (remember, b-trees don't actually call remove on vacant entries, so this
                    // code path should never actually happen)
                    *entry = prev;
                    None
                }
            }
        }
    }

    #[inline]
    fn get_mut(&mut self, index: Self::Index) -> Option<Self::RefMut<'_, T>> {
        self.get_ref_pointers_and_increment_num_refs(index).map(|(elem, num_active_refs)| RefMut {
            // SAFETY: Assuming this is only used by b-trees, there are no other references to
            // elem, because each b-tree effectively owns its elements and its borrow live ranges
            // are a superset
            elem: Some(unsafe { &mut *elem }),
            num_active_refs
        })
    }

    #[inline]
    fn clear_fast(&mut self) -> bool {
        // Not owned
        false
    }
}

impl<'a, T: ?Sized> Deref for Ref<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // `elem` will only be `None` when this is being consumed, and we don't deref then
        self.elem.as_ref().unwrap()
    }
}

impl<'a, T: ?Sized> crate::generic::store::Ref<'a, T> for Ref<'a, T> {
    type Mapped<U: ?Sized + 'a> = Ref<'a, U>;

    #[inline]
    fn map<U: ?Sized>(mut self, f: impl FnOnce(&T) -> &U) -> Self::Mapped<U> {
        // `elem` will only be `None` when this is being consumed, which is now; we can't/won't call
        // `map` on `self` again.
        let elem = self.elem.take().unwrap();
        Ref {
            elem: Some(f(elem)),
            num_active_refs: self.num_active_refs
        }
    }

    #[inline]
    fn try_map<U: ?Sized>(
        mut self,
        f: impl FnOnce(&T) -> Option<&U>
    ) -> Result<Self::Mapped<U>, Self> where Self: Sized {
        // `elem` will only be `None` when this is being consumed, which is now; we can't/won't call
        // `map` on `self` again until/unless we put it back and return `self`.
        let elem = self.elem.take().unwrap();
        match f(elem) {
            None => {
                self.elem = Some(elem);
                Err(self)
            },
            Some(elem) => Ok(Ref {
                elem: Some(elem),
                num_active_refs: self.num_active_refs
            })
        }
    }

    #[inline]
    fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
        r#ref: <Self::Mapped<U> as crate::generic::store::Ref<'a, U>>::Mapped<V>
    ) -> Self::Mapped<V> {
        r#ref
    }
}

impl<'a, T: ?Sized> Drop for Ref<'a, T> {
    fn drop(&mut self) {
        // If `elem` is `None` we mapped `self`, so don't decrement because the mapped ref will
        if self.elem.is_some() {
            // SAFETY: "Atomic", and no parallel access since we're in `UnsafeCell`
            unsafe { *self.num_active_refs -= 1; }
        }
    }
}

impl<'a, T: ?Sized> Deref for RefMut<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // `elem` will only be `None` when this is being consumed, and we don't deref then
        self.elem.as_ref().unwrap()
    }
}


impl<'a, T: ?Sized> DerefMut for RefMut<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // `elem` will only be `None` when this is being consumed, and we don't deref then
        self.elem.as_mut().unwrap()
    }
}

impl<'a, T: ?Sized> crate::generic::store::RefMut<'a, T> for RefMut<'a, T> {
    type Mapped<U: ?Sized + 'a> = RefMut<'a, U>;

    #[inline]
    fn map<U: ?Sized>(mut self, f: impl FnOnce(&mut T) -> &mut U) -> Self::Mapped<U> {
        // `elem` will only be `None` when this is being consumed, which is now; we can't/won't call
        // `map` on `self` again.
        let elem = self.elem.take().unwrap();
        RefMut {
            elem: Some(f(elem)),
            num_active_refs: self.num_active_refs
        }
    }

    #[inline]
    fn try_map<U: ?Sized>(
        mut self,
        f: impl FnOnce(&mut T) -> Option<&mut U>
    ) -> Result<Self::Mapped<U>, Self> where Self: Sized {
        // `elem` will only be `None` when this is being consumed, which is now; we can't/won't call
        // `map` on `self` again until/unless we put it back and return `self`.
        let elem = self.elem.take().unwrap();
        let elem_ptr = elem as *mut T;
        match f(elem) {
            None => {
                // SAFETY: self.elem is no longer borrowed because `None`, but Rust doesn't realize,
                // so we must set it unsafely (see the `RefMut::try_map` impl on `&mut T`)
                self.elem = Some(unsafe { &mut *elem_ptr });
                Err(self)
            },
            Some(elem) => Ok(RefMut {
                elem: Some(elem),
                num_active_refs: self.num_active_refs
            })
        }
    }

    #[inline]
    fn cast_map_transitive<U: ?Sized + 'a, V: ?Sized>(
        r#ref: <Self::Mapped<U> as crate::generic::store::RefMut<'a, U>>::Mapped<V>
    ) -> Self::Mapped<V> {
        r#ref
    }
}

impl<'a, T: ?Sized> Drop for RefMut<'a, T> {
    fn drop(&mut self) {
        // If `elem` is `None` we mapped `self`, so don't decrement because the mapped ref will
        if self.elem.is_some() {
            // SAFETY: "Atomic", and no parallel access since we're in `UnsafeCell`
            unsafe { *self.num_active_refs -= 1; }
        }
    }
}

// region stable versions of unstable functions
trait IntoRawPartsStable<T> {
    /// [Vec::into_raw_parts] but stable
    fn into_raw_parts_stable(self) -> VecRawParts<T>;
}

impl<T> IntoRawPartsStable<T> for Vec<T> {
    #[inline]
    fn into_raw_parts_stable(mut self) -> VecRawParts<T> {
        let ptr = self.as_mut_ptr();
        let len = self.len();
        let cap = self.capacity();
        forget(self);
        (ptr, len, cap)
    }
}
// endregion