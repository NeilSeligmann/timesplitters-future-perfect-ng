// SPDX-License-Identifier: GPL-3.0-or-later
//! Short-lived mapped extension access. TEMPORARY-SHIM, removal T2411.
use super::ExtensionAccess;
use crate::OwnerError;

/// The exact extent needed by the compatibility publisher, including +0xBAC.
const EXTENT: u32 = 0xbb0;

/// A host mapping descriptor, never a retained Rust reference to guest storage.
/// Each access rechecks the mapping before borrowing it; callbacks run only
/// after that borrow ends. The validator must check writable allocation/lifetime
/// ownership, not merely whether an address happens to be readable.
pub struct Window<V> {
    pointer: core::ptr::NonNull<u8>,
    extension: u32,
    validate: V,
}

impl<L, V: FnMut(u32, u32) -> Result<L, OwnerError>> Window<V> {
    /// Construct from the same host offset used by the guest-memory adapter.
    ///
    /// # Safety
    /// Whenever `validate` succeeds, its returned lease must keep the extension initialized,
    /// writable storage in one allocation at `offset + extension`, stable until
    /// the operation returns. The caller must exclude concurrent access and all
    /// overlapping Rust references during each operation. Acquisition must be
    /// nonblocking and run no guest callbacks. The lease is dropped after the
    /// operation, while the owner lock is still held: its destructor must not
    /// reenter the owner, call guest code, or acquire another lock. Returning `()`
    /// is valid only if the caller independently guarantees this entire contract.
    /// No operation may free or remap this storage. Between operations there is
    /// no Rust borrow; external effects may use it or invalidate the allocation,
    /// provided validation rejects later accesses to the invalidated lifetime.
    /// A readability-only probe does not satisfy these requirements.
    pub unsafe fn new(offset: usize, extension: u32, validate: V) -> Option<Self> {
        if extension == 0 {
            return None;
        }
        extension.checked_add(EXTENT)?;
        let address = offset.checked_add(extension as usize)?;
        address.checked_add(EXTENT as usize)?;
        Some(Self {
            pointer: core::ptr::NonNull::new(address as *mut u8)?,
            extension,
            validate,
        })
    }
}

impl<L, V: FnMut(u32, u32) -> Result<L, OwnerError>> ExtensionAccess for Window<V> {
    fn with<R>(&mut self, operation: impl FnOnce(&mut [u8]) -> R) -> Result<R, OwnerError> {
        let lease = (self.validate)(self.extension, EXTENT)?;
        // SAFETY: the constructor's contract requires a live, exclusive mapped
        // allocation after validation. The closure cannot return a reference
        // tied to this short borrow; no reference is kept in the descriptor.
        let bytes =
            unsafe { core::slice::from_raw_parts_mut(self.pointer.as_ptr(), EXTENT as usize) };
        let result = operation(bytes);
        // The slice borrow has ended before releasing the mapping. Unwinding
        // also drops this local lease; no lease survives into a guest effect.
        drop(lease);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    #[test]
    fn rejects_null_and_wrapping_windows_without_validation() {
        for (offset, extension) in [(0, 0), (0, u32::MAX - 1), (usize::MAX, 1)] {
            // SAFETY: rejected descriptors never access storage.
            let window = unsafe {
                Window::new(offset, extension, |_, _| -> Result<(), OwnerError> {
                    panic!()
                })
            };
            assert!(window.is_none());
        }
    }

    #[test]
    fn revalidates_lifetime_before_every_access_and_preserves_canaries() {
        let mut storage = [0xa5u8; EXTENT as usize + 2];
        // Keep the byte-width check before erasing the pointer type. An inferred
        // integer array would make the canary indices disagree with byte writes.
        let pointer: *mut u8 = storage.as_mut_ptr();
        let base = pointer as usize + 1;
        let live = Cell::new(true);
        let checks = Cell::new(0);
        // SAFETY: storage stays allocated, and is accessed exclusively inside
        // with while the descriptor is used. No callback or thread shares it.
        let mut window = unsafe {
            Window::new(base - 0x1000, 0x1000, |address, length| {
                assert_eq!((address, length), (0x1000, EXTENT));
                checks.set(checks.get() + 1);
                if live.get() {
                    Ok(())
                } else {
                    Err(OwnerError::Stale)
                }
            })
        }
        .unwrap();
        window
            .with(|bytes| {
                bytes[0] = 7;
                bytes[EXTENT as usize - 1] = 9;
            })
            .unwrap();
        live.set(false);
        assert_eq!(
            window.with(|_| panic!("invalid lifetime borrowed")),
            Err::<(), _>(OwnerError::Stale)
        );
        assert_eq!(checks.get(), 2);
        drop(window);
        let mut expected = [0xa5u8; EXTENT as usize + 2];
        expected[1] = 7;
        expected[EXTENT as usize] = 9;
        assert_eq!(storage, expected);
    }

    #[test]
    fn holds_mapping_lease_through_write_and_releases_before_return() {
        struct Lease<'a>(&'a Cell<bool>);
        impl Drop for Lease<'_> {
            fn drop(&mut self) {
                assert!(self.0.replace(false));
            }
        }
        let mut storage = [0u8; EXTENT as usize];
        let held = Cell::new(false);
        let available = Cell::new(true);
        let base = storage.as_mut_ptr() as usize;
        // SAFETY: this fixture owns storage for the whole descriptor lifetime;
        // only the scoped operation accesses it. The lease models host pinning.
        let mut window = unsafe {
            Window::new(base - 1, 1, |_, _| {
                if !available.get() {
                    return Err(OwnerError::Busy);
                }
                assert!(!held.replace(true));
                Ok(Lease(&held))
            })
        }
        .unwrap();
        for value in [7, 9] {
            let result = window.with(|bytes| {
                assert!(held.get());
                bytes[0] = value;
                value
            });
            assert_eq!(result, Ok(value));
            assert!(!held.get());
        }
        available.set(false);
        assert_eq!(
            window.with(|_| panic!("failed acquisition executed operation")),
            Err::<(), _>(OwnerError::Busy)
        );
        assert!(!held.get());
        drop(window);
        assert_eq!(storage[0], 9);
        assert!(storage[1..].iter().all(|byte| *byte == 0));
    }
}
