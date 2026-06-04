//! Fixed-capacity ring buffer (circular buffer).
//!
//! O(1) push, O(1) indexed access. When full, the oldest entry is dropped
//! and overwritten (FIFO eviction). No allocations after initial creation.
//!
//! Used as the default data store for [`VirtualTable`](super::VirtualTable),
//! but also usable standalone for any rolling-window scenario (logs, metrics, etc.).
//!
//! # Capacity
//!
//! Hard limit: [`MAX_TABLE_ROWS`] (10,000,000). Requests exceeding this are clamped.
//!
//! # Key Operations
//!
//! | Operation    | Complexity | Notes                              |
//! |-------------|------------|------------------------------------|
//! | `push`      | O(1)       | Drops oldest if at capacity        |
//! | `get`/`get_mut` | O(1)   | Logical index (0 = oldest)         |
//! | `remove`    | O(n)       | Linearizes first, then shifts      |
//! | `sort_by`   | O(n log n) | In-place after linearization       |
//! | `clear`     | O(n)       | Drops all elements                 |
//! | `iter`      | O(n)       | Oldest to newest                   |

use std::cmp::Ordering;
use std::mem::MaybeUninit;

/// Maximum number of rows a single table can hold.
/// At 10M rows the ring buffer consumes ~80 MB overhead + sizeof(T) per slot.
/// ListClipper renders only visible rows regardless of total count.
pub const MAX_TABLE_ROWS: usize = 10_000_000;

pub struct RingBuffer<T> {
    buf: Box<[MaybeUninit<T>]>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl<T> RingBuffer<T> {
    /// Create a ring buffer with the given capacity (clamped to [`MAX_TABLE_ROWS`]).
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.clamp(1, MAX_TABLE_ROWS);
        let mut v = Vec::with_capacity(capacity);
        // SAFETY: `MaybeUninit<T>` does not require initialization — setting the
        // length on an uninit vec is valid because every element is `MaybeUninit`.
        unsafe { v.set_len(capacity) };
        Self {
            buf: v.into_boxed_slice(),
            capacity,
            head: 0,
            len: 0,
        }
    }

    /// Push an item. O(1), no allocation after initial creation.
    pub fn push(&mut self, item: T) {
        if self.len == self.capacity {
            unsafe { self.buf[self.head].assume_init_drop() };
        }
        self.buf[self.head] = MaybeUninit::new(item);
        self.head = (self.head + 1) % self.capacity;
        if self.len < self.capacity {
            self.len += 1;
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Logical start index (oldest item) in the physical buffer.
    #[inline]
    fn start(&self) -> usize {
        if self.len < self.capacity {
            0
        } else {
            self.head
        }
    }

    /// Map logical index → physical index.
    #[inline]
    fn physical(&self, logical: usize) -> usize {
        (self.start() + logical) % self.capacity
    }

    /// Get item by logical index (0 = oldest visible item).
    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        Some(unsafe { self.buf[self.physical(index)].assume_init_ref() })
    }

    /// Get mutable reference by logical index.
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }
        let phys = self.physical(index);
        Some(unsafe { self.buf[phys].assume_init_mut() })
    }

    pub fn clear(&mut self) {
        let start = self.start();
        for i in 0..self.len {
            let actual = (start + i) % self.capacity;
            unsafe { self.buf[actual].assume_init_drop() };
        }
        self.head = 0;
        self.len = 0;
    }

    /// Remove element at logical index. O(n) — shifts elements.
    /// Returns the removed item if index is valid.
    pub fn remove(&mut self, logical_index: usize) -> Option<T> {
        if logical_index >= self.len {
            return None;
        }
        // Linearize so logical index == physical index; the shift is then a
        // single contiguous memmove (`Vec::remove` strategy).
        self.linearize();
        let phys = logical_index;
        // Move the target element out and hand it to the caller.
        let item = unsafe { self.buf[phys].assume_init_read() };
        // Shift the tail `[phys+1 .. len)` down by one to close the gap.
        //
        // `ptr::copy` (memmove) relocates those elements. The vacated slot at
        // `buf[len-1]` is left holding a *bitwise duplicate* of the new last
        // element — it MUST NOT be dropped: its heap resources are now owned by
        // the relocated element, so dropping it would double-free for `T: Drop`.
        // The slot becomes logically dead (`len` shrinks) and is overwritten
        // without a drop by the next `push`.
        let tail = self.len - phys - 1;
        if tail > 0 {
            unsafe {
                let base = self.buf.as_mut_ptr();
                std::ptr::copy(base.add(phys + 1), base.add(phys), tail);
            }
        }
        self.len -= 1;
        self.head = self.len % self.capacity;
        Some(item)
    }

    /// Sort all elements by a comparison function.
    /// Linearizes the ring first, then sorts in place.
    pub fn sort_by(&mut self, mut cmp: impl FnMut(&T, &T) -> Ordering) {
        if self.len <= 1 {
            return;
        }
        // Linearize: rotate so that start == 0
        self.linearize();
        // Now elements are at buf[0..len], sort them
        let slice = &mut self.buf[..self.len];
        slice.sort_by(|a, b| unsafe { cmp(a.assume_init_ref(), b.assume_init_ref()) });
    }

    /// Rotate internal buffer so logical index 0 is at physical index 0.
    fn linearize(&mut self) {
        if self.len < self.capacity || self.head == 0 {
            return; // already linear
        }
        // Rotate the occupied portion in-place using the slice rotate algorithm.
        // SAFETY: all `capacity` slots are initialized when len == capacity.
        self.buf[..self.capacity].rotate_left(self.head);
        self.head = 0;
    }

    /// Iterate over all elements (oldest to newest).
    pub fn iter(&self) -> RingIter<'_, T> {
        RingIter { ring: self, pos: 0 }
    }

    /// Iterate mutably over all elements (oldest to newest).
    pub fn iter_mut(&mut self) -> RingIterMut<'_, T> {
        let len = self.len;
        let start = self.start();
        let capacity = self.capacity;
        RingIterMut {
            ptr: self.buf.as_mut_ptr(),
            start,
            capacity,
            len,
            pos: 0,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> Drop for RingBuffer<T> {
    fn drop(&mut self) {
        self.clear();
    }
}

// ─── Iterators ──────────────────────────────────────────────────────────────

pub struct RingIter<'a, T> {
    ring: &'a RingBuffer<T>,
    pos: usize,
}

impl<'a, T> Iterator for RingIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.ring.len {
            return None;
        }
        let item = self.ring.get(self.pos);
        self.pos += 1;
        item
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.ring.len - self.pos;
        (remaining, Some(remaining))
    }
}

impl<T> ExactSizeIterator for RingIter<'_, T> {}

pub struct RingIterMut<'a, T> {
    ptr: *mut MaybeUninit<T>,
    start: usize,
    capacity: usize,
    len: usize,
    pos: usize,
    _marker: std::marker::PhantomData<&'a mut T>,
}

impl<'a, T> Iterator for RingIterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.len {
            return None;
        }
        let phys = (self.start + self.pos) % self.capacity;
        self.pos += 1;
        // SAFETY: each pos is visited exactly once, phys indices are unique,
        // and ptr is valid for the lifetime 'a.
        Some(unsafe { (*self.ptr.add(phys)).assume_init_mut() })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len - self.pos;
        (remaining, Some(remaining))
    }
}

impl<T> ExactSizeIterator for RingIterMut<'_, T> {}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn push_get_len() {
        let mut rb = RingBuffer::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        rb.push(30);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.get(3), None);
    }

    #[test]
    fn fifo_eviction_when_full() {
        let mut rb = RingBuffer::new(3);
        for v in 1..=5 {
            rb.push(v);
        }
        // Oldest two (1,2) evicted; window is [3,4,5].
        assert_eq!(rb.len(), 3);
        let got: Vec<_> = rb.iter().copied().collect();
        assert_eq!(got, vec![3, 4, 5]);
    }

    #[test]
    fn capacity_clamped_to_at_least_one() {
        let rb: RingBuffer<i32> = RingBuffer::new(0);
        assert_eq!(rb.capacity(), 1);
    }

    #[test]
    fn remove_middle_shifts() {
        let mut rb = RingBuffer::new(5);
        for v in [1, 2, 3, 4] {
            rb.push(v);
        }
        assert_eq!(rb.remove(1), Some(2));
        let got: Vec<_> = rb.iter().copied().collect();
        assert_eq!(got, vec![1, 3, 4]);
    }

    #[test]
    fn remove_first_and_last() {
        let mut rb = RingBuffer::new(5);
        for v in [1, 2, 3] {
            rb.push(v);
        }
        assert_eq!(rb.remove(0), Some(1));
        assert_eq!(rb.iter().copied().collect::<Vec<_>>(), vec![2, 3]);
        assert_eq!(rb.remove(1), Some(3));
        assert_eq!(rb.iter().copied().collect::<Vec<_>>(), vec![2]);
        assert_eq!(rb.remove(5), None);
    }

    #[test]
    fn remove_after_wraparound() {
        // Force a wrap (head != 0) then remove a middle element.
        let mut rb = RingBuffer::new(3);
        for v in 1..=5 {
            rb.push(v); // window [3,4,5], internally wrapped
        }
        assert_eq!(rb.remove(1), Some(4));
        assert_eq!(rb.iter().copied().collect::<Vec<_>>(), vec![3, 5]);
    }

    #[test]
    fn sort_by_orders_elements() {
        let mut rb = RingBuffer::new(5);
        for v in [30, 10, 20, 5] {
            rb.push(v);
        }
        rb.sort_by(|a, b| a.cmp(b));
        assert_eq!(rb.iter().copied().collect::<Vec<_>>(), vec![5, 10, 20, 30]);
    }

    #[test]
    fn iter_mut_modifies_in_order() {
        let mut rb = RingBuffer::new(4);
        for v in [1, 2, 3] {
            rb.push(v);
        }
        for x in rb.iter_mut() {
            *x *= 10;
        }
        assert_eq!(rb.iter().copied().collect::<Vec<_>>(), vec![10, 20, 30]);
    }

    // ── Drop-safety: the critical regression coverage for `remove` ──────────
    //
    // `DropTracker` bumps a shared counter on drop. A correct ring drops every
    // element exactly once. The pre-fix `remove` left a bitwise-duplicate of the
    // shifted-down last element in the vacated tail slot and then dropped it,
    // double-freeing for `T: Drop` — this counts that as an extra drop.

    struct DropTracker {
        _id: u32,
        counter: Rc<std::cell::Cell<usize>>,
    }
    impl Drop for DropTracker {
        fn drop(&mut self) {
            self.counter.set(self.counter.get() + 1);
        }
    }

    fn tracker(id: u32, counter: &Rc<std::cell::Cell<usize>>) -> DropTracker {
        DropTracker {
            _id: id,
            counter: Rc::clone(counter),
        }
    }

    #[test]
    fn remove_drops_each_element_exactly_once() {
        let counter = Rc::new(std::cell::Cell::new(0));
        let mut rb = RingBuffer::new(4);
        for id in 0..4 {
            rb.push(tracker(id, &counter));
        }
        // Remove a non-last element — this exercises the tail-shift path that
        // previously double-freed.
        let removed = rb.remove(1);
        drop(removed); // +1 legit drop (the returned element)
        drop(rb); // drops the remaining 3 elements, once each
        // 4 created → exactly 4 drops. Pre-fix this was 5 (one double-free).
        assert_eq!(
            counter.get(),
            4,
            "each element must be dropped exactly once"
        );
    }

    #[test]
    fn remove_then_use_survivors_no_uaf() {
        // After removing a middle element, the surviving elements must still be
        // valid (no use-after-free of the relocated tail element).
        let mut rb = RingBuffer::new(4);
        for s in ["a", "b", "c", "d"] {
            rb.push(String::from(s));
        }
        assert_eq!(rb.remove(1).as_deref(), Some("b"));
        // Touch every survivor — would read freed memory under the old bug.
        let joined: String = rb.iter().cloned().collect::<Vec<_>>().join(",");
        assert_eq!(joined, "a,c,d");
    }

    #[test]
    fn clear_drops_all_once() {
        let counter = Rc::new(std::cell::Cell::new(0));
        let mut rb = RingBuffer::new(8);
        for id in 0..6 {
            rb.push(tracker(id, &counter));
        }
        rb.clear();
        assert_eq!(counter.get(), 6);
        assert!(rb.is_empty());
    }

    #[test]
    fn eviction_drops_evicted_once() {
        let counter = Rc::new(std::cell::Cell::new(0));
        let mut rb = RingBuffer::new(2);
        rb.push(tracker(0, &counter));
        rb.push(tracker(1, &counter));
        rb.push(tracker(2, &counter)); // evicts id 0 → exactly one drop
        assert_eq!(counter.get(), 1);
        drop(rb); // drops remaining 2
        assert_eq!(counter.get(), 3);
    }
}
