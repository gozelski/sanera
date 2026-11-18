use sanera::Arena;

fn main() {
    {
        //We create our arena (lazily)
        let mut arena = Arena::new();

        {
            //The arena allocates the box
            let val1 = Box::new_in(2, &arena);
            let val2 = Box::new_in(4, &arena);
            let val3 = Box::new_in(8, &arena);
            println!("vals: {} {} {}", val1, val2, val3);
            //The value is never Dropped, but it will be deallocated when `arena` is dropped.
            // `_raw_val1` is bound to not outlive the lifetime of arena.
            let _raw_val1 = Box::leak(val1);
        }
        //The arena only actually "deallocates" the boxes here. (We technically do deallocate val3 early)
        arena.reset();
        {
            let val4 = Box::new_in(10, &arena);
            println!("val4: {}", val4);
        }
        //Deallocated as it is the only element in the arena
        // We could still call `arena.reset()` of course.
    }
    //Arena is now fully cleaned up
}
