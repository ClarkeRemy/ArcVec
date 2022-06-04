use core::sync::atomic::{self,AtomicUsize,};
use core::ptr::NonNull;
use core::marker::PhantomData;
extern crate alloc;
use alloc::alloc::{Layout,alloc,dealloc};

pub struct ArcVec<T>(
 NonNull<ArcVecAlloc<T>>,
 PhantomData<ArcVecAlloc<T>>);
unsafe impl<T: Sync + Send> Send for ArcVec<T> {}
unsafe impl<T: Sync + Send> Sync for ArcVec<T> {}

impl<T> ArcVec<T> {
 fn with_capacity(cap:usize)->ArcVec<T>{
  ArcVec(ArcVecAlloc::alloc(cap),PhantomData)
 }

 fn as_slice(&self)->&[T] {
	 let ArcVecAlloc { count: _, len, cap:_, data } 
   = unsafe{self.0.as_ref()};
  unsafe{core::ptr::slice_from_raw_parts(data as *const T, *len).as_ref()
   .expect("Null ptr error")}
 }
 /// Any time a mutable method or function is implemented with unsafe code, this must be called for safety
 fn mutate(&mut self)->&mut Self where T:Clone {
  use core::ptr::slice_from_raw_parts_mut as get_mut;
  const MSG : &str = "NULL pointer error";
  let ArcVecAlloc { count, len, cap, data }= 
   unsafe{self.0.as_ptr().as_ref().expect(MSG)};
  if count.load(atomic::Ordering::SeqCst) == 1 {return self}
		
  use core::mem::MaybeUninit;
  let mut switch = ArcVec::<T>::with_capacity(*cap);
  let reader = self.as_slice();
  let writer 
   = unsafe{get_mut((&mut switch.0.as_mut().data ).as_mut_ptr() as *mut MaybeUninit<T>, *len).as_mut()}.expect(MSG);
  for (read,write) in reader.into_iter().zip(writer) {write.write(read.clone());}
  count.fetch_sub(1, atomic::Ordering::SeqCst);
  core::mem::swap(self, &mut switch);
  self
 }
 fn as_slice_mut(&mut self)->&mut [T] where T: Clone {
  let s= self.mutate();
	 let ArcVecAlloc { count: _, len, cap:_, data } 
   = unsafe{self.0.as_mut()};
  unsafe{core::ptr::slice_from_raw_parts_mut(data as *mut T, *len).as_mut()
   .expect("Null pointer error")}
 } 
}



impl<T> Clone for ArcVec<T> {
 fn clone(&self)->Self {
  unsafe{self.0.as_ref()}
   .count.fetch_add(1, atomic::Ordering::SeqCst);
  unsafe {core::mem::transmute_copy(self)}
 }
}

impl<T> Drop for ArcVec<T> {
 fn drop(&mut self){
  use core::ptr::{slice_from_raw_parts_mut ,drop_in_place};
  let ptr = self.0.as_ptr();
  let ArcVecAlloc { count, len, cap, data } 
   = unsafe{ptr.as_mut().expect("Null ptr error")};
  // Saftey :: Non-Atomic mutability is only used after finding the reference count is == 1
  if count.fetch_sub(1, atomic::Ordering::SeqCst) == 1 {
   let to_drop
    = slice_from_raw_parts_mut(data.as_mut_ptr(), *len);
   unsafe{ 
    drop_in_place(to_drop); 
    let msg ="Logic error calculating `Layout` when deallocating";
    dealloc(ptr as *mut u8, 
     Layout::new::<ArcVecAlloc<T>>()
      .extend(Layout::array::<T>(*cap).expect(msg)).expect(msg).0)
   }
  }
 }
}

impl<T> core::ops::Deref for ArcVec<T> {
 type Target = [T];
 fn deref(&self) -> &Self::Target { self.as_slice() }
}
impl<T> core::ops::DerefMut for ArcVec<T> where T: Clone{
 fn deref_mut(&mut self) ->&mut <Self as core::ops::Deref>::Target { self.as_slice_mut() }
}

#[repr(C)] 
struct ArcVecAlloc<T> {
 count:AtomicUsize, 
 len:usize, cap:usize, data:[T;0]
}

impl<T> ArcVecAlloc<T> {
 fn alloc(cap:usize)->NonNull<Self> {
  if cap as usize >= isize::MAX as usize {alloc_overflow()};
  let (lay,_offset) = 
   Layout::new::<ArcVecAlloc<T>>().extend(Layout::array::<T>(cap)
    .expect("Layout Error")).expect("Layout Error");
  unsafe{ 
   let ptr = alloc(lay.clone());
   
   if ptr.is_null() { alloc::alloc::handle_alloc_error(lay)}
   let ptr = ptr as *mut ArcVecAlloc<T>;
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
