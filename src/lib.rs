use std::sync::Arc;
use core::sync::atomic::AtomicUsize;
use core::ptr::NonNull;
use core::marker::PhantomData;
extern crate alloc;
use alloc::alloc::{Layout,alloc};

pub struct ArcVec<T>(
 NonNull<VecMem<T>>,
 PhantomData<VecMem<T>>);
unsafe impl<T: Sync + Send> Send for ArcVec<T> {}
unsafe impl<T: Sync + Send> Sync for ArcVec<T> {}

#[repr(C)] 
struct VecMem<T> {
 count:AtomicUsize, 
 len:usize, cap:usize, data:[T;0]
}

impl<T> VecMem<T> {
 fn with_capacity_raw(cap:usize)->NonNull<Self> {
  if cap as usize >= isize::MAX as usize {alloc_overflow()};
  let (lay,_offset) = Layout::new::<VecMem<T>>().extend(Layout::array::<T>(cap).expect("Layout Error")).expect("Layout Error");
  unsafe{ 
   let ptr = alloc(lay.clone());
   
   if ptr.is_null() { alloc::alloc::handle_alloc_error(lay)}
   let ptr = ptr as *mut VecMem<T>;
   use core::ptr::write as w;
   w(&mut (*ptr).count, AtomicUsize::new(1));
   w(&mut (*ptr).len, 0);
   w(&mut (*ptr).cap, cap);
   NonNull::new_unchecked(ptr)
  }
 }
}


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
