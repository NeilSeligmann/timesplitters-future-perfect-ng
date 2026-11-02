// SPDX-License-Identifier: GPL-3.0-or-later
//! Small unsafe ABI adapter; all RTL behaviour stays in the safe memory crate.
use std::sync::Mutex;
use tsfp_engine_memory::{Access, GuestMemory, MemoryFault, kernel_rtl::KernelRtl};
use tsfp_types::GuestAddr;

type Read = unsafe extern "C" fn(u32, *mut u8, usize) -> bool;
type Write = unsafe extern "C" fn(u32, *const u8, usize) -> bool;
struct HostMemory {
    read: Read,
    write: Write,
}
impl GuestMemory for HostMemory {
    fn read(&mut self, address: GuestAddr, out: &mut [u8]) -> Result<(), MemoryFault> {
        if unsafe { (self.read)(address.0, out.as_mut_ptr(), out.len()) } {
            Ok(())
        } else {
            Err(MemoryFault {
                address,
                length: out.len(),
                access: Access::Read,
            })
        }
    }
    fn write(&mut self, address: GuestAddr, bytes: &[u8]) -> Result<(), MemoryFault> {
        if unsafe { (self.write)(address.0, bytes.as_ptr(), bytes.len()) } {
            Ok(())
        } else {
            Err(MemoryFault {
                address,
                length: bytes.len(),
                access: Access::Write,
            })
        }
    }
}
static RTL: Mutex<Option<KernelRtl>> = Mutex::new(None);

/// Callbacks must synchronously access only the supplied slice and return access success.
/// `args` must point to `count` readable u32 values. The C thunk validates its arity.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rtl_call(
    ordinal: u32,
    args: *const u32,
    count: usize,
    read: Read,
    write: Write,
) -> u32 {
    if args.is_null() || count > 3 {
        return 0;
    }
    let args = unsafe { std::slice::from_raw_parts(args, count) };
    let mut state = RTL.lock().unwrap_or_else(|e| e.into_inner());
    state
        .get_or_insert_with(KernelRtl::default)
        .dispatch(&mut HostMemory { read, write }, ordinal, args)
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rtl_reset() {
    *RTL.lock().unwrap_or_else(|e| e.into_inner()) = Some(KernelRtl::default());
}

/// Counter order documented in docs/t-kernel-rtl-rust.md.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rtl_counter(index: u32) -> u32 {
    let state = RTL.lock().unwrap_or_else(|e| e.into_inner());
    let Some(rtl) = state.as_ref() else {
        return 0;
    };
    [
        rtl.truncated,
        rtl.nonascii_folds,
        u32::from(rtl.last_nonascii),
        rtl.nonascii_conversions,
        rtl.compare_refused,
        rtl.time_refused,
        rtl.unmapped,
        rtl.last_unmapped,
    ]
    .get(index as usize)
    .copied()
    .unwrap_or(0)
}
