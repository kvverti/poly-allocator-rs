#![feature(allocator_api)]
#![feature(unsize)]
#![forbid(unsafe_op_in_unsafe_fn)]

use core::marker::{PhantomData, Unsize};
use core::ptr::{self, NonNull};
use std::alloc::{self, AllocError, Allocator, Layout};
use std::fmt::{self, Debug};

pub use crate::vtable::RawPolyAllocVTable;

pub mod vtable;

/// Holds an erased allocator data pointer and vtable reference.
/// The `Owned` type parameterizes this struct over the available auto traits, as well as
/// marking that this type owns a type-erased allocator.
struct RawPolyAllocator<Owned: ?Sized> {
    /// A pointer to the allocator (erased). The memory behind the pointer is
    /// allocated by the allocator.
    data: NonNull<()>,
    /// A reference to the vtable.
    vtable: &'static RawPolyAllocVTable,
    _ph: PhantomData<Owned>,
}

// Drop

impl<Owned: ?Sized> Drop for RawPolyAllocator<Owned> {
    fn drop(&mut self) {
        unsafe {
            (self.vtable.delete)(self.data);
        }
    }
}

// Send and Sync

unsafe impl<Owned: Send + ?Sized> Send for RawPolyAllocator<Owned> {}
unsafe impl<Owned: Sync + ?Sized> Sync for RawPolyAllocator<Owned> {}

// Clone

impl<Owned: ?Sized> Clone for RawPolyAllocator<Owned> {
    fn clone(&self) -> Self {
        Self {
            data: unsafe { (self.vtable.clone)(self.data.as_ptr()) },
            vtable: self.vtable,
            _ph: PhantomData,
        }
    }
}

impl<Owned: ?Sized> RawPolyAllocator<Owned> {
    /// SAFETY: `data` must be currently allocated by the allocator it points to, and `vtable`
    ///         must be a vtable compatible with the allocator type. Additionally, the `Owned`
    ///         type must be coercible from the allocator type.
    unsafe fn new_unchecked(data: NonNull<()>, vtable: &'static RawPolyAllocVTable) -> Self {
        Self {
            data,
            vtable,
            _ph: PhantomData,
        }
    }

    fn try_new<A>(allocator: A) -> Result<Self, AllocError>
    where
        A: Allocator + Clone + Unsize<Owned>,
    {
        let layout = Layout::new::<A>();
        let storage = allocator.allocate(layout)?.cast::<A>();
        // SAFETY: `storage` points to allocated memory for type `A`.
        unsafe {
            ptr::write(storage.as_ptr(), allocator);
            Ok(Self::new_unchecked(
                storage.cast::<()>(),
                RawPolyAllocVTable::of::<A>(),
            ))
        }
    }

    fn new<A>(allocator: A) -> Self
    where
        A: Allocator + Clone + Unsize<Owned>,
    {
        match Self::try_new(allocator) {
            Ok(ret) => ret,
            Err(_) => alloc::handle_alloc_error(Layout::new::<A>()),
        }
    }
}

// Allocator for RawPolyAllocator

/// SAFETY: we forward all method impls to the underlying allocator.
unsafe impl<Owned: ?Sized> Allocator for RawPolyAllocator<Owned> {
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

// Debug impl for all types
impl<Owned: ?Sized> Debug for RawPolyAllocator<Owned> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawPolyAllocator")
            .field("data", &self.data)
            .field("vtable", &self.vtable)
            .finish()
    }
}

// Allocator wrappers

#[derive(Clone, Debug)]
pub struct PolyAllocator<'a>(RawPolyAllocator<dyn Allocator + Send + 'a>);

impl<'a> PolyAllocator<'a> {
    pub fn new<A>(allocator: A) -> Self
    where
        A: Allocator + Clone + Send + 'a,
    {
        Self(RawPolyAllocator::new(allocator))
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
        // let allocator = PolyAllocator::new(Global);
        let allocator = PolyAllocator::new(PolyAllocator::new(PolyAllocator::new(Global)));
        let mut v = Vec::new_in(allocator);
        v.push(3);
        v.push(4);

        let _ = v.clone();

        let (a, mut _b) = (Global, None);
        _b = Some(PolyAllocator::new(&a));

        // let allocator = PolyAllocator::new(Global);
        // let v = Box::new_in(3, allocator);
    }
}
