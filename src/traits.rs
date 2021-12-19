use core::marker::PhantomData;

/// Marker for allocators that are neither Send nor Sync.
pub struct LocalTrait(PhantomData<*const ()>);

/// Marker for allocators that are Send but not Sync.
pub struct SendTrait(PhantomData<*const ()>);

/// SAFETY: This should be Send but not Sync for marking allocators.
unsafe impl Send for SendTrait {}

/// Marker for allocators that are both Send and Sync.
pub struct SendSyncTrait(PhantomData<()>);
