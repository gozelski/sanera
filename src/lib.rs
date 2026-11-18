#![doc = include_str!("../README.md")]

use core::{
    alloc::{AllocError, Allocator, Layout},
    cell::Cell,
    ptr::{NonNull, null_mut},
};

#[cfg(feature = "std")]
use std::alloc::Global;

/// An arena allocator to be used with types implementing the allocator API.
/// The allocator is lazy and only pre-allocates a large chunk of memory once used.
/// `Arena` grows by powers of two in size.
/// When reset `Arena` only keeps the last (and largest) chunk, deallocating the other chunks.
/// The allocator deallocates only when `Arena` is dropped or the allocator is `reset`.
///
/// Safety: `Arena` can only be dropped or reset when nothing is allocated on the Arena.
pub struct Arena<
    #[cfg(not(feature = "std"))] A: Allocator,
    #[cfg(feature = "std")] A: Allocator = Global,
> {
    parent: A,
    ptr: Cell<*mut u8>,
    head: Cell<Option<NonNull<ChunkHeader>>>,
}

impl Arena<Global> {
    // Lazily create an arena in the standard library default `Global` allocator.
    // No backing memory is allocated until requested by an allocation.
    pub fn new() -> Self {
        Self::new_in(Global)
    }
}

impl<A: Allocator> Arena<A> {
    // Lazily create an arena in a given parent allocator.
    // No backing memory is allocated until requested by an allocation.
    pub fn new_in(parent: A) -> Self {
        Arena {
            parent,
            ptr: Cell::new(null_mut()),
            head: Cell::new(None),
        }
    }

    fn try_alloc_layout(&self, chunk: NonNull<ChunkHeader>, layout: Layout) -> Option<NonNull<u8>> {
        unsafe {
            let chunk_start = chunk.as_ptr().cast::<u8>();
            let chunk_end = chunk_start.add(chunk.as_ref().layout.size());

            let cur = self.ptr.get();
            let align_offset = cur.align_offset(layout.align());
            if align_offset == usize::MAX {
                return None;
            }

            let aligned = cur.add(align_offset);
            let end_ptr = aligned.add(layout.size());

            if end_ptr <= chunk_end {
                self.ptr.set(end_ptr);
                NonNull::new(aligned)
            } else {
                None
            }
        }
    }

    #[cold]
    fn extend(&self, layout: Layout) -> Result<NonNull<ChunkHeader>, AllocError> {
        let prev = self.head.get();
        let prev_size = prev.map_or(0, |p| unsafe { p.as_ref().layout.size() });

        //TODO: page size or stop defaulting to "magic" size.
        let min_data = layout.size().max(4096);
        let target_data_size = prev_size.saturating_mul(2).max(min_data);
        let chunk_align = layout.align().max(16);

        let header_layout = Layout::new::<ChunkHeader>();
        let data_layout =
            Layout::from_size_align(target_data_size, chunk_align).map_err(|_| AllocError)?;

        let (full_layout, data_offset) =
            header_layout.extend(data_layout).map_err(|_| AllocError)?;
        let full_layout = full_layout.pad_to_align();

        unsafe {
            let raw = self.parent.allocate(full_layout).map_err(|_| AllocError)?;
            let chunk = raw.cast::<ChunkHeader>();
            chunk.as_ptr().write(ChunkHeader {
                prev,
                layout: full_layout,
            });
            self.head.set(Some(chunk));
            self.ptr.set(raw.as_ptr().cast::<u8>().add(data_offset));
            Ok(chunk)
        }
    }

    pub fn reset(&mut self) {
        let Some(head) = self.head.get() else {
            return;
        };

        unsafe {
            let mut current = head.as_ref().prev;
            while let Some(chunk) = current {
                let header = chunk.as_ptr().read();
                current = header.prev;
                self.deallocate(chunk.cast::<u8>(), header.layout);
            }

            (*head.as_ptr()).prev = None;

            let header_layout = Layout::new::<ChunkHeader>();
            let data_align = head.as_ref().layout.align();
            let (_, data_offset) = header_layout
                .extend(Layout::from_size_align_unchecked(0, data_align))
                .unwrap_unchecked();
            self.ptr.set(head.as_ptr().cast::<u8>().add(data_offset));
        }
    }
}

unsafe impl<A: Allocator> Allocator for Arena<A> {
    fn allocate(
        &self,
        layout: std::alloc::Layout,
    ) -> Result<std::ptr::NonNull<[u8]>, std::alloc::AllocError> {
        if layout.size() == 0 {
            let ptr = NonNull::<u8>::without_provenance(layout.align().try_into().unwrap());
            return Ok(NonNull::slice_from_raw_parts(ptr, 0));
        }

        if let Some(head) = self.head.get() {
            if let Some(ptr) = self.try_alloc_layout(head, layout) {
                return Ok(NonNull::slice_from_raw_parts(ptr, layout.size()));
            }
        }

        let new_head = self.extend(layout)?;
        let ptr = self.try_alloc_layout(new_head, layout).ok_or(AllocError)?;
        return Ok(NonNull::slice_from_raw_parts(ptr, layout.size()));
    }

    ///Deallocation is lazy and only occurs when `Arena` is dropped
    /// However, drop is called the same.
    unsafe fn deallocate(&self, ptr: std::ptr::NonNull<u8>, layout: std::alloc::Layout) {
        if layout.size() == 0 {
            return;
        }

        // LIFO optimization.
        // If we are at the last element, instead of NO-OP we can reset to its base address.
        let end_of_block = unsafe { ptr.as_ptr().add(layout.size()) };
        if end_of_block == self.ptr.get() {
            self.ptr.set(ptr.as_ptr());
        }
    }

    unsafe fn grow(
        &self,
        ptr: std::ptr::NonNull<u8>,
        old_layout: std::alloc::Layout,
        new_layout: std::alloc::Layout,
    ) -> Result<std::ptr::NonNull<[u8]>, std::alloc::AllocError> {
        debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );

        // LIFO optimization.
        let end_of_old_block = unsafe { ptr.as_ptr().add(old_layout.size()) };
        let end_of_new_block = unsafe { ptr.as_ptr().add(new_layout.size()) };
        let chunk_start = self.head.as_ptr().cast::<u8>();
        let chunk_end =
            unsafe { chunk_start.add(self.head.get().ok_or(AllocError)?.as_ref().layout.size()) };
        if self.ptr.get() == end_of_old_block && end_of_new_block <= chunk_end {
            self.ptr.set(ptr.as_ptr());
            if let Some(ptr) = NonNull::new(self.ptr.get()) {
                return Ok(NonNull::slice_from_raw_parts(ptr, new_layout.size()));
            }
        }

        return self.allocate(new_layout);
    }
}

impl<A: Allocator> Drop for Arena<A> {
    fn drop(&mut self) {
        let mut current = self.head.get();
        while let Some(chunk) = current {
            unsafe {
                let header = chunk.as_ptr().read();
                // Safety: We take current by value before the chunk it is stored in is deallocated.
                current = header.prev;
                self.parent.deallocate(chunk.cast::<u8>(), header.layout);
            }
        }
    }
}

/// Internal type. This is not to be used as API.
#[repr(C)]
struct ChunkHeader {
    prev: Option<NonNull<ChunkHeader>>,
    layout: Layout,
}
