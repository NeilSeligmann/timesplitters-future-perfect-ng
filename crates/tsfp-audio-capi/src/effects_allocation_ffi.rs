//! Rust-owned allocation transaction for the effects binding guest views.
//!
//! The adapter supplies the host's guest-heap callbacks. Rust fixes allocation order and sizes,
//! rolls back incomplete allocations, and returns guest addresses. Guest writes use the
//! fault-safe process_vm address ABI separately.

use std::ptr;

const VIEW_SIZES: [u32; 6] = [296, 8080, 8088, 393_216, 4, 8088];
const INITIAL_HEAP_BYTES: u32 = 0x1_0000;

pub const EFFECTS_ALLOCATION_OK: u32 = 0;
pub const EFFECTS_ALLOCATION_OOM: u32 = 0x8007_000e;
pub const EFFECTS_ALLOCATION_REFUSED: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GuestMemoryOps {
    pub heap_create: Option<unsafe extern "C" fn(u32, u32, u32) -> u32>,
    pub heap_alloc: Option<unsafe extern "C" fn(u32, u32) -> u32>,
    pub heap_valid: Option<unsafe extern "C" fn(u32) -> bool>,
    pub heap_block_size: Option<unsafe extern "C" fn(u32, u32, *mut u32) -> bool>,
    pub heap_destroy: Option<unsafe extern "C" fn(u32) -> bool>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct EffectsAllocation {
    pub heap: u32,
    pub addresses: [u32; 6],
}

impl Default for EffectsAllocation {
    fn default() -> Self {
        Self {
            heap: 0,
            addresses: [0; 6],
        }
    }
}

/// Allocate the fixed effects binding views. A zero sixth address is required when GP staging is
/// disabled. Output is preserved on refusal. Callbacks must stay valid for this synchronous call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_effects_binding_allocate_rust(
    gp_enabled: bool,
    ops: *const GuestMemoryOps,
    output: *mut EffectsAllocation,
) -> u32 {
    if ops.is_null() || output.is_null() {
        return EFFECTS_ALLOCATION_REFUSED;
    }
    // SAFETY: the ABI requires one readable callback table for the duration of this call.
    let ops = unsafe { ptr::read(ops) };
    let (Some(heap_create), Some(heap_alloc), Some(heap_destroy)) =
        (ops.heap_create, ops.heap_alloc, ops.heap_destroy)
    else {
        return EFFECTS_ALLOCATION_REFUSED;
    };

    // SAFETY: validated synchronous callbacks operate on the newly-created heap.
    let heap = unsafe { heap_create(0, INITIAL_HEAP_BYTES, 0) };
    if heap == 0 {
        return EFFECTS_ALLOCATION_OOM;
    }

    let count = if gp_enabled {
        VIEW_SIZES.len()
    } else {
        VIEW_SIZES.len() - 1
    };
    let mut value = EffectsAllocation {
        heap,
        ..EffectsAllocation::default()
    };
    for (index, size) in VIEW_SIZES.iter().copied().take(count).enumerate() {
        // SAFETY: heap is live and each requested size is a fixed contract value.
        let address = unsafe { heap_alloc(heap, size) };
        if address == 0 {
            // SAFETY: this transaction created the heap and has not published it.
            unsafe { heap_destroy(heap) };
            return EFFECTS_ALLOCATION_OOM;
        }
        value.addresses[index] = address;
    }

    // SAFETY: output is writable for one EffectsAllocation per ABI contract. Commit only after
    // every allocation succeeded.
    unsafe { ptr::write(output, value) };
    EFFECTS_ALLOCATION_OK
}

/// Release an effects heap through the same typed guest-memory contract used by allocation.
/// Invalid or zero handles are refused without invoking the host destroy callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_effects_binding_destroy_rust(
    heap: u32,
    ops: *const GuestMemoryOps,
) -> bool {
    if heap == 0 || ops.is_null() {
        return false;
    }
    // SAFETY: the ABI requires one readable callback table for the duration of this call.
    let ops = unsafe { ptr::read(ops) };
    let (Some(heap_valid), Some(heap_destroy)) = (ops.heap_valid, ops.heap_destroy) else {
        return false;
    };
    // SAFETY: validated synchronous callbacks operate on the supplied heap handle.
    unsafe { heap_valid(heap) && heap_destroy(heap) }
}

fn intact_with(
    heap: u32,
    gp_enabled: bool,
    addresses: &[u32; 6],
    ops: GuestMemoryOps,
    expected_descriptor: &[u8],
    expected_code: &[u8],
    mut read_guest: impl FnMut(u32, &mut [u8]) -> bool,
) -> bool {
    if heap == 0 || expected_descriptor.len() != 296 || expected_code.len() != 8080 {
        return false;
    }
    let (Some(heap_valid), Some(heap_block_size)) = (ops.heap_valid, ops.heap_block_size) else {
        return false;
    };
    // SAFETY: the callback table is copied from the caller and remains valid for this call.
    if !unsafe { heap_valid(heap) } {
        return false;
    }
    let sizes = [296, 8080, 8088, 393_216, 4, 8088];
    for index in 0..if gp_enabled { 6 } else { 5 } {
        let mut requested = 0;
        // SAFETY: requested is a live output word and the callback is synchronous.
        if !unsafe { heap_block_size(heap, addresses[index], &mut requested) }
            || requested != sizes[index]
        {
            return false;
        }
    }
    let mut actual_descriptor = vec![0; expected_descriptor.len()];
    let mut actual_code = vec![0; expected_code.len()];
    read_guest(addresses[0], &mut actual_descriptor)
        && actual_descriptor == expected_descriptor
        && read_guest(addresses[1], &mut actual_code)
        && actual_code == expected_code
}

/// Check that the live effects heap and its immutable guest views still match the published binding.
///
/// # Safety
/// `ops` points to a readable callback table, `addresses` to six readable words, and the
/// descriptor/code pointers cover 296 and 8080 readable bytes respectively.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_effects_binding_intact_rust(
    host_pid: i32,
    heap: u32,
    gp_enabled: bool,
    ops: *const GuestMemoryOps,
    addresses: *const u32,
    descriptor: *const u8,
    code: *const u8,
) -> bool {
    if host_pid <= 0
        || heap == 0
        || ops.is_null()
        || addresses.is_null()
        || descriptor.is_null()
        || code.is_null()
    {
        return false;
    }
    // SAFETY: all pointers and their extents are part of this function's ABI contract.
    let ops = unsafe { ptr::read(ops) };
    let addresses = unsafe { &*addresses.cast::<[u32; 6]>() };
    let expected_descriptor = unsafe { std::slice::from_raw_parts(descriptor, 296) };
    let expected_code = unsafe { std::slice::from_raw_parts(code, 8080) };
    intact_with(
        heap,
        gp_enabled,
        addresses,
        ops,
        expected_descriptor,
        expected_code,
        |address, output| {
            // SAFETY: output is a writable Rust-owned buffer; the helper checks the guest span.
            unsafe {
                crate::effects_guest_copy_ffi::tsfp_effects_guest_copy_read_rust(
                    host_pid,
                    address,
                    output.as_mut_ptr(),
                    output.len(),
                )
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Calls {
        sizes: Vec<u32>,
        fail_alloc_at: Option<usize>,
        live_heap: bool,
        wrong_block_size: bool,
        destroys: usize,
    }

    thread_local! { static CALLS: RefCell<Calls> = RefCell::new(Calls::default()); }

    unsafe extern "C" fn create(_flags: u32, initial: u32, maximum: u32) -> u32 {
        assert_eq!((initial, maximum), (INITIAL_HEAP_BYTES, 0));
        7
    }
    unsafe extern "C" fn allocate(_heap: u32, size: u32) -> u32 {
        CALLS.with(|calls| {
            let mut calls = calls.borrow_mut();
            calls.sizes.push(size);
            if calls.fail_alloc_at == Some(calls.sizes.len()) {
                0
            } else {
                0x1000 + calls.sizes.len() as u32 * 0x10000
            }
        })
    }
    unsafe extern "C" fn destroy(_heap: u32) -> bool {
        CALLS.with(|calls| calls.borrow_mut().destroys += 1);
        true
    }
    unsafe extern "C" fn valid(_heap: u32) -> bool {
        CALLS.with(|calls| calls.borrow().live_heap)
    }
    unsafe extern "C" fn block_size(_heap: u32, address: u32, out: *mut u32) -> bool {
        let sizes = [296, 8080, 8088, 393_216, 4, 8088];
        let Some(index) = address.checked_sub(0x100).map(|offset| offset as usize) else {
            return false;
        };
        let Some(size) = sizes.get(index) else {
            return false;
        };
        if out.is_null() {
            return false;
        }
        let wrong = CALLS.with(|calls| calls.borrow().wrong_block_size);
        // SAFETY: the callback contract supplies one writable word.
        unsafe { out.write(*size + u32::from(wrong && index == 0)) };
        true
    }

    fn ops() -> GuestMemoryOps {
        GuestMemoryOps {
            heap_create: Some(create),
            heap_alloc: Some(allocate),
            heap_valid: Some(valid),
            heap_block_size: Some(block_size),
            heap_destroy: Some(destroy),
        }
    }

    #[test]
    fn fixed_effects_views_are_allocated_in_contract_order() {
        CALLS.with(|calls| *calls.borrow_mut() = Calls::default());
        let callback_ops = ops();
        let mut output = EffectsAllocation::default();
        let status =
            unsafe { tsfp_effects_binding_allocate_rust(true, &callback_ops, &mut output) };
        assert_eq!(status, EFFECTS_ALLOCATION_OK);
        assert_eq!(output.heap, 7);
        assert_eq!(CALLS.with(|calls| calls.borrow().sizes.clone()), VIEW_SIZES);
        assert_eq!(output.addresses[5], 0x1000 + 6 * 0x10000);
        assert_eq!(CALLS.with(|calls| calls.borrow().destroys), 0);
    }

    #[test]
    fn partial_allocation_failure_rolls_back_without_publishing() {
        CALLS.with(|calls| {
            let mut value = Calls::default();
            value.fail_alloc_at = Some(3);
            *calls.borrow_mut() = value;
        });
        let callback_ops = ops();
        let sentinel = EffectsAllocation {
            heap: 0xCAFE,
            ..EffectsAllocation::default()
        };
        let mut output = sentinel;
        let status =
            unsafe { tsfp_effects_binding_allocate_rust(false, &callback_ops, &mut output) };
        assert_eq!(status, EFFECTS_ALLOCATION_OOM);
        assert_eq!(output.heap, sentinel.heap);
        assert_eq!(CALLS.with(|calls| calls.borrow().destroys), 1);
    }

    #[test]
    fn reset_destroys_only_a_live_heap_through_the_contract() {
        CALLS.with(|calls| {
            let mut live = Calls::default();
            live.live_heap = true;
            *calls.borrow_mut() = live;
        });
        let callback_ops = ops();
        assert!(unsafe { tsfp_effects_binding_destroy_rust(7, &callback_ops) });
        assert_eq!(CALLS.with(|calls| calls.borrow().destroys), 1);

        CALLS.with(|calls| *calls.borrow_mut() = Calls::default());
        assert!(!unsafe { tsfp_effects_binding_destroy_rust(7, &callback_ops) });
        assert_eq!(CALLS.with(|calls| calls.borrow().destroys), 0);
        assert!(!unsafe { tsfp_effects_binding_destroy_rust(0, &callback_ops) });
    }

    #[test]
    fn intact_check_owns_heap_and_immutable_view_validation() {
        CALLS.with(|calls| {
            let mut live = Calls::default();
            live.live_heap = true;
            *calls.borrow_mut() = live;
        });
        let callback_ops = ops();
        let addresses = [0x100, 0x101, 0x102, 0x103, 0x104, 0x105];
        let descriptor = vec![0x31; 296];
        let code = vec![0x72; 8080];
        let read_expected = |address: u32, output: &mut [u8]| match address {
            0x100 if output.len() == descriptor.len() => {
                output.copy_from_slice(&descriptor);
                true
            }
            0x101 if output.len() == code.len() => {
                output.copy_from_slice(&code);
                true
            }
            _ => false,
        };
        assert!(intact_with(
            7,
            true,
            &addresses,
            callback_ops,
            &descriptor,
            &code,
            read_expected,
        ));

        CALLS.with(|calls| calls.borrow_mut().wrong_block_size = true);
        assert!(!intact_with(
            7,
            true,
            &addresses,
            callback_ops,
            &descriptor,
            &code,
            read_expected,
        ));
        CALLS.with(|calls| calls.borrow_mut().wrong_block_size = false);
        assert!(!intact_with(
            7,
            true,
            &addresses,
            callback_ops,
            &descriptor,
            &code,
            |address, output| {
                if !read_expected(address, output) {
                    return false;
                }
                if address == 0x101 {
                    output[0] ^= 1;
                }
                true
            },
        ));
    }
}
