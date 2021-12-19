use alloc::alloc::handle_alloc_error;
use core::alloc::{AllocError, Allocator, Layout};
use core::marker::PhantomData;
use core::ptr::{self, NonNull};

use crate::traits::*;
use crate::vtable::RawPolyAllocVTable;

/// A polymorphic allocator.
#[derive(Debug)]
pub struct PolyAllocator<'a, Traits> {
    /// A pointer to the allocator (erased). In the case of owned allocations, the memory behind
    /// the pointer is allocated by the allocator.
    data: NonNull<()>,
    /// A reference to the vtable.
    vtable: &'static RawPolyAllocVTable,
    _ph: PhantomData<dyn Allocator + 'a>,
    _traits: PhantomData<Traits>,
}

// autp trait impls

/// SAFETY: We only allow constructing with Send backing allocators when using this trait.
unsafe impl Send for PolyAllocator<'_, SendTrait> {}

/// SAFETY: We only allow constructing with Send backing allocators when using this trait.
unsafe impl Send for PolyAllocator<'_, SendSyncTrait> {}

/// SAFETY: We only allow constructing with Sync backing allocators when using this trait.
unsafe impl Sync for PolyAllocator<'_, SendSyncTrait> {}

// Drop

impl<Traits> Drop for PolyAllocator<'_, Traits> {
    fn drop(&mut self) {
        /// SAFETY: This allocator was constructed with a valid deleter and we will not
        /// access the data again.
        unsafe {
            (self.vtable.delete)(self.data);
        }
    }
}

// Clone

impl<Traits> Clone for PolyAllocator<'_, Traits> {
    fn clone(&self) -> Self {
        // SAFETY: We have a proper new data pointer from the clone method in the vtable
        unsafe { Self::from_raw_parts((self.vtable.clone)(self.data.as_ptr()), self.vtable) }
    }
}

impl<'a, Traits> PolyAllocator<'a, Traits> {
    /// SAFETY: `vtable` must be a vtable compatible with the allocator type underlying `data`.
    ///         Additionally, the underlying type must live for `'a`.
    pub unsafe fn from_raw_parts(data: NonNull<()>, vtable: &'static RawPolyAllocVTable) -> Self {
        Self {
            data,
            vtable,
            _ph: PhantomData,
            _traits: PhantomData,
        }
    }

    pub fn into_raw_parts(self) -> (NonNull<()>, &'static RawPolyAllocVTable) {
        (self.data, self.vtable)
    }

    fn try_owned_internal<A>(allocator: A) -> Result<Self, AllocError>
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

    fn owned_internal<A>(allocator: A) -> Self
    where
        A: Allocator + Clone + 'a,
    {
        match Self::try_owned_internal(allocator) {
            Ok(ret) => ret,
            Err(_) => handle_alloc_error(Layout::new::<A>()),
        }
    }

    fn borrowed_internal<A>(allocator: &'a A) -> Self
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
}

impl<'a> PolyAllocator<'a, LocalTrait> {
    /// Construct a polymorphic allocator by placing an allocator in its own storage.
    /// Returns an error if storage could not be allocated.
    pub fn try_owned<A>(allocator: A) -> Result<Self, AllocError>
    where
        A: Allocator + Clone + 'a,
    {
        Self::try_owned_internal(allocator)
    }

    /// Construct a polymorphic allocator by placing an allocator in its own storage.
    pub fn owned<A>(allocator: A) -> Self
    where
        A: Allocator + Clone + 'a,
    {
        Self::owned_internal(allocator)
    }

    /// Construct a polymorphic allocator from a borrow of an allocator.
    pub fn borrowed<A>(allocator: &'a A) -> Self
    where
        A: Allocator + 'a,
    {
        Self::borrowed_internal(allocator)
    }
}

impl<'a> PolyAllocator<'a, SendTrait> {
    /// Construct a polymorphic allocator by placing an allocator in its own storage.
    /// Returns an error if storage could not be allocated.
    pub fn try_owned<A>(allocator: A) -> Result<Self, AllocError>
    where
        A: Allocator + Clone + Send + 'a,
    {
        Self::try_owned_internal(allocator)
    }

    /// Construct a polymorphic allocator by placing an allocator in its own storage.
    pub fn owned<A>(allocator: A) -> Self
    where
        A: Allocator + Clone + Send + 'a,
    {
        Self::owned_internal(allocator)
    }

    /// Construct a polymorphic allocator from a borrow of an allocator.
    pub fn borrowed<A>(allocator: &'a A) -> Self
    where
        A: Allocator + Sync + 'a,
    {
        Self::borrowed_internal(allocator)
    }
}

impl<'a> PolyAllocator<'a, SendSyncTrait> {
    /// Construct a polymorphic allocator by placing an allocator in its own storage.
    /// Returns an error if storage could not be allocated.
    pub fn try_owned<A>(allocator: A) -> Result<Self, AllocError>
    where
        A: Allocator + Clone + Send + Sync + 'a,
    {
        Self::try_owned_internal(allocator)
    }

    /// Construct a polymorphic allocator by placing an allocator in its own storage.
    pub fn owned<A>(allocator: A) -> Self
    where
        A: Allocator + Clone + Send + Sync + 'a,
    {
        Self::owned_internal(allocator)
    }

    /// Construct a polymorphic allocator from a borrow of an allocator.
    pub fn borrowed<A>(allocator: &'a A) -> Self
    where
        A: Allocator + Sync + 'a,
    {
        Self::borrowed_internal(allocator)
    }
}

/// SAFETY: we forward all method impls to the underlying allocator.
unsafe impl<Traits> Allocator for PolyAllocator<'_, Traits> {
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
