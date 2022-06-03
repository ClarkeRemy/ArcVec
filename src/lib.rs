use std::sync::Arc;
use core::sync::atomic::AtomicUsize;
use core::ptr::NonNull;
use core::marker::PhantomData;
extern crate alloc;
use alloc::alloc::{Layout,alloc,dealloc};

pub struct ArcVec<T>(
 NonNull<VecMem<T>>,
 PhantomData<VecMem<T>>);
unsafe impl<T: Sync + Send> Send for ArcVec<T> {}
unsafe impl<T: Sync + Send> Sync for ArcVec<T> {}

impl<T> ArcVec<T> {
 fn with_capacity(cap:usize)->ArcVec<T>{
  ArcVec(VecMem::alloc(cap),PhantomData)
 }
}

impl<T> Clone for ArcVec<T> {
 fn clone(&self)->Self {
  unsafe{self.0.as_ref()}
   .count.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
  unsafe {core::mem::transmute_copy(self)}
 }
}

impl<T> Drop for ArcVec<T> {
 fn drop(&mut self){
  use core::ptr::{slice_from_raw_parts_mut,drop_in_place};
  let ptr = self.0.as_ptr();
  let VecMem { count, len, cap, data } 
   = unsafe{&mut *ptr};
  if count.fetch_sub(1, core::sync::atomic::Ordering::SeqCst) == 1 {
  let to_drop
   = slice_from_raw_parts_mut(data.as_mut_ptr(), *len);
  unsafe{ 
   drop_in_place(to_drop); 
   let msg ="Logic error calculating `Layout` when deallocating";
   dealloc(ptr as *mut u8, 
    Layout::new::<VecMem<T>>()
     .extend(Layout::array::<T>(*cap).expect(msg)).expect(msg).0)
   }
  }
 }
}

#[repr(C)] 
struct VecMem<T> {
 count:AtomicUsize, 
 len:usize, cap:usize, data:[T;0]
}

impl<T> VecMem<T> {
 fn alloc(cap:usize)->NonNull<Self> {
  if cap as usize >= isize::MAX as usize {alloc_overflow()};
  let (lay,_offset) = 
   Layout::new::<VecMem<T>>().extend(Layout::array::<T>(cap)
    .expect("Layout Error")).expect("Layout Error");
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
