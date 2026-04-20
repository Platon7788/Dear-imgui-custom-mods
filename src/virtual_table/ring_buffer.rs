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
//! Hard limit: [`MAX_TABLE_ROWS`] (1,000,000). Requests exceeding this are clamped.
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
        // Linearize first to make shifting straightforward
        self.linearize();
        let phys = logical_index; // after linearize, logical == physical
        let item = unsafe { self.buf[phys].assume_init_read() };
        // Shift elements left
        for i in phys..self.len - 1 {
            unsafe {
                let next = std::ptr::read(&self.buf[i + 1]);
                std::ptr::write(&mut self.buf[i], next);
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
