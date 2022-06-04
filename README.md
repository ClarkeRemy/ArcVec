#ArcVec

## this library is not ready yet

A library to have atomically reference counted vectors behind thin pointers. to reduce memory footprint.
Assuming you mostly are doing reads, this shoud be low cost.

Any attempt to mutate the ArcVec will first ensure that it has a Atomic count of 1, else it will clone the vector.
Note that this does not include an internal `RWLock` or `Mutex`. Each ArcVec behaves as an owned value.

For best performance, it is assumed that you should be reading heavily, and mutating as many elements as possible. (prefer `extend` over `push`)

This crate is `no_std`