use core::{alloc::Allocator, sync::atomic::AtomicPtr};

use std::alloc::Global;

///An arena allocator to be used with all types implementing the allocator API.
/// The allocator pre-allocates a large chunk of memory at first, growing by page values.
/// The allocator deallocates only when `Arena` is dropped or the allocator is `clear`-ed
///
/// Safety: `Arena` can only be dropped or cleared when nothing is allocated on the Arena.
pub struct Arena<
    #[cfg(not(feature = "std"))] A: Allocator,
    #[cfg(feature = "std")] A: Allocator = Global,
> {
    parent: A,
    end_ptr: AtomicPtr<u8>,
}

impl Arena<Global> {
    pub fn new() -> Self {
        Self::new_in(Global)
    }
}

impl<A: Allocator> Arena<A> {
    pub fn new_in(parent: A) -> Self {
        Arena {
            parent,
            end_ptr: todo!("Allocate Arena"),
        }
    }
}

unsafe impl<A: Allocator> Allocator for Arena<A> {
    fn allocate(
        &self,
        layout: std::alloc::Layout,
    ) -> Result<std::ptr::NonNull<[u8]>, std::alloc::AllocError> {
        self.parent.allocate(layout)
    }

    ///Deallocation is lazy and only occurs when `Arena` is dropped
    /// However, drop is called the same. When a type using
    unsafe fn deallocate(&self, _: std::ptr::NonNull<u8>, _: std::alloc::Layout) {}

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

        let new_ptr = self.allocate(new_layout)?;

        // SAFETY: because `new_layout.size()` must be greater than or equal to
        // `old_layout.size()`, both the old and new memory allocation are valid for reads and
        // writes for `old_layout.size()` bytes. Also, because the old allocation wasn't yet
        // deallocated, it cannot overlap `new_ptr`. Thus, the call to `copy_nonoverlapping` is
        // safe. The safety contract for `dealloc` must be upheld by the caller.
        unsafe {
            core::ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_ptr_mut(), old_layout.size());
            self.deallocate(ptr, old_layout);
        }

        Ok(new_ptr)
    }
}

impl<A: Allocator> Drop for Arena<A> {
    fn drop(&mut self) {
        todo!("Drop the arena and free the backed memory")
    }
}
