use core::ptr::{self, NonNull};
use std::alloc::{self, AllocError, Allocator, Layout};

/// Allocator trait vtable struct.
/// SAFETY: All functions must be called using a valid data pointer for the type
/// represented in this vtable.
#[derive(Debug)]
pub struct RawPolyAllocVTable {
    pub allocate: unsafe fn(*const (), Layout) -> Result<NonNull<[u8]>, AllocError>,
    pub allocate_zeroed: unsafe fn(*const (), Layout) -> Result<NonNull<[u8]>, AllocError>,
    pub deallocate: unsafe fn(*const (), NonNull<u8>, Layout),
    pub grow:
        unsafe fn(*const (), NonNull<u8>, Layout, Layout) -> Result<NonNull<[u8]>, AllocError>,
    pub grow_zeroed:
        unsafe fn(*const (), NonNull<u8>, Layout, Layout) -> Result<NonNull<[u8]>, AllocError>,
    pub shrink:
        unsafe fn(*const (), NonNull<u8>, Layout, Layout) -> Result<NonNull<[u8]>, AllocError>,
    pub delete: unsafe fn(NonNull<()>),
    pub clone: unsafe fn(*const ()) -> NonNull<()>,
}

macro_rules! allocator_fwd {
    ($name:ident ($($param:ident : $typ:ty),*) $(-> $ret:ty)?) => {
        pub unsafe fn $name<A>(this: *const (), $($param: $typ),*) $(-> $ret)?
        where
            A: Allocator,
        {
            let this = this.cast::<A>();
            #[allow(unused_unsafe)]
            unsafe { (&*this).$name($($param),*) }
        }
    }
}

allocator_fwd!(allocate(layout: Layout) -> Result<NonNull<[u8]>, AllocError>);
allocator_fwd!(allocate_zeroed(layout: Layout) -> Result<NonNull<[u8]>, AllocError>);
allocator_fwd!(deallocate(ptr: NonNull<u8>, layout: Layout));
allocator_fwd!(grow(ptr: NonNull<u8>, old_layout: Layout, new_layout: Layout) -> Result<NonNull<[u8]>, AllocError>);
allocator_fwd!(grow_zeroed(ptr: NonNull<u8>, old_layout: Layout, new_layout: Layout) -> Result<NonNull<[u8]>, AllocError>);
allocator_fwd!(shrink(ptr: NonNull<u8>, old_layout: Layout, new_layout: Layout) -> Result<NonNull<[u8]>, AllocError>);

/// Moves the allocator out of its place, deallocates the backing memory, and drops the
/// allocator.
/// SAFETY: `this` must be a pointer to an allocator of type `A`. Additionally, the memory
///         must be allocated by the pointed-to allocator.
pub unsafe fn default_delete<A>(this: NonNull<()>)
where
    A: Allocator,
{
    let storage = this.cast::<A>();
    // SAFETY: `storage` holds an allocator of type `A` because of the
    //         function preconditions, and it can be deallocated because of
    //         the same.
    unsafe {
        let layout = Layout::new::<A>();
        let allocator = ptr::read(storage.as_ptr());
        allocator.deallocate(storage.cast::<u8>(), layout);
    }
    println!("Dropped allocator!");
}

/// Clones the underlying allocator into a new allocation.
/// SAFETY: `this` must point to a value of type `A`.
pub unsafe fn default_clone<A>(this: *const ()) -> NonNull<()>
where
    A: Allocator + Clone,
{
    let this = unsafe { &*this.cast::<A>() };
    let layout = Layout::new::<A>();
    let new_storage = match this.allocate(layout) {
        Ok(storage) => storage.cast::<A>(),
        Err(_) => alloc::handle_alloc_error(layout),
    };
    // SAFETY: we just allocated `new_storage` for a value of type `A`.
    unsafe {
        ptr::write(new_storage.as_ptr(), this.clone());
        new_storage.cast::<()>()
    }
}

/// Deleter for allocators which do not need `Drop`.
pub fn ref_delete(_this: NonNull<()>) {}

/// Specialized clone functionality for shared references to allocators.
/// SAFETY: `this` must point to a value of type `A`.
pub unsafe fn ref_clone<A>(this: *const ()) -> NonNull<()>
where
    A: Allocator,
{
    // SAFETY: `this` is necessarily non-null.
    unsafe { NonNull::new_unchecked(this as *mut ()) }
}

impl RawPolyAllocVTable {
    /// Returns a reference to a vtable compatible with `A`. This vtable is usable for modeling
    /// owned allocators.
    pub fn owned<A>() -> &'static Self
    where
        A: Allocator + Clone,
    {
        &Self {
            allocate: allocate::<A>,
            allocate_zeroed: allocate_zeroed::<A>,
            deallocate: deallocate::<A>,
            grow: grow::<A>,
            grow_zeroed: grow_zeroed::<A>,
            shrink: shrink::<A>,
            delete: default_delete::<A>,
            clone: default_clone::<A>,
        }
    }

    /// Returns a reference to a vtable compatible with `A`. This vtable is usable for modeling
    /// shared borrowed allocators.
    pub fn borrowed<A>() -> &'static Self
    where
        A: Allocator,
    {
        &Self {
            allocate: allocate::<A>,
            allocate_zeroed: allocate_zeroed::<A>,
            deallocate: deallocate::<A>,
            grow: grow::<A>,
            grow_zeroed: grow_zeroed::<A>,
            shrink: shrink::<A>,
            delete: ref_delete,
            clone: ref_clone::<A>,
        }
    }
}
