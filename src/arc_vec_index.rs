use super::ArcVec;

impl<T> core::ops::Index<usize> for ArcVec<T> {
  type Output = T;
  fn index(&self, index: usize) -> &T {
    &self.as_slice()[index]
  }
}

pub(crate) fn neg_idx(index: isize, len: usize) -> usize {
  let len = len as isize;
  if index < -len {
    panic!["Index < (- Len)"]
  }
  if index > len - 1 {
    panic!["Index > (Len - 1)"]
  }
  (index % len) as usize
}
/// Extends indexing to allow negative indexing.
/// negative numbers select from the back
impl<T> core::ops::Index<isize> for ArcVec<T> {
  type Output = T;
  fn index(&self, index: isize) -> &T {
    &self[neg_idx(index, self.as_slice().len())]
  }
}
macro_rules! ArcVec_RangeUsize { ($($R:ident)*) => { $(
impl<T> core::ops::Index<core::ops::$R<usize>> for ArcVec<T> {
    type Output = [T];
    fn index(&self,index: core::ops::$R<usize>)->&[T] {&self.as_slice()[index]}
}
)*};}
ArcVec_RangeUsize! {Range RangeFrom RangeTo RangeInclusive RangeToInclusive}

impl<T> core::ops::Index<core::ops::RangeFull> for ArcVec<T> {
  type Output = [T];
  fn index(&self, index: core::ops::RangeFull) -> &[T] {
    &self.as_slice()[index]
  }
}

impl<T> core::ops::Index<core::ops::Range<isize>> for ArcVec<T> {
  type Output = [T];
  fn index(&self, index: core::ops::Range<isize>) -> &[T] {
    let s = self.as_slice();
    let len = s.len();
    let [start, end] = [index.start, index.end].map(|x| neg_idx(x, len));
    &s[start..end]
  }
}
impl<T> core::ops::Index<core::ops::RangeInclusive<isize>> for ArcVec<T> {
  type Output = [T];
  fn index(&self, index: core::ops::RangeInclusive<isize>) -> &[T] {
    let s = self.as_slice();
    let len = s.len();
    let [start, end] = [index.start(), index.end()].map(|x| neg_idx(*x, len));
    &s[start..=end]
  }
}
impl<T> core::ops::Index<core::ops::RangeToInclusive<isize>> for ArcVec<T> {
  type Output = [T];
  fn index(&self, index: core::ops::RangeToInclusive<isize>) -> &[T] {
    let s = self.as_slice();
    &s[..=neg_idx(index.end, s.len())]
  }
}
impl<T> core::ops::Index<core::ops::RangeTo<isize>> for ArcVec<T> {
  type Output = [T];
  fn index(&self, index: core::ops::RangeTo<isize>) -> &[T] {
    let s = self.as_slice();
    &s[..neg_idx(index.end, s.len())]
  }
}
impl<T> core::ops::Index<core::ops::RangeFrom<isize>> for ArcVec<T> {
  type Output = [T];
  fn index(&self, index: core::ops::RangeFrom<isize>) -> &[T] {
    let s = self.as_slice();
    &s[neg_idx(index.start, s.len())..]
  }
}
