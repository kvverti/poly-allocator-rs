#![feature(allocator_api)]
#![forbid(unsafe_op_in_unsafe_fn)]

use core::marker::PhantomData;
use core::ptr::{self, NonNull};
use std::alloc::{self, AllocError, Allocator, Layout};

pub use crate::vtable::RawPolyAllocVTable;

pub mod vtable;

/// A polymorphic allocator.
#[derive(Debug)]
pub struct LocalPolyAllocator<'a> {
    /// A pointer to the allocator (erased). In the case of owned allocations, the memory behind
    /// the pointer is allocated by the allocator.
    data: NonNull<()>,
    /// A reference to the vtable.
    vtable: &'static RawPolyAllocVTable,
    _ph: PhantomData<dyn Allocator + 'a>,
}

// Drop

impl Drop for LocalPolyAllocator<'_> {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.delete)(self.data);
        }
    }
}

// Clone

impl Clone for LocalPolyAllocator<'_> {
    fn clone(&self) -> Self {
        // SAFETY: We have a proper new data pointer from the clone method in the vtable
        unsafe { Self::from_raw_parts((self.vtable.clone)(self.data.as_ptr()), self.vtable) }
    }
}

impl<'a> LocalPolyAllocator<'a> {
    /// SAFETY: `vtable` must be a vtable compatible with the allocator type underlying `data`.
    ///         Additionally, the underlying type must live for `'a`.
    pub unsafe fn from_raw_parts(data: NonNull<()>, vtable: &'static RawPolyAllocVTable) -> Self {
        Self {
            data,
            vtable,
            _ph: PhantomData,
        }
    }

    pub fn try_owned<A>(allocator: A) -> Result<Self, AllocError>
    where
        A: Allocator + Clone + 'a,
    {
        let layout = Layout::new::<A>();
        let storage = allocator.allocate(layout)?.cast::<A>();
        // SAFETY: `storage` points to allocated memory for type `A`, which the generic
        //         bounds guarantee lives for `'a`.
        unsafe {
            ptr::write(storage.as_ptr(), allocator);
            Ok(Self::from_raw_parts(
                storage.cast::<()>(),
                RawPolyAllocVTable::owned::<A>(),
            ))
        }
    }

    pub fn owned<A>(allocator: A) -> Self
    where
        A: Allocator + Clone + 'a,
    {
        match Self::try_owned(allocator) {
            Ok(ret) => ret,
            Err(_) => alloc::handle_alloc_error(Layout::new::<A>()),
        }
    }

    pub fn borrowed<A>(allocator: &'a A) -> Self
    where
        A: Allocator + 'a,
    {
        // SAFETY: The vtable is compatible with `A` in a borrowed context, and we borrow
        //         the allocator for `'a`.
        unsafe {
            Self::from_raw_parts(
                NonNull::from(allocator).cast::<()>(),
                RawPolyAllocVTable::borrowed::<A>(),
            )
        }
    }

    pub fn into_raw_parts(self) -> (NonNull<()>, &'static RawPolyAllocVTable) {
        (self.data, self.vtable)
    }
}

/// SAFETY: we forward all method impls to the underlying allocator.
unsafe impl Allocator for LocalPolyAllocator<'_> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { (self.vtable.allocate)(self.data.as_ptr(), layout) }
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { (self.vtable.allocate_zeroed)(self.data.as_ptr(), layout) }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        unsafe { (self.vtable.deallocate)(self.data.as_ptr(), ptr, layout) }
    }

    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { (self.vtable.grow)(self.data.as_ptr(), ptr, old_layout, new_layout) }
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { (self.vtable.grow_zeroed)(self.data.as_ptr(), ptr, old_layout, new_layout) }
    }

    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { (self.vtable.shrink)(self.data.as_ptr(), ptr, old_layout, new_layout) }
    }
}

// Allocator wrappers

/// A polymorphic allocator which can be sent across threads.
#[derive(Clone, Debug)]
pub struct PolyAllocator<'a>(LocalPolyAllocator<'a>);

/// SAFETY: Only constructed with `Send` underlying allocators.
unsafe impl Send for PolyAllocator<'_> {}

impl<'a> PolyAllocator<'a> {
    pub fn owned<A>(allocator: A) -> Self
    where
        A: Allocator + Clone + Send + 'a,
    {
        Self(LocalPolyAllocator::owned(allocator))
    }

    pub fn borrowed<A>(allocator: &'a A) -> Self
    where
        A: Allocator + Send + 'a,
    {
        Self(LocalPolyAllocator::borrowed(allocator))
    }
}

/// SAFETY: we forward all method impls to the underlying allocator.
unsafe impl Allocator for PolyAllocator<'_> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.0.allocate(layout)
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.0.allocate_zeroed(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        unsafe { self.0.deallocate(ptr, layout) }
    }

    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { self.0.grow(ptr, old_layout, new_layout) }
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { self.0.grow_zeroed(ptr, old_layout, new_layout) }
    }

    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { self.0.shrink(ptr, old_layout, new_layout) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::Global;

    #[test]
    fn it_works() {
        println!("PolyAllocator: {}", core::mem::size_of::<PolyAllocator>());
        println!(
            "Box<(), PolyAllocator>: {}",
            core::mem::size_of::<Box<(), PolyAllocator>>()
        );
        let allocator = PolyAllocator::owned(PolyAllocator::owned(PolyAllocator::owned(Global)));
        let mut v = Vec::new_in(allocator);
        v.push(3);
        v.push(4);

        let _ = v.clone();

        let (a, mut _b) = (Global, None);
        _b = Some(PolyAllocator::borrowed(&a));

        // let allocator = PolyAllocator::new(Global);
        // let v = Box::new_in(3, allocator);
    }
}
