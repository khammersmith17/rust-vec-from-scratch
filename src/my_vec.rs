use std::alloc::{self, Layout};
use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::ptr::{self, NonNull};

struct RawMyVec<T> {
    ptr: NonNull<T>,
    cap: usize,
}

impl<T> RawMyVec<T> {
    pub fn new() -> Self {
        // Vec is lazily allocated, we only create space when we want the space
        // on instatiation, we do not need any space yet
        assert!(mem::size_of::<T>() != 0, "Bad");
        Self {
            ptr: NonNull::dangling(),
            cap: 0,
        }
    }

    fn grow(&mut self) {
        let (new_capacity, new_layout) = if self.cap == 0 {
            (1, Layout::array::<T>(1).unwrap())
        } else {
            // this cannot overflow since 2*self.cap < isize.MAX
            let new_cap = self.cap * 2;

            // layout array checks to see if the size < usize.MAX
            // but we know that the new capcity is < isize.MAX
            let new_layout = Layout::array::<T>(self.cap).unwrap();
            (new_cap, new_layout)
        };

        assert!(
            new_layout.size() <= isize::MAX as usize,
            "Allocation too large"
        );

        let new_ptr = if self.cap == 0 {
            // new allocation
            unsafe { alloc::alloc(new_layout) }
        } else {
            // allocate new space
            // move old space
            let old_layout = Layout::array::<T>(self.cap).unwrap();
            let old_ptr = self.ptr.as_ptr() as *mut u8;
            unsafe { alloc::realloc(old_ptr, old_layout, new_layout.size()) }
        };

        // giving the pointer to the new space
        // if this fails, panic
        self.ptr = match NonNull::new(new_ptr as *mut T) {
            Some(n) => n,
            None => alloc::handle_alloc_error(new_layout),
        };
        self.cap = new_capacity;
    }
}

impl<T> Drop for RawMyVec<T> {
    fn drop(&mut self) {
        // we have no allocated memory is cap == 0
        if self.cap != 0 {
            // popping off until we have no allocated space left
            // once everything is popped off, we can free our capacity
            let layout = Layout::array::<T>(self.cap).unwrap();
            unsafe {
                alloc::dealloc(self.ptr.as_ptr() as *mut u8, layout);
            }
        }
    }
}

struct MyVec<T> {
    buf: RawMyVec<T>,
    len: usize,
}

impl<T> MyVec<T> {
    pub fn new() -> Self {
        // Vec is lazily allocated, we only create space when we want the space
        // on instatiation, we do not need any space yet
        assert!(mem::size_of::<T>() != 0, "Bad");
        Self {
            buf: RawMyVec::new(),
            len: 0,
        }
    }

    pub fn push(&mut self, element: T) {
        if self.len == self.buf.cap {
            unsafe {
                self.buf.grow();
            }
        }

        // write the new thing to self.ptr + self.len which is our offset into this objects space
        unsafe {
            ptr::write(self.buf.ptr.as_ptr().add(self.len), element);
        }

        // increment len
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        // we are empty here
        if self.len == 0 {
            None
        } else {
            // decremnt len before so we can use this as the offset to the base pointer
            self.len -= 1;
            // read in ptr + offset (len)
            unsafe { Some(ptr::read(self.buf.ptr.as_ptr().add(self.len))) }
        }
    }

    pub fn insert(&mut self, index: usize, element: T) {
        assert!(index < self.len, "Out of bounds");
        if self.len == self.buf.cap {
            unsafe {
                self.buf.grow();
            }
        }

        unsafe {
            ptr::copy(
                self.buf.ptr.as_ptr().add(index),
                self.buf.ptr.as_ptr().add(index + 1),
                self.len - index,
            );
            ptr::write(self.buf.ptr.as_ptr().add(index), element);
        }
        self.len += 1;
    }

    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len, "Out of bounds index");
        unsafe {
            self.len -= 1;
            let result = ptr::read(self.buf.ptr.as_ptr().add(index));
            ptr::copy(
                self.buf.ptr.as_ptr().add(index + 1),
                self.buf.ptr.as_ptr().add(index),
                self.len - index,
            );
            result
        }
    }
}

impl<T> Drop for MyVec<T> {
    fn drop(&mut self) {
        while let Some(_) = self.pop() {}
    }
}

impl<T> Deref for MyVec<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.buf.ptr.as_ptr(), self.len) }
    }
}

impl<T> DerefMut for MyVec<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.buf.ptr.as_ptr(), self.len) }
    }
}

struct RawValIter<T> {
    start: *const T,
    end: *const T,
}

impl<T> RawValIter<T> {
    fn new(slice: &[T]) -> Self {
        // unsafe is needed because of the associateed lifetimes
        unsafe {
            Self {
                start: slice.as_ptr(),
                end: if slice.len() == 0 {
                    slice.as_ptr()
                } else {
                    slice.as_ptr().add(slice.len())
                },
            }
        }
    }
}

impl<T> Iterator for RawValIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        // if our 2 pointers are equal -> done iteratring
        if self.start == self.end {
            return None;
        } else {
            unsafe {
                // get the value at start, our pointer to the begining
                // increment the start by an offset of 1
                let result = ptr::read(self.start);
                self.start = self.start.offset(1);
                Some(result)
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = (self.end as usize - self.start as usize) / mem::size_of::<T>();
        (len, Some(len))
    }
}

impl<T> DoubleEndedIterator for RawValIter<T> {
    fn next_back(&mut self) -> Option<T> {
        if self.start == self.end {
            None
        } else {
            unsafe {
                let result = ptr::read(self.end);
                self.end = self.end.offset(-1);
                Some(result)
            }
        }
    }
}

pub struct IntoIter<T> {
    _buf: RawMyVec<T>,
    iter: RawValIter<T>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<T> {
        self.iter.next_back()
    }
}

impl<T> Drop for IntoIter<T> {
    fn drop(&mut self) {
        // drop the remaining elements in the iterator
        // these are the items that were not yielded
        for _ in &mut *self {}
    }
}

impl<T> IntoIterator for MyVec<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> IntoIter<T> {
        let buf = unsafe { ptr::read(&self.buf) };

        // pass in a reference to self
        // this passes a slice
        // we get this since we implement deref
        let iter = RawValIter::new(&self);

        mem::forget(self);

        IntoIter { _buf: buf, iter }
    }
}
