///An arena allocator to be used with all types implementing the allocator API.
/// The allocator pre-allocates a large chunk of memory at first, growing by page values.
/// The allocator deallocates only when `Arena` is dropped.
///
/// Warning: It is not recommended to use `Arena` as the global allocator as it will essential act as a memory leak until the program exits.
pub struct Arena {}
