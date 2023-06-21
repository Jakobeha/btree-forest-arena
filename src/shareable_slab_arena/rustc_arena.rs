// TODO: move into its own crate and add tests for untested (e.g. dropless arena)
#![allow(unused)]

use std::alloc::Layout;
use std::cell::{Cell, RefCell};
use std::cmp::max;
use std::fmt::{Debug, Formatter};
use std::iter::repeat_with;
use std::marker::PhantomData;
use std::mem::{align_of, forget, MaybeUninit, needs_drop, size_of, transmute};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::ptr::{drop_in_place, NonNull, null_mut, slice_from_raw_parts_mut, write};
use std::slice::{from_raw_parts, from_raw_parts_mut};

use smallvec::SmallVec;

#[cfg(test)]
mod tests;

/// An arena that can hold objects of only one type.
pub struct TypedArena<T> {
    /// The number of inserted entries
    len: Cell<usize>,
    /// A pointer to the next object to be allocated.
    ptr: Cell<*mut T>,
    /// A pointer to the end of the allocated area. When this pointer is
    /// reached, a new chunk is allocated.
    end: Cell<*mut T>,
    /// A vector of arena chunks.
    chunks: RefCell<Vec<ArenaChunk<T>>>,
    /// The # of chunks actually used by the arena. The rest were allocated but are now empty,
    /// and we will try to re-use them before allocating a new chunk.
    used_chunks: Cell<usize>,
    /// Marker indicating that dropping the arena causes its owned
    /// instances of `T` to be dropped.
    _own: PhantomData<T>,
}

/// An arena that can hold objects of multiple different types that impl `Copy`
/// and/or satisfy `!mem::needs_drop`.
pub struct DroplessArena {
    /// A pointer to the start of the free space.
    start: Cell<*mut u8>,
    /// A pointer to the end of free space.
    ///
    /// The allocation proceeds downwards from the end of the chunk towards the
    /// start. (This is slightly simpler and faster than allocating upwards,
    /// see <https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html>.)
    /// When this pointer crosses the start pointer, a new chunk is allocated.
    end: Cell<*mut u8>,
    /// A vector of arena chunks.
    chunks: RefCell<Vec<ArenaChunk>>,
}

struct ArenaChunk<T = u8> {
    /// Pointer to raw storage for the arena chunk.
    storage: *mut T,
    /// The number of valid entries in the chunk (the `len` of storage), **except** the last chunk's
    /// entries is always 0 (unset), because it is being filled and we don't want to access the
    /// [ArenaChunk] on the fast path, we want to be able to only access its memory. **Also**,
    /// [DroplessArena] doesn't use this, so all of its chunks' `entries` is 0
    entries: usize,
    /// \# of elements the raw storage can hold AKA size of the raw storage allocation
    capacity: usize,
}

/// Iterates all elements in an arena, and can handle new elements being added.
pub type ArenaIter<'a, T> = ArenaGenIter<'a, T, true>;

/// Iterates pointers to all elements in the arena, and can handle new elements being added.
pub type ArenaPtrIter<'a, T> = ArenaGenIter<'a, T, false>;

/// Iterates all elements in an arena, and can handle new elements being added.
///
/// `ITER_REF` determines whether or not this iterates raw [NonNull] pointers or references.
pub struct ArenaGenIter<'a, T, const ITER_REF: bool> {
    /// The arena being iterated
    arena: &'a TypedArena<T>,
    /// Index of the current chunk being iterated
    chunk_index: usize,
    /// Pointer to the next entry in the current chunk being iterated
    chunk_data: NonNull<T>,
    /// Entries remaining in the current chunk being iterated, **except** like [ArenaChunk], if we
    /// are iterating the last chunk, this will be 0 (unset) even though we have more entries
    chunk_remaining_entries: usize,
    /// Index in the arena of the current element being iterated
    element_index: usize
}

pub trait IterWithFastAlloc<T> {
    fn alloc_into(self, arena: &TypedArena<T>) -> &[T];
}

// The arenas start with PAGE-sized chunks, and then each new chunk is twice as
// big as its predecessor, up until we reach HUGE_PAGE-sized chunks, whereupon
// we stop growing. This scales well, from arenas that are barely used up to
// arenas that are used for 100s of MiBs. Note also that the chosen sizes match
// the usual sizes of pages and huge pages on Linux.
const PAGE: usize = 4096;
const HUGE_PAGE: usize = 2 * 1024 * 1024;

impl<T> Default for TypedArena<T> {
    /// Creates a new, empty arena
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> TypedArena<T> {
    /// Creates a new, empty arena
    #[inline]
    pub fn new() -> Self {
        Self {
            len: Cell::new(0),
            // We set both `ptr` and `end` to 0 so that the first call to
            // alloc() will trigger a grow().
            ptr: Cell::new(null_mut()),
            end: Cell::new(null_mut()),
            chunks: Default::default(),
            used_chunks: Cell::new(0),
            _own: PhantomData,
        }
    }

    /// Allocates an object in the `TypedArena`, returning a reference to it.
    ///
    /// Unlike `rustc`'s arena, we only return shared references, because we also allow iterating
    /// all elements behind a shared reference.
    #[inline]
    pub fn alloc(&self, object: T) -> &T {
        self.len.set(self.len.get() + 1);
        if size_of::<T>() == 0 {
            // We don't actually allocate ZSTs, just prevent them from being dropped and return a
            // reference to random data (this is a valid ZST reference).
            unsafe {
                let ptr = NonNull::<T>::dangling().as_ptr();
                // This `write` is equivalent to `forget`
                write(ptr, object);
                return &*ptr
            }
        }

        if self.ptr == self.end {
            self.grow(1)
        }

        unsafe {
            let ptr = self.ptr.get();
            // Advance the pointer.
            self.ptr.set(self.ptr.get().add(1));
            // Write into uninitialized memory.
            write(ptr, object);
            &*ptr
        }
    }

    /// Allocates multiple objects in a contiguous slice, returning a reference to the slice.
    ///
    /// Unlike `rustc`'s arena, we only return shared references, because we also allow iterating
    /// all elements behind a shared reference.
    /// 
    /// This collects into a `SmallVec` and then allocates by copying from it. Use `alloc_from_iter`
    /// if possible because it's more efficient, copying directly without the intermediate
    /// collecting step. This default could be made more efficient, like
    /// [DroplessArena::alloc_from_iter], but it's not hot enough to bother.
    #[inline]
    pub fn alloc_from_iter(&self, iter: impl IntoIterator<Item=T>) -> &[T] {
        self.alloc_from_iter_fast(iter.into_iter().collect::<SmallVec<[_; 8]>>())
    }

    /// Allocates multiple objects in a contiguous slice, returning a reference to the slice.
    ///
    /// Unlike `rustc`'s arena, we only return shared references, because we also allow iterating
    /// all elements behind a shared reference.
    /// 
    /// This is equivalent semantics to [Self::alloc_from_iter] except it's faster, whereas
    /// [Self::alloc_from_iter] permits more types.
    #[inline]
    fn alloc_from_iter_fast(&self, iter: impl IterWithFastAlloc<T>) -> &[T] {
        assert_ne!(size_of::<T>(), 0);
        iter.alloc_into(self)
    }

    /// Returns the number of allocated elements in the arena.
    #[inline]
    pub fn len(&self) -> usize {
        self.len.get()
    }

    /// Iterates all allocated elements in the arena.
    ///
    /// The iterator can handle new objects being allocated. If you allocate new objects they will
    /// be added to the end. If the iterator has already ended and you allocate new objects, it will
    /// suddenly have more elements; if you don't want that behavior use `fuse`.
    #[inline]
    pub fn iter(&self) -> ArenaIter<'_, T> {
        ArenaIter::new(self)
    }

    /// Iterates pointers to all allocated elements in the arena.
    ///
    /// The iterator can handle new objects being allocated. If you allocate new objects they will
    /// be added to the end. If the iterator has already ended and you allocate new objects, it will
    /// suddenly have more elements; if you don't want that behavior use `fuse`.
    #[inline]
    pub fn ptr_iter(&self) -> ArenaPtrIter<'_, T> {
        ArenaPtrIter::new(self)
    }

    /// Clears the arena, dropping all elements, but doesn't free up its memory.
    ///
    /// This means we can insert new elements without having to reallocate, until we reach the old
    /// capacity or allocate a slice too large to fit in an existing region.
    #[inline]
    pub fn clear(&mut self) {
        // Ensure that, even on panic, we resize len (we leak elements we didn't drop yet instead of
        // double-freeing elements we did)
        let panic_result = catch_unwind(AssertUnwindSafe(|| {
            for elem in self.ptr_iter() {
                // SAFETY: we're shrinking the arena, so we A) won't drop later if we drop the arena
                // before growing it again, and B) if we do grow it again, we'll overwrite this data
                // before setting it to "initialized" (we might also grow past this data but it will
                // still be uninitialized and therefore not dropped).
                //
                // Also, elem.as_ptr() is alive, and we have the only reference since we have a mutable
                // reference to the entire arena.
                unsafe { drop_in_place(elem.as_ptr()); }
            }
        }));

        // This code will run even if we panic
        // Update len, num used chunks, used chunk entries, ptr, and end
        self.len.set(0);
        if size_of::<T>() != 0 {
            for chunk in self.chunks.borrow_mut().iter_mut().take(self.used_chunks.get()) {
                chunk.entries = 0;
            }
            self.used_chunks.set(0);
            // ptr and end can be null and we'll still reuse instead of allocating new chunks
            self.ptr.set(null_mut());
            self.end.set(null_mut());
        }


        // Still unwind if we panicked
        if let Err(caught_panic) = panic_result {
            resume_unwind(caught_panic)
        }
    }

    /// Removes some elements from this arena, and coalesces the rest so that we don't have gaps.
    ///
    /// Pointers to regions in the memory may be invalidated as elements get rearranged. This
    /// function is behind a mutable reference, which ensures that there are no references to
    /// rearranged elements, but if there are any raw pointers they can no longer be dereferenced
    /// without UB.
    #[inline]
    pub fn retain(&mut self, mut predicate: impl FnMut(&T) -> bool) {
        // Ensure that, even on panic, we resize len (we leak elements we didn't drop yet instead of
        // double-freeing elements we did). Furthermore, kept elements are still in the arena,
        // although this doesn't really matter and is subject to change between versions.
        let mut num_kept = 0;
        let panic_result = catch_unwind(AssertUnwindSafe(|| {
            let mut write_iter = self.ptr_iter();
            let mut is_write_iter_at_read_iter = true;
            for elem in self.ptr_iter() {
                let elem_ptr = elem.as_ptr();
                // SAFETY: elem is alive (Self::iter and Self::ptr_iter only iterate initialized data)
                // and we have a mutable reference to the arena, so there are no other references to
                // elem. Therefore, we can dereference and drop elem_ptr.
                //
                // write_ptr is allocated (inside this struct) and aligned (came from Self::ptr_iter).
                // It has previously pointed to a live object since it has been elem_ptr, but we may
                // have dropped that elem_ptr so it's no longer alive. However, we can still write to
                // it.
                //
                // Lastly, we can read from elem_ptr when we write to write_ptr (effectively copying the
                // value) because we will either overwrite the value when write_ptr advances to it, or
                // (if elem_ptr advances to the end first) we will shrink the arena to be before it, so
                // that it is effectively forgotten; and then it will either be re-allocated if we grow
                // the arena again, or released without drop if we drop the arena.
                unsafe {
                    if !predicate(elem.as_ref()) {
                        // Drop the element, keep write_iter at the same position
                        is_write_iter_at_read_iter = false;
                        drop_in_place(elem_ptr);
                    } else {
                        // Keep the element, but move it to write_iter if unaligned. Advance write_iter
                        num_kept += 1;

                        // If write_chunk can hold more elements (length < capacity), we should
                        // desync write_iter from read_iter and do so (length = capacity)
                        if write_iter.chunk_remaining_entries == 1 {
                            let mut chunks = self.chunks.borrow_mut();
                            let write_chunk = chunks.get_mut(write_iter.chunk_index)
                                .expect("write_iter chunk index out of bounds");
                            let difference = write_chunk.capacity - write_chunk.entries;
                            if difference > 0 {
                                is_write_iter_at_read_iter = false;
                                debug_assert_eq!(
                                    write_chunk.entries > 0,
                                    write_iter.chunk_remaining_entries > 0
                                );
                                // If write_chunk is the last chunk its entries are unset (0), but
                                // if not we need to update the count. We also need to update
                                // write_iter's count so that it won't reach the chunk end until it
                                // reaches write_chunk's capacity.
                                if write_chunk.entries > 0 {
                                    write_chunk.entries += difference;
                                    write_iter.chunk_remaining_entries += difference;
                                }
                                // Even if elem_iter (the implicit iterator returning elem) is
                                // synced, we still want it to move on, not read the chunk's
                                // remaining memory because it;s uninitialized
                            }
                        }

                        if size_of::<T>() != 0 && !is_write_iter_at_read_iter {
                            let write_ptr = write_iter.next()
                                .expect("read_iter not done but write_iter is, write_iter should always be behind")
                                .as_ptr();
                            write_ptr.write(elem_ptr.read());
                        }
                    }
                }
            }
        }));

        // This code will run even if we panic
        // Update len, num used chunks, used chunk entries, ptr, and end
        let old_len = self.len.get();
        self.len.set(num_kept);
        if size_of::<T>() != 0 {
            let mut chunks = self.chunks.borrow_mut();
            let mut num_entries = 0;
            let used_chunks = chunks.iter().take_while(|chunk| {
                if num_entries < num_kept {
                    num_entries += chunk.entries;
                    true
                } else {
                    false
                }
            }).count();
            if num_entries < num_kept {
                debug_assert_eq!(used_chunks, self.used_chunks.get());
                num_entries = old_len;
            } else {
                self.used_chunks.set(used_chunks);
            }
            if used_chunks == 0 {
                // These assertions are pretty obvious
                debug_assert_eq!((num_entries, num_kept), (0, 0));
                self.ptr.set(null_mut());
                self.end.set(null_mut());
            } else {
                let num_in_last = num_entries - num_kept;
                let mut last_chunk = &mut chunks[used_chunks - 1];
                // This is the last chunk, so unset (0) its entries, even though there actually are some
                last_chunk.entries = 0;
                // Set ptr and end to this chunk, and make sure ptr is offset past the existing entries
                self.ptr.set(unsafe { last_chunk.storage.add(num_in_last) });
                self.ptr.set(last_chunk.end());
            }
        }

        // Still unwind if we panicked
        if let Err(caught_panic) = panic_result {
            resume_unwind(caught_panic)
        }
    }

    /// Destroys this arena and collects all elements into a vector.
    #[inline]
    pub fn into_vec(self) -> Vec<T> {
        let mut elements = Vec::with_capacity(self.len());
        if size_of::<T>() == 0 {
            // Create `len` ZSTs which will be dropped when the vector is.
            // Remember: a random non-null pointer is a valid reference to a ZST, and dereferencing
            // is probably a no-op
            elements.extend((0..self.len()).map(|_| unsafe { NonNull::<T>::dangling().as_ptr().read() }));
            return elements;
        }

        let mut remaining = self.len();
        let mut chunks_borrow = self.chunks.borrow_mut();
        let mut prev_chunk = None;
        for chunk in chunks_borrow.iter_mut().take(self.used_chunks.get()) {
            if let Some(prev_chunk) = prev_chunk.replace(chunk) {
                // SAFETY: This chunk has all entries filled because we've moved on to the next one
                //   (and we resize the chunk's entries when we move on, even though it has more capacity).
                let mut prev_entries = unsafe { prev_chunk.destroy_and_return(prev_chunk.entries) };
                elements.append(&mut prev_entries);
                remaining -= prev_chunk.entries;
            }
        }
        if let Some(last_chunk) = prev_chunk {
            // SAFETY: This chunk only has `remaining` entries filled
            let mut last_entries = unsafe { last_chunk.destroy_and_return(remaining) };
            elements.append(&mut last_entries);
        }
        // Ensure we don't destroy these chunks' contents in `Drop`, only free their memory
        self.used_chunks.set(0);
        elements
    }

    /// Checks if `additional` elements can be inserted into the arena without creating a new chunk
    #[inline]
    fn can_allocate(&self, additional: usize) -> bool {
        debug_assert_ne!(size_of::<T>(), 0);
        // FIXME: this should *likely* use `offset_from`, but more
        //   investigation is needed (including running tests in miri).
        let available_bytes = self.end.get().addr_() - self.ptr.get().addr_();
        let additional_bytes = additional.checked_mul(size_of::<T>()).unwrap();
        available_bytes >= additional_bytes
    }

    /// Ensures there's enough space in the current chunk to fit `len` objects. If not, it will
    /// create a new chunk.
    #[inline]
    fn ensure_capacity(&self, additional: usize) {
        if !self.can_allocate(additional) {
            self.grow(additional);
            debug_assert!(self.can_allocate(additional));
        }
    }

    /// Allocate a contiguous slice of data and return a pointer to the start of the slice. The
    /// slice is uninitialized (why we return a pointer), and you must initialize it before calling
    /// other arena methods or dropping the arena, or you will cause UB.
    #[inline]
    unsafe fn alloc_raw_slice(&self, len: usize) -> *mut T {
        assert_ne!(len, 0);

        self.len.set(self.len.get() + len);

        if size_of::<T>() == 0 {
            // ZSTs have no memory, so we won't allocate.
            // Remember: a random non-null pointer is a valid reference to a ZST
            return NonNull::<T>::dangling().as_ptr();
        }
        self.ensure_capacity(len);

        let start_ptr = self.ptr.get();
        self.ptr.set(start_ptr.add(len));
        start_ptr
    }

    /// Grows the arena = creates a new chunk which will hold at least `additional` elements,
    /// or reuses a chunk if we have extras.
    #[inline(never)]
    #[cold]
    fn grow(&self, additional: usize) {
        debug_assert_ne!(size_of::<T>(), 0);
        let used_chunks = self.used_chunks.get();
        let mut chunks = self.chunks.borrow_mut();
        let mut reused_a_chunk = false;
        for potential_reuse_idx in used_chunks..chunks.len() {
            let potential_reuse_chunk = &mut chunks[potential_reuse_idx];
            if potential_reuse_chunk.capacity >= additional {
                // We found a chunk that can hold the additional elements, so we'll use it.
                // Make sure to update the # entries; since this is the last chunk, we unset (0) it
                // even though there are actually additional (see `ArenaChunk.entries` doc)
                potential_reuse_chunk.entries = 0;
                // Set ptr and end to the reused chunk
                self.ptr.set(potential_reuse_chunk.storage);
                self.end.set(potential_reuse_chunk.end());
                if used_chunks != potential_reuse_idx {
                    // We have to ensure the reused chunk is the next one
                    chunks.swap(used_chunks, potential_reuse_idx);
                }
                reused_a_chunk = true;
                break;
            }
        }

        if !reused_a_chunk {
            // Actually grow = insert a chunk at used_chunks with the required capacity
            unsafe {
                // We need the element size to convert chunk sizes (ranging from
                // PAGE to HUGE_PAGE bytes) to element counts.
                let elem_size = max(1, size_of::<T>());
                let mut new_cap;
                if let Some(last_chunk) = used_chunks.checked_sub(1).map(|i| &mut chunks[i]) {
                    // If a type is `!needs_drop`, we don't need to keep track of how many elements
                    // the chunk stores - the field will be ignored anyway.
                    // FIXME: this should *likely* use `offset_from`, but more
                    //   investigation is needed (including running tests in miri).
                    let used_bytes = self.ptr.get().addr_() - last_chunk.storage.addr_();
                    // Set # entries since this is no longer the last chunk
                    last_chunk.entries = used_bytes / size_of::<T>();

                    // If the previous chunk's capacity is less than HUGE_PAGE
                    // bytes, then this chunk will be least double the previous
                    // chunk's size.
                    new_cap = last_chunk.capacity.min(HUGE_PAGE / elem_size / 2);
                    new_cap *= 2;
                } else {
                    new_cap = PAGE / elem_size;
                }
                // Also ensure that this chunk can fit `additional`.
                new_cap = max(additional, new_cap);

                let mut chunk = ArenaChunk::<T>::new(new_cap);
                // Set ptr and end to the new chunk
                self.ptr.set(chunk.storage);
                self.end.set(chunk.end());

                // Add chunk to index used_chunks (used_chunks will be incremented in grow())
                let last_index = chunks.len();
                chunks.push(chunk);
                if used_chunks < last_index {
                    chunks.swap(used_chunks, last_index);
                }
            }
        }

        self.used_chunks.set(used_chunks + 1);
    }

    /// Drops the contents of the last chunk. The last chunk is partially empty, unlike all other
    /// chunks.
    fn clear_last_chunk(&self, last_chunk: &mut ArenaChunk<T>) {
        debug_assert_ne!(size_of::<T>(), 0);
        // Determine how much was filled.
        let start = last_chunk.storage.addr_();
        // We obtain the value of the pointer to the first uninitialized element.
        let end = self.ptr.get().addr_();
        // We then calculate the number of elements to be dropped in the last chunk,
        // which is the filled area's length.
        // FIXME: this should *likely* use `offset_from`, but more
        //   investigation is needed (including running tests in miri).
        let diff = (end - start) / size_of::<T>();
        // Pass that to the `destroy` method.
        unsafe {
            last_chunk.destroy(diff);
        }
        // Reset the chunk.
        self.ptr.set(last_chunk.storage);
    }
}

impl<T: Debug> Debug for TypedArena<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "TypedArena")?;
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T> Drop for TypedArena<T> {
    fn drop(&mut self) {
        if size_of::<T>() == 0 {
            // These invariants always hold, we only assert them here
            debug_assert!(self.ptr.get().is_null());
            debug_assert!(self.end.get().is_null());
            debug_assert_eq!(self.chunks.borrow().len(), 0);
            debug_assert_eq!(self.used_chunks.get(), 0);

            // Drop `len` ZSTs.
            // Remember: a dangling pointer is a valid ZST reference, `drop_in_place` will only run
            // the ZSTs drop code (which probably shouldn't rely on the address, since it was
            // allocated into an arena and therefore already in an effectively undefined location,
            // without any adjacent structures)
            for _ in 0..self.len() {
                unsafe { drop_in_place(NonNull::<T>::dangling().as_ptr()); }
            }
        } else {
            // `ArenaChunk` drop ensures that the memory is dropped, but we have to drop the contents
            // here because chunks can't because they don't always know their size
            unsafe {
                // Determine how much was filled.
                let mut chunks_borrow = self.chunks.borrow_mut();
                // Remove unused chunks (we don't need to destroy because we've already dropped or moved
                // their contents)
                for _ in 0..(chunks_borrow.len() - self.used_chunks.get()) {
                    chunks_borrow.pop();
                }
                // Drop elements in the used chunks
                if let Some(mut last_chunk) = chunks_borrow.pop() {
                    // Drop the contents of the last chunk.
                    self.clear_last_chunk(&mut last_chunk);
                    // The last chunk will be dropped. Destroy all other chunks.
                    for chunk in chunks_borrow.iter_mut() {
                        chunk.destroy(chunk.entries);
                    }
                }
            }
        }
    }
}

impl<'a, T> IntoIterator for &'a TypedArena<T> {
    type Item = &'a T;
    type IntoIter = ArenaIter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

unsafe impl<T: Send> Send for TypedArena<T> {}

impl DroplessArena {
    /// Create a new, empty arena
    #[inline]
    fn new() -> DroplessArena {
        DroplessArena {
            start: Cell::new(null_mut()),
            end: Cell::new(null_mut()),
            chunks: Default::default(),
        }
    }

    /// Allocates a raw region of data
    #[inline]
    pub fn alloc_raw(&self, layout: Layout) -> *mut u8 {
        assert_ne!(layout.size(), 0);
        loop {
            if let Some(a) = self.alloc_raw_without_grow(layout) {
                break a;
            }
            // No free space left. Allocate a new chunk to satisfy the request.
            // On failure the grow will panic or abort.
            self.grow(layout.size());
        }
    }

    /// Allocates an object which doesn't need to be dropped.
    ///
    /// *Panics* if given a type with drop code. This method's signature looks like it can allocate
    /// any object, but it asserts ![needs_drop] at runtime.
    #[inline]
    pub fn alloc<T>(&self, object: T) -> &mut T {
        assert!(!needs_drop::<T>());

        let mem = self.alloc_raw(Layout::for_value::<T>(&object)) as *mut T;

        unsafe {
            // Write into uninitialized memory.
            write(mem, object);
            &mut *mem
        }
    }

    /// Allocates an iterator of objects which don't need to be dropped.
    ///
    /// *Panics* if you try to allocate ZSTs. Additionally, like with [Self::alloc], this *panics*
    /// if you allocate an object with drop code.
    #[inline]
    pub fn alloc_from_iter<T, I: IntoIterator<Item = T>>(&self, iter: I) -> &mut [T] {
        let iter = iter.into_iter();
        assert_ne!(size_of::<T>(), 0);
        assert!(!needs_drop::<T>());

        let size_hint = iter.size_hint();

        match size_hint {
            (min, Some(max)) if min == max => {
                // We know the exact number of elements the iterator will produce here
                let len = min;

                if len == 0 {
                    return &mut [];
                }

                let mem = self.alloc_raw(Layout::array::<T>(len).unwrap()) as *mut T;
                unsafe { self.write_from_iter(iter, len, mem) }
            }
            (_, _) => {
                cold_path(move || -> &mut [T] {
                    let mut vec: SmallVec<[_; 8]> = iter.collect();
                    if vec.is_empty() {
                        return &mut [];
                    }
                    // Move the content to the arena by copying it and then forgetting
                    // the content of the SmallVec
                    unsafe {
                        let len = vec.len();
                        let start_ptr =
                            self.alloc_raw(Layout::for_value::<[T]>(vec.as_slice())) as *mut T;
                        vec.as_ptr().copy_to_nonoverlapping(start_ptr, len);
                        vec.set_len(0);
                        from_raw_parts_mut(start_ptr, len)
                    }
                })
            }
        }
    }

    /// Allocates a slice of objects that are copied into the `DroplessArena`, returning a mutable
    /// reference to it.
    ///
    /// This will *panic* if passed a zero-sized type or empty slice. Like [Self::alloc], it can't
    /// be given a type with drop code, but the `T: Copy` trait checks this at compile time.
    #[inline]
    pub fn alloc_slice<T: Copy>(&self, slice: &[T]) -> &mut [T] {
        assert_ne!(size_of::<T>(), 0);
        assert!(!slice.is_empty());

        let mem = self.alloc_raw(Layout::for_value::<[T]>(slice)) as *mut T;

        unsafe {
            mem.copy_from_nonoverlapping(slice.as_ptr(), slice.len());
            from_raw_parts_mut(mem, slice.len())
        }
    }

    #[inline]
    unsafe fn write_from_iter<T, I: Iterator<Item = T>>(
        &self,
        mut iter: I,
        len: usize,
        mem: *mut T,
    ) -> &mut [T] {
        let mut i = 0;
        // Use a manual loop since LLVM manages to optimize it better for
        // slice iterators
        loop {
            let value = iter.next();
            if i >= len || value.is_none() {
                // We only return as many items as the iterator gave us, even
                // though it was supposed to give us `len`
                return from_raw_parts_mut(mem, i);
            }
            write(mem.add(i), value.unwrap());
            i += 1;
        }
    }

    /// Allocates a byte slice with specified layout from the current memory
    /// chunk. Returns `None` if there is no free space left to satisfy the
    /// request.
    #[inline]
    fn alloc_raw_without_grow(&self, layout: Layout) -> Option<*mut u8> {
        let start = self.start.get().addr_();
        let old_end = self.end.get();
        let end = old_end.addr_();

        let align = layout.align();
        let bytes = layout.size();

        let new_end = end.checked_sub(bytes)? & !(align - 1);
        if start <= new_end {
            let new_end = old_end.with_addr_(new_end);
            self.end.set(new_end);
            Some(new_end)
        } else {
            None
        }
    }

    #[inline(never)]
    #[cold]
    fn grow(&self, additional: usize) {
        unsafe {
            let mut chunks = self.chunks.borrow_mut();
            let mut new_cap;
            if let Some(last_chunk) = chunks.last_mut() {
                // There is no need to update `last_chunk.entries` because that
                // field isn't used by `DroplessArena`.

                // If the previous chunk's capacity is less than HUGE_PAGE
                // bytes, then this chunk will be least double the previous
                // chunk's size.
                new_cap = last_chunk.capacity.min(HUGE_PAGE / 2);
                new_cap *= 2;
            } else {
                new_cap = PAGE;
            }
            // Also ensure that this chunk can fit `additional`.
            new_cap = max(additional, new_cap);

            let mut chunk = ArenaChunk::new(new_cap);
            self.start.set(chunk.storage);
            self.end.set(chunk.end());
            chunks.push(chunk);
        }
    }
}

// We can't write a good `Debug` impl for `DroplessArena` because its chunks are full of
// uninitialized memory (between alignment and their entries aren't accurate). We could only report
// the capacity, but at that point it's not worth it. `TypedArena` has a good `Debug` impl because
// we actually support iterating its memory.

impl Default for DroplessArena {
    /// Create a new, empty arena
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for DroplessArena {}

impl<T> ArenaChunk<T> {
    #[inline]
    unsafe fn new(capacity: usize) -> ArenaChunk<T> {
        debug_assert_ne!(size_of::<T>(), 0);
        debug_assert_ne!(capacity, 0);
        // Vec doesn't allocate ZSTs but we want 1 byte per ZST, so if T is a ZST we allocate a u8
        // Vec instead of a `T` Vec.
        let mut vec = Vec::with_capacity(capacity);
        let storage = vec.as_mut_ptr();
        forget(vec);
        ArenaChunk { storage, entries: 0, capacity }
    }

    /// Destroys this arena chunk.
    #[inline]
    unsafe fn destroy(&mut self, len: usize) {
        // The branch on needs_drop() is an -O1 performance optimization.
        // Without the branch, dropping TypedArena<u8> takes linear time.
        if needs_drop::<T>() {
            // Here we run drop code
            drop_in_place(&mut *slice_from_raw_parts_mut(self.storage, len));
        }
        // And when the `ArenaChunk` is dropped, we'll free the memory
    }

    /// Returns an iterator of this chunk's elements which effectively destroys this chunk.
    ///
    /// Even though this takes a `&mut self`, you must not access the chunk's data after calling
    /// this.
    #[inline]
    unsafe fn destroy_and_return(&mut self, len: usize) -> Vec<T> {
        // Vec doesn't allocate ZSTs but we want 1 byte per ZST, so if T is a ZST we allocate a u8
        // Vec instead of a `T` Vec. But...it still works, and even drops?
        // ???: this may be UB or rely on a part of `Vec`'s implementation which could change
        let vec = Vec::from_raw_parts(self.storage, len, self.capacity);
        // Set `storage` to null so that we don't try to free the memory on `Drop`
        self.storage = null_mut();
        vec
    }

    // Returns a pointer to the end of the allocated space.
    #[inline]
    fn end(&mut self) -> *mut T {
        unsafe { self.storage.add(self.capacity) }
    }
}

impl<T> Drop for ArenaChunk<T> {
    fn drop(&mut self) {
        // If `storage` is null, we don't want to drop. Otherwise we've already run the drop code,
        // but need to free the memory.
        if !self.storage.is_null() {
            // This will cause the memory to be freed, but no drop code since u8 has none
            unsafe {
                Box::<[u8]>::from_raw(
                    slice_from_raw_parts_mut(self.storage.cast::<u8>(), self.capacity)
                );
            }
        }
    }
}

// Similarly to [DroplessArena], there's no `Debug` for [ArenaChunk]

impl<'a, T, const IS_REF: bool> ArenaGenIter<'a, T, IS_REF> {
    /// Create a new iterator for the arena
    #[inline]
    fn new(arena: &'a TypedArena<T>) -> Self {
        let chunks = arena.chunks.borrow();
        let chunk = chunks.first();
        Self {
            arena,
            chunk_index: 0,
            chunk_data: chunk.map_or(NonNull::dangling(), |c| NonNull::new(c.storage).unwrap()),
            chunk_remaining_entries: chunk.map_or(0, |c| c.entries),
            element_index: 0,
        }
    }

    /// Gets a the next element as a pointer
    pub fn next_ptr(&mut self) -> Option<NonNull<T>> {
        if !self.has_next() {
            return None
        }

        let element = self.chunk_data;
        self.element_index += 1;

        // If this is a ZST we only need to count the # of items to iterate, and `chunk_data` is
        // already a dangling pointer fron `Self::new` since there are no chunks.
        if size_of::<T>() != 0 {
            // If chunk_remaining_entries is 0, we actually still have entries but are on the last
            // chunk. We'll run out when `has_next` returns false.
            if self.chunk_remaining_entries == 1 {
                // We've exhausted the current chunk, so move to the next one
                self.chunk_index += 1;
                let chunks = self.arena.chunks.borrow();
                let chunk = chunks.get(self.chunk_index)
                    .expect("ArenaIter::next invariant error: arena has more elements but no more chunks");
                self.chunk_data = NonNull::new(chunk.storage).unwrap();
                self.chunk_remaining_entries = chunk.entries;
            } else {
                // SAFETY: We're still in the chunk, so we have a valid pointer and add is valid
                self.chunk_data = unsafe { NonNull::new_unchecked(self.chunk_data.as_ptr().add(1)) };
                self.chunk_remaining_entries = self.chunk_remaining_entries.saturating_sub(1);
            }
        }
        Some(element)
    }

    /// Gets the next element as a reference
    #[inline]
    pub fn next_ref(&mut self) -> Option<&'a T> {
        // SAFETY: The value is initialized, because the chunk has more entries and (important for
        //   the last chunk) the arena has more elements
        self.next_ptr().map(|e| unsafe { e.as_ref() })
    }


    /// Get the number of remaining elements, assuming there are no new ones
    #[inline]
    pub fn remaining(&self) -> usize {
        self.arena.len() - self.element_index
    }

    /// Whether we have a next element
    #[inline]
    pub fn has_next(&self) -> bool {
        self.remaining() > 0
    }
}

impl<'a, T> PartialEq for ArenaIter<'a, T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.arena, other.arena) && self.element_index == other.element_index
    }
}

impl<'a, T> Iterator for ArenaIter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.next_ref()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // We will return at least len more elements, but we can't return an upper bound in case
        // some get added
        (self.remaining(), None)
    }
}

impl<'a, T> Iterator for ArenaPtrIter<'a, T> {
    type Item = NonNull<T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.next_ptr()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // We will return at least len more elements, but we can't return an upper bound in case
        // some get added
        (self.remaining(), None)
    }
}

impl<T, const N: usize> IterWithFastAlloc<T> for std::array::IntoIter<T, N> {
    #[inline]
    fn alloc_into(self, arena: &TypedArena<T>) -> &[T] {
        let len = self.len();
        if len == 0 {
            return &[];
        }
        // Move the content to the arena by copying and then forgetting it.
        unsafe {
            let start_ptr = arena.alloc_raw_slice(len);
            self.as_slice().as_ptr().copy_to_nonoverlapping(start_ptr, len);
            forget(self);
            from_raw_parts(start_ptr, len)
        }
    }
}

impl<T> IterWithFastAlloc<T> for Vec<T> {
    #[inline]
    fn alloc_into(mut self, arena: &TypedArena<T>) -> &[T] {
        let len = self.len();
        if len == 0 {
            return &[];
        }
        // Move the content to the arena by copying and then forgetting it.
        unsafe {
            let start_ptr = arena.alloc_raw_slice(len);
            self.as_ptr().copy_to_nonoverlapping(start_ptr, len);
            self.set_len(0);
            from_raw_parts(start_ptr, len)
        }
    }
}

impl<A: smallvec::Array> IterWithFastAlloc<A::Item> for SmallVec<A> {
    #[inline]
    fn alloc_into(mut self, arena: &TypedArena<A::Item>) -> &[A::Item] {
        let len = self.len();
        if len == 0 {
            return &[];
        }
        // Move the content to the arena by copying and then forgetting it.
        unsafe {
            let start_ptr = arena.alloc_raw_slice(len);
            self.as_ptr().copy_to_nonoverlapping(start_ptr, len);
            self.set_len(0);
            from_raw_parts(start_ptr, len)
        }
    }
}

#[inline(never)]
#[cold]
fn cold_path<F: FnOnce() -> R, R>(f: F) -> R {
    f()
}

// region stable implementations of unstable functions
pub trait PtrUnstables<T: ?Sized> {
    #[must_use]
    fn wrapping_byte_offset_(self, count: isize) -> Self;
    #[must_use]
    fn addr_(self) -> usize;
    #[must_use]
    fn with_addr_(self, addr: usize) -> Self;
}

//noinspection DuplicatedCode
impl<T> PtrUnstables<T> for *const T {
    #[inline(always)]
    fn wrapping_byte_offset_(self, count: isize) -> Self {
        // Right now we can get away with using regular wrapping offset and requiring alignment,
        // because we never use this with an unaligned count
        if count % align_of::<T>() as isize == 0 {
            self.wrapping_offset(count / align_of::<T>() as isize)
        } else {
            panic!("wrapping_byte_offset_ called with unaligned count")
        }
    }

    #[inline(always)]
    fn addr_(self) -> usize {
        // XXXXX(strict_provenance_magic): I am magic and should be a compiler intrinsic.
        // SAFETY: Pointer-to-integer transmutes are valid (if you are okay with losing the
        // provenance).
        unsafe { transmute(self.cast::<()>()) }
    }

    #[inline]
    fn with_addr_(self, addr: usize) -> Self {
        // XXXXX(strict_provenance_magic): I am magic and should be a compiler intrinsic.
        //
        // In the mean-time, this operation is defined to be "as if" it was
        // a wrapping_offset, so we can emulate it as such. This should properly
        // restore pointer provenance even under today's compiler.
        let self_addr = self.addr_() as isize;
        let dest_addr = addr as isize;
        let offset = dest_addr.wrapping_sub(self_addr);

        // This is the canonical desugarring of this operation
        self.wrapping_byte_offset_(offset)
    }
}

//noinspection DuplicatedCode
impl<T> PtrUnstables<T> for *mut T {
    #[inline(always)]
    fn wrapping_byte_offset_(self, count: isize) -> Self {
        // Right now we can get away with using regular wrapping offset and requiring alignment,
        // because we never use this with an unaligned count
        if count % align_of::<T>() as isize == 0 {
            self.wrapping_offset(count / align_of::<T>() as isize)
        } else {
            panic!("wrapping_byte_offset_ called with unaligned count")
        }
    }

    #[inline(always)]
    fn addr_(self) -> usize {
        // XXXXX(strict_provenance_magic): I am magic and should be a compiler intrinsic.
        // SAFETY: Pointer-to-integer transmutes are valid (if you are okay with losing the
        // provenance).
        unsafe { transmute(self.cast::<()>()) }
    }

    #[inline]
    fn with_addr_(self, addr: usize) -> Self {
        // XXXXX(strict_provenance_magic): I am magic and should be a compiler intrinsic.
        //
        // In the mean-time, this operation is defined to be "as if" it was
        // a wrapping_offset, so we can emulate it as such. This should properly
        // restore pointer provenance even under today's compiler.
        let self_addr = self.addr_() as isize;
        let dest_addr = addr as isize;
        let offset = dest_addr.wrapping_sub(self_addr);

        // This is the canonical desugarring of this operation
        self.wrapping_byte_offset_(offset)
    }
}
