#![no_std]
#![allow(unsafe_op_in_unsafe_fn, unused_unsafe)]

use core::marker::PhantomData;
use core::ptr::{addr_of, addr_of_mut, NonNull};
use core::sync::atomic::{self, AtomicUsize};
extern crate alloc;
use alloc::alloc::{alloc, dealloc, Layout};

pub struct ArcVec<T>(NonNull<ArcVecAlloc<T>>, PhantomData<ArcVecAlloc<T>>);
unsafe impl<T: Sync + Send> Send for ArcVec<T> {}
unsafe impl<T: Sync + Send> Sync for ArcVec<T> {}

impl<T> ArcVec<T> {
    fn with_capacity(cap: usize) -> ArcVec<T> {
        ArcVec(ArcVecAlloc::alloc(cap), PhantomData)
    }

    fn as_slice(&self) -> &[T] {
        let ptr = self.0.as_ptr();
        let data_ptr: *const T = unsafe {
            ptr.cast::<u8>()
                .add(core::mem::size_of::<ArcVecAlloc<T>>())
                .cast::<T>()
        };
        unsafe { core::slice::from_raw_parts(data_ptr, *addr_of!((*ptr).len)) }
    }
    /// Any time a mutable method or function is implemented with unsafe code, this must be called for safety
    fn mutate(&mut self) -> &mut MutArcVec<T>
    where
        T: Clone,
    {
        let ptr = self.0.as_ptr();

        // safety :: MutArcVec<T> has the same layout
        if self.is_unique() {
            return unsafe { core::mem::transmute(self) };
        }

        use core::mem::MaybeUninit;
        let mut switch = ArcVec::<MaybeUninit<T>>::with_capacity(unsafe { *addr_of!((*ptr).cap) });
        {
            let length = unsafe { *addr_of!((*ptr).len) };
            unsafe { addr_of_mut!((*switch.0.as_ptr()).len).write(length) };

            let reader = self.as_slice();
            let writer = unsafe { switch.unchecked_slice_mut() };
            for (read, write) in reader.into_iter().zip(writer) {
                write.write(read.clone());
            }
        }
        // safety :: this forces the value at the original self to drop
        *self = unsafe { core::mem::transmute::<ArcVec<MaybeUninit<T>>, ArcVec<T>>(switch) };

        // safety :: MutArcVec<T> has the same layout
        unsafe { core::mem::transmute(self) }
    }
    unsafe fn unchecked_slice_mut(&mut self) -> &mut [T] {
        let ptr = self.0.as_ptr();
        let data_ptr: *mut T = unsafe {
            ptr.cast::<u8>()
                .add(core::mem::size_of::<ArcVecAlloc<T>>())
                .cast::<T>()
        };
        unsafe { core::slice::from_raw_parts_mut(data_ptr, *addr_of!((*ptr).len)) }
    }
    // fn as_slice_mut(&mut self) -> &mut [T]
    // where
    //     T: Clone,
    // {
    //     let s = self.mutate();
    //     unsafe{ s.unchecked_slice_mut() }
    // }

    pub fn is_unique(&self) -> bool {
        1 == unsafe { &*core::ptr::addr_of!((*self.0.as_ptr()).count) }
            .load(atomic::Ordering::Acquire)
    }

    pub fn try_as_mut_slice(&mut self) -> Option<&mut [T]> {
        if self.is_unique() {
            Some(unsafe { self.unchecked_slice_mut() })
        } else {
            None
        }
    }
}

impl<T> Clone for ArcVec<T> {
    fn clone(&self) -> Self {
        unsafe { &*core::ptr::addr_of!((*self.0.as_ptr()).count) }
            .fetch_add(1, atomic::Ordering::Relaxed);
        Self(self.0, PhantomData)
    }
}

impl<T> Drop for ArcVec<T> {
    fn drop(&mut self) {
        use core::ptr::{drop_in_place, slice_from_raw_parts_mut};
        let ptr = self.0.as_ptr();
        // this is technically undefined, but the standard library depends on this kind of code.
        let count: &AtomicUsize = unsafe { &*core::ptr::addr_of!((*ptr).count) };

        // Saftey :: Non-Atomic mutability is only used after finding the reference count is == 1
        if count.fetch_sub(1, atomic::Ordering::AcqRel) == 1 {
            let data_ptr: *mut T = unsafe {
                ptr.cast::<u8>()
                    .add(core::mem::size_of::<ArcVecAlloc<T>>())
                    .cast::<T>()
            };

            unsafe {
                let to_drop = slice_from_raw_parts_mut(data_ptr, *core::ptr::addr_of!((*ptr).len));
                drop_in_place(to_drop);
                let msg = "Logic error calculating `Layout` when deallocating";
                dealloc(
                    ptr as *mut u8,
                    Layout::new::<ArcVecAlloc<T>>()
                        .extend(Layout::array::<T>(*core::ptr::addr_of!((*ptr).cap)).expect(msg))
                        .expect(msg)
                        .0,
                )
            }
        }
    }
}

impl<T> core::ops::Deref for ArcVec<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> core::ops::Deref for MutArcVec<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> core::ops::DerefMut for MutArcVec<T> {
    fn deref_mut(&mut self) -> &mut <Self as core::ops::Deref>::Target {
        self.as_mut_slice()
    }
}

#[repr(transparent)]
struct MutArcVec<T>(ArcVec<T>);
impl<T> MutArcVec<T> {
    fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { self.0.unchecked_slice_mut() }
    }
    fn as_slice(&self) -> &[T] {
        self.0.as_slice()
    }
}

#[repr(C)]
struct ArcVecAlloc<T> {
    count: AtomicUsize,
    len: usize,
    cap: usize,
    /// this field is here for alignment
    data: [T; 0],
}

impl<T> ArcVecAlloc<T> {
    fn alloc(cap: usize) -> NonNull<Self> {
        if cap as usize >= isize::MAX as usize {
            alloc_overflow()
        };
        let (lay, _offset) = Layout::new::<ArcVecAlloc<T>>()
            .extend(Layout::array::<T>(cap).expect("Layout Error"))
            .expect("Layout Error");
        unsafe {
            let ptr = alloc(lay.clone());

            if ptr.is_null() {
                alloc::alloc::handle_alloc_error(lay)
            }
            let ptr = ptr as *mut ArcVecAlloc<T>;
            use core::ptr::addr_of_mut;
            addr_of_mut!((*ptr).count).write(AtomicUsize::new(1));
            addr_of_mut!((*ptr).len).write(0);
            addr_of_mut!((*ptr).cap).write(cap);
            NonNull::new_unchecked(ptr)
        }
    }
}
/*
           let ptr = ptr as *mut ArcVecAlloc<T>;
           core::ptr::addr_of_mut!((*ptr).count).write(AtomicUsize::new(1));
           core::ptr::addr_of_mut!((*ptr).len).write(0);
           core::ptr::addr_of_mut!((*ptr).count).write(cap);
           NonNull::new_unchecked(ptr)
*/

#[inline(never)]
#[cold]
fn alloc_overflow() -> ! {
    panic!("overflow during Layout computation")
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
