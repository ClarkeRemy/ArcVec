#![no_std]
#![allow(unsafe_op_in_unsafe_fn, unused_unsafe)]

use core::marker::PhantomData;
use core::mem::{ManuallyDrop, MaybeUninit};
// use core::mem::{ManuallyDrop, MaybeUninit};
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
  /// Returns a __read-only slice__
  pub fn as_slice(&self) -> &[T] {
    let ptr = self.0.as_ptr();
    let data_ptr: *const T = unsafe {
      ptr
        .cast::<u8>()
        .add(core::mem::size_of::<ArcVecAlloc<T>>())
        .cast::<T>()
    };
    unsafe { core::slice::from_raw_parts(data_ptr, *addr_of!((*ptr).len)) }
  }
  /// Uses `mutate` to create an owned value of MutArcVec
  pub fn own(mut self) -> MutArcVec<T>
  where
    T: Clone, {
    self.mutate();
    // Safety :. * `self` has been mutated to no longer share data.
    //           * `transmute` is safe because of `#[repr(transparent)]`
    unsafe { core::mem::transmute(self) }
  }
  /// Try to make `MutArcVec<T>`
  /// On the error case, recover the original value
  ///
  /// Most suitable when `T` isn't `Clone`
  pub fn try_own(self) -> Result<MutArcVec<T>, Self> {
    if self.is_unique() {
      return Ok(MutArcVec(self));
    }
    Err(self)
  }
  /// Try to make `&mut MutArcVec<T>`
  /// On failure : `None`
  ///
  /// Most suitable when `T` isn't `Clone`
  pub fn try_mut(&mut self) -> Option<&mut MutArcVec<T>> {
    if self.is_unique() {
      // Safety :. `transmute` is safe because of `#[repr(transparent)]`
      return unsafe { Some(core::mem::transmute(self)) };
    }
    None
  }
  /// Any time a mutable method or function is implemented with unsafe code, this must be called for safety
  pub fn mutate(&mut self) -> &mut MutArcVec<T>
  where
    T: Clone, {
    if self.is_unique() {
      // Safety :. `transmute` is safe because of `#[repr(transparent)]`
      return unsafe { core::mem::transmute(self) };
    }

    let ptr = self.0.as_ptr();

    let mut switch =
      ArcVec::<core::mem::MaybeUninit<T>>::with_capacity(unsafe { *addr_of!((*ptr).cap) });
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
    *self = unsafe { core::mem::transmute::<ArcVec<core::mem::MaybeUninit<T>>, ArcVec<T>>(switch) };

    // safety :: MutArcVec<T> has the same layout
    unsafe { core::mem::transmute(self) }
  }
  unsafe fn unchecked_slice_mut(&mut self) -> &mut [T] {
    let ptr = self.0.as_ptr();
    let data_ptr: *mut T = unsafe {
      ptr
        .cast::<u8>()
        .add(core::mem::size_of::<ArcVecAlloc<T>>())
        .cast::<T>()
    };
    unsafe { core::slice::from_raw_parts_mut(data_ptr, *addr_of!((*ptr).len)) }
  }

  pub fn is_unique(&self) -> bool {
    1 == unsafe { &*core::ptr::addr_of!((*self.0.as_ptr()).count) }.load(atomic::Ordering::Acquire)
  }
  pub fn is_empty(&self) -> bool {
    if self.len() == 0 {
      true
    } else {
      false
    }
  }
  pub fn len(&self) -> usize {
    unsafe { *addr_of![(*self.0.as_ptr()).len] }
  }
  pub fn cap(&self) -> usize {
    unsafe { *addr_of![(*self.0.as_ptr()).cap] }
  }
  pub fn len_cap(&self) -> [usize; 2] {
    unsafe {
      [
        *addr_of![(*self.0.as_ptr()).len],
        *addr_of![(*self.0.as_ptr()).cap],
      ]
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
        ptr
          .cast::<u8>()
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

mod arc_vec_index;

// Acts as a mutable reference
// it's creation should ensure that it is unique
#[repr(transparent)]
pub struct MutArcVec<T>(ArcVec<T>);
impl<T> MutArcVec<T> {
  pub fn with_capacity(cap: usize) -> Self {
    MutArcVec(ArcVec::with_capacity(cap))
  }
  pub fn as_mut_slice(&mut self) -> &mut [T] {
    unsafe { self.0.unchecked_slice_mut() }
  }
  pub fn as_slice(&self) -> &[T] {
    self.0.as_slice()
  }
  pub fn len(&self) -> usize {
    self.0.len()
  }
  pub fn cap(&self) -> usize {
    self.0.cap()
  }
  pub fn len_cap(&self) -> [usize; 2] {
    self.0.len_cap()
  }
  /// reserves at least the ammount specified
  pub fn reserve(&mut self, reserve: usize) -> &mut Self {
    let [len, cap] = self.len_cap();
    if cap - len < reserve {
      let mut switch = ArcVec::<T>::with_capacity(len + reserve);
      let switch_ptr = switch.0.as_ptr();
      unsafe { *addr_of_mut!((*switch_ptr).len) = len };
      for (read, write) in self
        .as_mut_slice()
        .iter_mut()
        .zip(unsafe { switch.unchecked_slice_mut().iter_mut() })
      {
        core::mem::swap(read, write)
      }
      core::mem::swap(&mut self.0, &mut switch);
      let _ = core::mem::ManuallyDrop::new(switch);
    }
    self
  }

  pub fn push(&mut self, mut val: T) -> &mut Self {
    let [len, cap] = self.len_cap();
    if len == cap {
      self.reserve(cap);
    }
    // now we can safely push
    {
      let ptr = self.0 .0.as_ptr();
      unsafe { *addr_of_mut!((*ptr).len) += 1 };
      core::mem::swap(
        &mut self.as_mut_slice()[unsafe { *addr_of_mut!((*ptr).len) }],
        &mut val,
      );
      // Safety :. We know that this value is garbage that __must__ not drop
      let _ = core::mem::ManuallyDrop::new(val);
    }
    self
  }
  // Moves MutArcVec to the end of self
  pub fn append(&mut self, other: Self) -> &mut Self {
    self.append_reserve(other, 0)
  }
  // Moves another MutArcVec to the end of self, reserving excess capacity
  // consider using this if you expect to continue mutating
  pub fn append_reserve(&mut self, mut other: Self, excess: usize) -> &mut Self {
    let [len_1, cap_1] = self.len_cap();
    let [len_2, _cap_2] = other.len_cap();
    if cap_1 < len_1 + len_2 + excess {
      self.reserve(len_2 + excess);
    }
    let ptr_1 = self.0 .0.as_ptr();
    unsafe {
      *addr_of_mut!((*ptr_1).len) += len_2;
    }
    let data_1 = self.as_mut_slice();
    let data_2 = other.as_mut_slice();
    for (to, from) in (len_1..len_1 + len_2).zip(0..len_2) {
      core::mem::swap(&mut data_1[to], &mut data_2[from])
    }
    // Safety :. We know that all values here are garbage that __must not drop__
    let _ = core::mem::ManuallyDrop::new(other);
    self
  }
  pub fn shrink(&mut self) -> &mut Self {
    let [len, cap] = self.len_cap();
    if len == cap {
      return self;
    }
    let mut switch = MutArcVec(ArcVec::with_capacity(len));
    core::mem::swap(self, &mut switch);
    self.append(switch)
  }
  pub fn shrink_to(&mut self, min_cap: usize) -> &mut Self {
    let [len, cap] = self.len_cap();
    if len >= min_cap || cap < min_cap {
      return self;
    }
    let mut switch = MutArcVec(ArcVec::with_capacity(min_cap));
    core::mem::swap(self, &mut switch);
    self.append(switch)
  }
  // takes the MutArcVec, leaving an empty one in it's place
  pub fn take(&mut self) -> Self {
    let mut switch = MutArcVec(ArcVec::with_capacity(0));
    core::mem::swap(self, &mut switch);
    switch
  }
  /// first len elements; maintain capacity
  pub fn truncate(&mut self, len: usize) -> &mut Self {
    let length = self.len();
    if len < length {
      return self;
    }
    let s = self.as_mut_slice();
    for idx in len..length {
      // Safety :. we need to drop the values that will be truncated
      unsafe { core::ptr::drop_in_place(&mut s[idx]) }
    }
    let ptr = self.0 .0.as_ptr();
    // Safety :. The values at the back are now garbage,
    //           so the length must be accesed to finally truncate
    unsafe {
      *addr_of_mut!((*ptr).len) = len;
    }
    self
  }
  /// Swaps index element with last element,
  /// Then pops the back with &mut self
  /// # Panics
  /// Panics if `index` is out of bounds
  pub fn swap_remove(&mut self, index: usize) -> T {
    let ptr = self.0 .0.as_ptr();
    let len = unsafe { *addr_of!((*ptr).len) };
    if len <= index {
      panic!("index out of bounds")
    }
    let s = self.as_mut_slice();
    s.swap(index, len - 1);
    self.pop().unwrap()
  }
  pub fn swap_remove_(&mut self, index: usize) -> (&mut Self, T) {
    let pop = self.swap_remove(index);
    (self, pop)
  }
  /// Insert element at index
  /// and shift all after it to the right.
  /// # Panics
  /// Panics if index > len.
  pub fn insert(&mut self, index: usize, element: T) -> &mut Self {
    let len = self.len();
    assert![index < len];
    let ptr = self.reserve(1).0 .0.as_ptr();
    let data_ptr: *mut T = unsafe {
      ptr
        .cast::<u8>()
        .add(core::mem::size_of::<ArcVecAlloc<T>>())
        .cast::<T>()
    };
    unsafe {
      let p: *mut T = data_ptr.add(index);
      // slide values in the back to the right
      core::ptr::copy(p, p.offset(1), len - index);
      // overwrite without dropping, element gets moved
      core::ptr::write(p, element);
      // set the length
      *addr_of_mut!((*ptr).len) += 1;
    }
    self
  }
  /// Remove at index, shifting all to the right one position left
  pub fn remove(&mut self, index: usize) -> (&mut Self, T) {
    let len = self.len();
    assert![index < len];

    let pop = unsafe { core::ptr::read(&mut self.as_mut_slice()[index]) };
    let ptr = self.0 .0.as_ptr();
    let data_ptr: *mut T = unsafe {
      ptr
        .cast::<u8>()
        .add(core::mem::size_of::<ArcVecAlloc<T>>())
        .cast::<T>()
    };
    // Safety :. we need to overwrite the removed value __without__ dropping it
    unsafe {
      let p: *mut T = data_ptr.add(index);
      // slide values in the back to the left, overwriting the value
      core::ptr::copy(p, p.offset(-1), len - index);
      // set the length, duplicate last value will not be dropped
      *addr_of_mut!((*ptr).len) -= 1;
    }
    (self, pop)
  }

  /// Retains only the elements specified by the predicate.
  /// operates in place
  pub fn retain<F>(&mut self, mut f: F) -> &mut Self
  where
    F: FnMut(&T) -> bool, {
    let len = self.len();
    let mut swap_idx = len; // assumes best case initialy
    let s = self.as_mut_slice();
    for idx in 0..len {
      if !f(&s[idx]) {
        swap_idx = idx;
        break;
      }
    }
    if swap_idx != len {
      for idx in swap_idx + 1..len {
        if !f(&s[idx]) {
          continue;
        }
        s.swap(idx, swap_idx);
        swap_idx += 1;
      }
    }
    for drop_idx in swap_idx..len {
      unsafe { core::ptr::drop_in_place(&mut s[drop_idx]) }
    }
    let ptr = self.0 .0.as_ptr();
    unsafe { *addr_of_mut!((*ptr).len) = swap_idx }
    self
  }
  /// Retains only the elements specified by the predicate.
  /// operates in place
  pub fn retain_mut<F>(&mut self, mut f: F) -> &mut Self
  where
    F: FnMut(&mut T) -> bool, {
    // I'd like to refactor retain and retain_mut, to avoid code duplication.
    let len = self.len();
    let mut swap_idx = len; // assumes best case initialy
    let s = self.as_mut_slice();
    for idx in 0..len {
      if !f(&mut s[idx]) {
        swap_idx = idx;
        break;
      }
    }
    if swap_idx != len {
      for idx in swap_idx + 1..len {
        if !f(&mut s[idx]) {
          continue;
        }
        s.swap(idx, swap_idx);
        swap_idx += 1;
      }
    }
    for drop_idx in swap_idx..len {
      unsafe { core::ptr::drop_in_place(&mut s[drop_idx]) }
    }
    let ptr = self.0 .0.as_ptr();
    unsafe { *addr_of_mut!((*ptr).len) = swap_idx }
    self
  }
  pub fn pop(&mut self) -> Option<T> {
    let len = self.len();
    if len == 0 {
      return None;
    }
    let pop = unsafe { core::ptr::read(&self.as_slice()[len - 1]) };
    let ptr = self.0 .0.as_ptr();
    unsafe { *addr_of_mut!((*ptr).len) -= 1 };
    Some(pop)
  }
  pub fn pop_(&mut self) -> (&mut Self, Option<T>) {
    let pop = self.pop();
    (self, pop)
  }
  pub fn clear(&mut self) -> &mut Self {
    let s = self.as_mut_slice();
    for to_drop in s {
      unsafe { core::ptr::drop_in_place(to_drop) }
    }
    let ptr = self.0 .0.as_ptr();
    unsafe { *addr_of_mut!((*ptr).len) = 0 }
    self
  }
  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
  /// the split off `MutArcVec` first element is `at`
  /// # Panics
  /// Panics at > len
  pub fn split_off(&mut self, at: usize) -> Self {
    let len = self.len();
    assert! {!(at > len)}
    let off_cap = len - at;
    let off = Self::with_capacity(off_cap);
    let ptr_off = off.0 .0.as_ptr();
    let s = self.as_mut_slice();
    let data_ptr: *mut T = unsafe {
      ptr_off
        .cast::<u8>()
        .add(core::mem::size_of::<ArcVecAlloc<T>>())
        .cast::<T>()
    };
    unsafe { core::ptr::copy(&mut s[at], data_ptr, off_cap) }
    unsafe { *addr_of_mut!((*ptr_off).len) = off_cap }
    let ptr = self.0 .0.as_ptr();
    unsafe { *addr_of_mut!((*ptr).len) = at }
    off
  }
  pub fn split_off_(&mut self, at: usize) -> (&mut Self, Self) {
    let split = self.split_off(at);
    (self, split)
  }

  /// Returns the remaining spare capacity of the vector as a slice of MaybeUninit<T>.
  pub fn spare_capacity_mut(&mut self) -> &mut [MaybeUninit<T>] {
    // for now we will do the lazy implementation
    self.split_at_spare_mut().1
  }
  /// similar, but also returns the init slice
  pub fn split_at_spare_mut(&mut self) -> (&mut [T], &mut [MaybeUninit<T>]) {
    let [len, cap] = self.len_cap();
    let ptr = self.0 .0.as_ptr();
    let data_ptr: *mut T = unsafe {
      ptr
        .cast::<u8>()
        .add(core::mem::size_of::<ArcVecAlloc<T>>())
        .cast::<T>()
    };
    let spare_ptr = unsafe {
      data_ptr
        .offset(len as isize)
        .cast::<core::mem::MaybeUninit<T>>()
    };
    unsafe {
      (
        core::slice::from_raw_parts_mut(data_ptr, len),
        core::slice::from_raw_parts_mut(spare_ptr, cap - len),
      )
    }
  }
}
impl<T> MutArcVec<T>
where
  T: core::cmp::PartialEq<T>,
{
  pub fn dedup(&mut self) -> &mut Self {
    let len = self.len();
    if len < 2 {
      return self;
    }

    let ptr = self.0 .0.as_ptr();
    let data_ptr: *mut T = unsafe {
      ptr
        .cast::<u8>()
        .add(core::mem::size_of::<ArcVecAlloc<T>>())
        .cast::<T>()
    };

    let mut final_len = len;
    // Safety :. the pointers will be always be different by at least an
    //           offset of 1 one in either loop,
    //           so they never alias while in actual use.
    let [mut left, mut right]: [*mut T; 2] = [data_ptr; 2];
    // optimistic loop first
    for idx in 1..len {
      left = right;
      right = unsafe { data_ptr.add(idx) };
      if unsafe { &addr_of!(left) == &addr_of!(right) } {
        final_len = idx + 1;
        break;
      }
    }
    // if the optimistic loop breaks, this one will run
    for idx in final_len..len {
      right = unsafe { data_ptr.add(idx) };
      if unsafe { &addr_of!(left) != &addr_of!(right) } {
        unsafe {
          let overwrite_ptr = left.add(1);
          core::ptr::drop_in_place(overwrite_ptr);
          core::ptr::write(overwrite_ptr, core::ptr::read(right));
        }
        left = unsafe { left.add(1) };
        continue;
      }
    }
    unsafe { *addr_of_mut!((*ptr).len) = final_len }
    self
  }
}
impl<T> MutArcVec<T>
where
  T: Clone,
{
  pub fn extend_from_slice(&mut self, other: &[T]) -> &mut Self {
    let o_len = other.len();
    self.reserve(o_len);
    let len = self.len();
    let ptr = self.0 .0.as_ptr();
    let data_ptr = unsafe {
      ptr
        .cast::<u8>()
        .add(core::mem::size_of::<ArcVecAlloc<T>>())
        .cast::<T>()
    };
    for (src, dest) in (0..o_len).zip(len..len + o_len) {
      let _ =
        unsafe { core::mem::ManuallyDrop::new(data_ptr.add(dest).replace(other[src].clone())) };
    }
    self
  }

  pub fn extend_from_within<R>(&mut self, src: R) -> &mut Self
  where
    R: core::ops::RangeBounds<usize>, {
    let len = self.len();

    use core::ops::Bound::{Excluded as Ex, Included as In, Unbounded as Un};
    #[rustfmt::skip]
    let range = match (src.start_bound(), src.end_bound()) {
      (Un   , Un   ) =>  0..len,
      (In(s), Un   ) => *s..len,
      (Un   , Ex(e)) =>  0..*e,
      (In(s), Ex(e)) => *s..*e,
      (Un   , In(e)) =>  0.. e + 1,
      (In(s), In(e)) => *s.. e + 1,
      _ => unreachable!("Ranges that start exclusively are not supported"),
    };
    if !(range.start < len && range.end <= len) {
      panic!("Range does not fit within bounds")
    };
    let range_len = range.len();
    self.reserve(range_len);
    let ptr = self.0 .0.as_ptr();
    let data_ptr = unsafe {
      ptr
        .cast::<u8>()
        .add(core::mem::size_of::<ArcVecAlloc<T>>())
        .cast::<T>()
    };
    for (src, dest) in range.zip(len..len + range_len) {
      let _ = ManuallyDrop::new(unsafe {
        core::ptr::replace(data_ptr.add(dest), (*addr_of![*data_ptr.add(src)]).clone())
      });
    }
    unsafe {
      *addr_of_mut!((*ptr).len) = len + range_len;
    }
    self
  }

  pub fn resize(&mut self, new_len: usize, value: T) -> &mut Self {
    let len = self.len();
    if len >= new_len {
      return self.truncate(new_len);
    }
    let ptr = self.0 .0.as_ptr();
    let spare_cap_ptr = unsafe {
      ptr
        .cast::<u8>()
        .add(core::mem::size_of::<ArcVecAlloc<T>>())
        .cast::<T>()
        .add(len)
    };
    for idx in 0..new_len - len {
      let _ =
        ManuallyDrop::new(unsafe { core::ptr::replace(spare_cap_ptr.add(idx), value.clone()) });
    }
    self
  }
}

// TODO Soon
// resize_with
// replace_with

// TODO Last :
// dedup_by_key
// dedup_by
// drain
// drain_filter
// set_len

// deref?
// first
// first_mut
// split_first
// split_first_mut
// split_last
// split_last_mut
// last
// last_mut
// swap
// reverse

// iter
// iter_mut
// windows
//
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

mod mut_arc_vec_index;

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
