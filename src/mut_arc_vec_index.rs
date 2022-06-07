use super::{arc_vec_index::neg_idx, MutArcVec};

macro_rules! MutArcVec_IndexInnerT { ($($IN:ty)* | $($IN_:ty)*) => {
$(
    impl<T> core::ops::Index<$IN> for MutArcVec<T> {
        type Output = T;
        fn index(&self,index: $IN)->&T {&self.0[index]}
    }
)*
$(
    impl<T> core::ops::Index<$IN_> for MutArcVec<T> {
        type Output = [T];
        fn index(&self,index: $IN_)->&[T] {&self.0[index]}
    }
)*

};}
MutArcVec_IndexInnerT! {
usize isize
|
core::ops::Range<usize>             core::ops::Range<isize>
core::ops::RangeInclusive<usize>    core::ops::RangeInclusive<isize>
core::ops::RangeToInclusive<usize>  core::ops::RangeToInclusive<isize>
core::ops::RangeFull
core::ops::RangeFrom<usize>         core::ops::RangeFrom<isize>
core::ops::RangeTo<usize>           core::ops::RangeTo<isize>
}

impl<T> core::ops::IndexMut<usize> for MutArcVec<T> {
  fn index_mut(&mut self, index: usize) -> &mut T {
    &mut self.as_mut_slice()[index]
  }
}

/// Extends indexing to allow negative indexing.
/// negative numbers select from the back
impl<T> core::ops::IndexMut<isize> for MutArcVec<T> {
  fn index_mut(&mut self, index: isize) -> &mut T {
    let s = self.as_mut_slice();
    let len = s.len();
    &mut s[neg_idx(index, len)]
  }
}
macro_rules! MutArcVec_RangeUsize { ($($R:ident)*) => { $(
impl<T> core::ops::IndexMut<core::ops::$R<usize>> for MutArcVec<T> {
    fn index_mut(&mut self,index: core::ops::$R<usize>)->&mut [T] {&mut self.as_mut_slice()[index]}
}
)*};}
MutArcVec_RangeUsize! {Range RangeFrom RangeTo RangeInclusive RangeToInclusive}

impl<T> core::ops::IndexMut<core::ops::RangeFull> for MutArcVec<T> {
  fn index_mut(&mut self, index: core::ops::RangeFull) -> &mut [T] {
    &mut self.as_mut_slice()[index]
  }
}

impl<T> core::ops::IndexMut<core::ops::Range<isize>> for MutArcVec<T> {
  fn index_mut(&mut self, index: core::ops::Range<isize>) -> &mut [T] {
    let s = self.as_mut_slice();
    let len = s.len();
    let [start, end] = [index.start, index.end].map(|x| neg_idx(x, len));
    &mut s[start..end]
  }
}
impl<T> core::ops::IndexMut<core::ops::RangeInclusive<isize>> for MutArcVec<T> {
  fn index_mut(&mut self, index: core::ops::RangeInclusive<isize>) -> &mut [T] {
    let s = self.as_mut_slice();
    let len = s.len();
    let [start, end] = [index.start(), index.end()].map(|x| neg_idx(*x, len));
    &mut s[start..=end]
  }
}
impl<T> core::ops::IndexMut<core::ops::RangeToInclusive<isize>> for MutArcVec<T> {
  fn index_mut(&mut self, index: core::ops::RangeToInclusive<isize>) -> &mut [T] {
    let s = self.as_mut_slice();
    let len = s.len();
    &mut s[..=neg_idx(index.end, len)]
  }
}
impl<T> core::ops::IndexMut<core::ops::RangeTo<isize>> for MutArcVec<T> {
  fn index_mut(&mut self, index: core::ops::RangeTo<isize>) -> &mut [T] {
    let s = self.as_mut_slice();
    let len = s.len();
    &mut s[..neg_idx(index.end, len)]
  }
}
impl<T> core::ops::IndexMut<core::ops::RangeFrom<isize>> for MutArcVec<T> {
  fn index_mut(&mut self, index: core::ops::RangeFrom<isize>) -> &mut [T] {
    let s = self.as_mut_slice();
    let len = s.len();
    &mut s[neg_idx(index.start, len)..]
  }
}
