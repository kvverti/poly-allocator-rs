use core::cell::Cell;
use core::marker::PhantomData;

/// Marker for allocators that are neither Send nor Sync.
pub struct LocalTrait(PhantomData<&'static Cell<()>>);

/// Marker for allocators that are Send but not Sync.
pub struct SendTrait(PhantomData<Cell<()>>);

/// Marker for allocators that are both Send and Sync.
pub struct SendSyncTrait(PhantomData<()>);
