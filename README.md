# Sanera

`sanera` is a simple arena allocator using rust's [Allocator](https://doc.rust-lang.org/std/alloc/trait.Allocator.html) API



# MSRV

`sanera` requires at least version `1.100.0` of rust


# Example


```rust
use sanera::Arena;

fn main() {
    {
        //We create our arena (lazily)
        let mut arena = Arena::new();

        //The arena allocates the box
        let val1 = Box::new_in(2, &arena);
        let val2 = Box::new_in(4, &arena);

        //We then drop the values, but they are not yet strictly deallocated.
        //Rread the docs for `Arena` for more information.
        std::mem::drop(val2);
        //The value is never Dropped,  but it will be deallocated when `arena` is dropped.
        // `raw_val1` is bound to not outlive the lifetime of arena.
        let raw_val1 = Box::leak(val1);
        
        //The arena is reset and "deallocates" the boxes here.
        // also deallocating all but the largest chunk of the arena.
        arena.reset();
        {
            let val3 = Box::new_in(10, &arena);
        }
        //Deallocated as it is the only element in the arena
        // We could still call `arena.reset()` of course.
    }
    //Arena is now fully cleaned up. (It deallocates everything on Drop)
    // This can be allowed as all allocations need to borrow an `&Arena`.
    // Drop and `reset` only occur and are only available when no allocation is made.
}
```
