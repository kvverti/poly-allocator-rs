#![feature(allocator_api)]
#![forbid(unsafe_op_in_unsafe_fn)]
#![no_std]

extern crate alloc;

pub mod allocator;
pub mod traits;
pub mod vtable;

pub type LocalPolyAllocator<'a> = allocator::PolyAllocator<'a, traits::LocalTrait>;
pub type SendPolyAllocator<'a> = allocator::PolyAllocator<'a, traits::SendTrait>;
pub type SharedPolyAllocator<'a> = allocator::PolyAllocator<'a, traits::SendSyncTrait>;

#[cfg(test)]
mod tests {
    use alloc::alloc::Global;
    use alloc::vec::Vec;

    use super::*;

    #[test]
    fn it_works() {
        let allocator =
            SendPolyAllocator::owned(SendPolyAllocator::owned(SendPolyAllocator::owned(Global)));
        let mut v = Vec::new_in(allocator);
        v.push(3);
        v.push(4);

        let _ = v.clone();

        let (a, mut _b) = (Global, None);
        _b = Some(SendPolyAllocator::borrowed(&a));

        // let allocator = PolyAllocator::new(Global);
        // let v = Box::new_in(3, allocator);
    }
}
