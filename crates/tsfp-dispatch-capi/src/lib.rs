// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI of `tsfp-dispatch` (T3869, lane B phase 1). Linked into the host only with
//! `-DTSFP_RUST_DISPATCH=ON` (cmake/rust-dispatch.cmake) and selected at run time with
//! `TSFP_RUST_DISPATCH=1` (src/host/rust_dispatch.c); the C dispatcher stays the default and the
//! oracle. Declarations: src/host/rust_dispatch.h.
//!
//! Every entry catches panics (a panic must not unwind into C frames) and reports failure. A guest
//! stop inside a called function leaves by `siglongjmp` across these frames, so no value with a
//! destructor is live across a guest call (see `tsfp_dispatch::guest_call`).
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;

use tsfp_dispatch::{
    Agreement, DispatchTable, FpuState, GuestThread, Registers, TableError, call_cdecl,
    call_stdcall, verify_against,
};

/// A lifted guest function (C `recomp_func_t`).
pub type GuestFn = unsafe extern "C" fn();

/// ABI version; the C side refuses a library that does not report this value.
pub const ABI_VERSION: u32 = 1;

/// One generated dispatch row, C `recomp_dispatch_row` / generated `recomp_entry_t`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DispatchRow {
    pub xbox_va: u32,
    pub func: Option<GuestFn>,
}

/// `tsfp_dispatch_install` results.
pub const INSTALL_OK: i32 = 0;
pub const INSTALL_ALREADY: i32 = 1;
pub const INSTALL_NULL_ROWS: i32 = -1;
pub const INSTALL_UNSORTED: i32 = -2;
pub const INSTALL_SPAN_TOO_LARGE: i32 = -3;
pub const INSTALL_NULL_FUNCTION: i32 = -4;
pub const INSTALL_PANIC: i32 = -5;

/// Agreement report for C, `tsfp_dispatch_agreement`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AgreementReport {
    pub rows: u64,
    pub checked: u64,
    pub mismatches: u64,
    /// First differing VA, valid when `mismatches != 0`.
    pub first_mismatch: u32,
}

impl From<Agreement> for AgreementReport {
    fn from(agreement: Agreement) -> Self {
        Self {
            rows: agreement.rows,
            checked: agreement.checked,
            mismatches: agreement.mismatches,
            first_mismatch: agreement.first_mismatch.unwrap_or(0),
        }
    }
}

/// The current guest thread, supplied by the C host (its register file is C thread-local).
#[repr(C)]
pub struct GuestThreadHost {
    /// Writes EAX, ECX, EDX, EBX, ESI, EDI, EBP, ESP to `out[8]`.
    pub read_registers: Option<unsafe extern "C" fn(out: *mut u32)>,
    /// Sets the eight registers from `words[8]`, same order.
    pub write_registers: Option<unsafe extern "C" fn(words: *const u32)>,
    pub read_fpu: Option<unsafe extern "C" fn(top: *mut i32, control_word: *mut u16)>,
    pub write_fpu: Option<unsafe extern "C" fn(top: i32, control_word: u16)>,
    /// `g_fp_stack[index]`.
    pub fpu_register: Option<unsafe extern "C" fn(index: u32) -> f64>,
    /// C `kernel_guest_write_bytes`: all or nothing.
    pub write_guest:
        Option<unsafe extern "C" fn(address: u32, bytes: *const u8, length: usize) -> bool>,
}

struct HostThread<'a> {
    read_registers: unsafe extern "C" fn(*mut u32),
    write_registers: unsafe extern "C" fn(*const u32),
    read_fpu: unsafe extern "C" fn(*mut i32, *mut u16),
    write_fpu: unsafe extern "C" fn(i32, u16),
    fpu_register: unsafe extern "C" fn(u32) -> f64,
    write_guest: unsafe extern "C" fn(u32, *const u8, usize) -> bool,
    _host: core::marker::PhantomData<&'a GuestThreadHost>,
}

impl<'a> HostThread<'a> {
    fn new(host: &'a GuestThreadHost) -> Option<Self> {
        Some(Self {
            read_registers: host.read_registers?,
            write_registers: host.write_registers?,
            read_fpu: host.read_fpu?,
            write_fpu: host.write_fpu?,
            fpu_register: host.fpu_register?,
            write_guest: host.write_guest?,
            _host: core::marker::PhantomData,
        })
    }
}

impl GuestThread<GuestFn> for HostThread<'_> {
    fn registers(&self) -> Registers {
        let mut words = [0u32; 8];
        // SAFETY: the host callback writes exactly eight words.
        unsafe { (self.read_registers)(words.as_mut_ptr()) };
        Registers::from_array(words)
    }
    fn set_registers(&mut self, registers: Registers) {
        let words = registers.to_array();
        // SAFETY: the host callback reads exactly eight words.
        unsafe { (self.write_registers)(words.as_ptr()) };
    }
    fn fpu(&self) -> FpuState {
        let mut state = FpuState::default();
        // SAFETY: both out pointers are valid locals.
        unsafe { (self.read_fpu)(&mut state.top, &mut state.control_word) };
        state
    }
    fn set_fpu(&mut self, state: FpuState) {
        // SAFETY: plain values.
        unsafe { (self.write_fpu)(state.top, state.control_word) };
    }
    fn fpu_register(&self, index: usize) -> f64 {
        // SAFETY: index is masked to 0..8 by the caller.
        unsafe { (self.fpu_register)(index as u32 & 7) }
    }
    fn write_guest(&mut self, address: u32, bytes: &[u8]) -> bool {
        // SAFETY: `bytes` is a live slice for the duration of the call.
        unsafe { (self.write_guest)(address, bytes.as_ptr(), bytes.len()) }
    }
    fn invoke(&mut self, function: GuestFn) {
        // SAFETY: the function came from the generated dispatch table of this process.
        unsafe { function() }
    }
}

static TABLE: OnceLock<DispatchTable<GuestFn>> = OnceLock::new();

/// Builds a table from C rows (shared by install and the oracle tests).
///
/// # Safety
/// `rows` must point at `count` readable rows.
pub unsafe fn table_from_rows(
    rows: *const DispatchRow,
    count: usize,
) -> Result<DispatchTable<GuestFn>, i32> {
    if rows.is_null() && count != 0 {
        return Err(INSTALL_NULL_ROWS);
    }
    let rows = if count == 0 {
        &[][..]
    } else {
        // SAFETY: guaranteed by the caller.
        unsafe { core::slice::from_raw_parts(rows, count) }
    };
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        entries.push((row.xbox_va, row.func.ok_or(INSTALL_NULL_FUNCTION)?));
    }
    DispatchTable::new(entries).map_err(|error| match error {
        TableError::Unsorted { .. } => INSTALL_UNSORTED,
        TableError::SpanTooLarge { .. } => INSTALL_SPAN_TOO_LARGE,
    })
}

/// The installed table, if any.
pub fn installed() -> Option<&'static DispatchTable<GuestFn>> {
    TABLE.get()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_dispatch_abi_version() -> u32 {
    ABI_VERSION
}

/// Installs the dispatch rows once per process (the host passes `recomp_dispatch_entries`).
///
/// # Safety
/// `rows` must point at `count` readable rows; the function pointers must stay valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_dispatch_install(rows: *const DispatchRow, count: usize) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if TABLE.get().is_some() {
            return INSTALL_ALREADY;
        }
        // SAFETY: forwarded caller contract.
        match unsafe { table_from_rows(rows, count) } {
            Ok(table) => {
                if TABLE.set(table).is_ok() {
                    INSTALL_OK
                } else {
                    INSTALL_ALREADY
                }
            }
            Err(code) => code,
        }
    }))
    .unwrap_or(INSTALL_PANIC)
}

/// Rust `recomp_lookup`: NULL when nothing is installed or the VA has no function.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_dispatch_lookup(xbox_va: u32) -> Option<GuestFn> {
    TABLE.get().and_then(|table| table.lookup(xbox_va))
}

/// Number of installed rows (0 when nothing is installed).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_dispatch_count() -> usize {
    TABLE.get().map_or(0, DispatchTable::len)
}

/// Compares the installed table with `oracle` (C `recomp_lookup`) for every row, its neighbours and
/// the span edges. True only when rows were installed and every probe agrees.
///
/// # Safety
/// `oracle` must be callable with any VA; `report` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_dispatch_verify(
    oracle: Option<unsafe extern "C" fn(u32) -> Option<GuestFn>>,
    report: *mut AgreementReport,
) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let (Some(oracle), Some(table)) = (oracle, TABLE.get()) else {
            return false;
        };
        // SAFETY: guaranteed by the caller.
        let agreement = verify_against(table, |va| unsafe { oracle(va) });
        if !report.is_null() {
            // SAFETY: checked non-null, writable by contract.
            unsafe { report.write(agreement.into()) };
        }
        agreement.agrees()
    }))
    .unwrap_or(false)
}

/// # Safety
/// `arguments` must hold `count` words when `count != 0`.
unsafe fn argument_slice<'a>(arguments: *const u32, count: u32) -> Option<&'a [u32]> {
    match (count, arguments.is_null()) {
        (0, _) => Some(&[]),
        (_, true) => None,
        // SAFETY: guaranteed by the caller.
        _ => Some(unsafe { core::slice::from_raw_parts(arguments, count as usize) }),
    }
}

/// Rust `recomp_guest_call_stdcall` over the installed table. False on refusal, imbalance, a
/// missing table or a host vtable with an empty slot.
///
/// # Safety
/// `arguments` holds `count` words; `host` is valid and its callbacks act on the current guest thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_dispatch_guest_call_stdcall(
    address: u32,
    arguments: *const u32,
    count: u32,
    host: *const GuestThreadHost,
) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        // Over-long lists are refused by count before the pointer is read, like C.
        if count as usize > tsfp_dispatch::STDCALL_MAX_ARGS {
            return false;
        }
        // SAFETY: forwarded caller contract.
        let (Some(table), Some(arguments), Some(host)) = (
            TABLE.get(),
            unsafe { argument_slice(arguments, count) },
            unsafe { host.as_ref() },
        ) else {
            return false;
        };
        let Some(mut thread) = HostThread::new(host) else {
            return false;
        };
        call_stdcall(&mut thread, table, address, arguments).is_ok()
    }))
    .unwrap_or(false)
}

/// Rust `recomp_guest_call_cdecl` / `_st0`: `result` and `st0` are each null or writable and are
/// written whenever the call ran (balanced or not), as in C. Returns the balance.
///
/// # Safety
/// As [`tsfp_dispatch_guest_call_stdcall`]; `result` and `st0` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_dispatch_guest_call_cdecl(
    address: u32,
    arguments: *const u32,
    count: u32,
    host: *const GuestThreadHost,
    result: *mut u32,
    st0: *mut f64,
) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        if count as usize > tsfp_dispatch::CDECL_MAX_ARGS {
            return false;
        }
        // SAFETY: forwarded caller contract.
        let (Some(table), Some(arguments), Some(host)) = (
            TABLE.get(),
            unsafe { argument_slice(arguments, count) },
            unsafe { host.as_ref() },
        ) else {
            return false;
        };
        let Some(mut thread) = HostThread::new(host) else {
            return false;
        };
        match call_cdecl(&mut thread, table, address, arguments) {
            Ok(outcome) => {
                // SAFETY: null or writable by contract.
                unsafe {
                    if let Some(result) = result.as_mut() {
                        *result = outcome.eax;
                    }
                    if let Some(st0) = st0.as_mut() {
                        *st0 = outcome.st0;
                    }
                }
                outcome.balanced
            }
            // A refusal (nothing ran, nothing written) or a cdecl call never reports Unbalanced.
            Err(_) => false,
        }
    }))
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" fn first() {}
    unsafe extern "C" fn second() {}

    #[test]
    fn rows_convert_and_bad_rows_are_refused() {
        let rows = [
            DispatchRow {
                xbox_va: 0x1000,
                func: Some(first),
            },
            DispatchRow {
                xbox_va: 0x1010,
                func: Some(second),
            },
        ];
        // SAFETY: local array of two rows.
        let table = unsafe { table_from_rows(rows.as_ptr(), 2) }.unwrap();
        assert_eq!(
            table.lookup(0x1010).map(|f| f as usize),
            Some(second as GuestFn as usize)
        );
        let unsorted = [rows[1], rows[0]];
        // SAFETY: local arrays.
        assert_eq!(
            unsafe { table_from_rows(unsorted.as_ptr(), 2) }.err(),
            Some(INSTALL_UNSORTED)
        );
        let null = [DispatchRow {
            xbox_va: 0x1000,
            func: None,
        }];
        // SAFETY: local array.
        assert_eq!(
            unsafe { table_from_rows(null.as_ptr(), 1) }.err(),
            Some(INSTALL_NULL_FUNCTION)
        );
        // SAFETY: null with a non-zero count is refused before reading.
        assert_eq!(
            unsafe { table_from_rows(core::ptr::null(), 3) }.err(),
            Some(INSTALL_NULL_ROWS)
        );
    }

    #[test]
    fn entries_before_install_fail_closed() {
        // This test binary never installs, so every entry reports "no table".
        assert_eq!(tsfp_dispatch_abi_version(), ABI_VERSION);
        assert!(tsfp_dispatch_lookup(0x1000).is_none());
        assert_eq!(tsfp_dispatch_count(), 0);
        // SAFETY: null host and report are refused before use.
        unsafe {
            assert!(!tsfp_dispatch_verify(None, core::ptr::null_mut()));
            assert!(!tsfp_dispatch_guest_call_stdcall(
                0x1000,
                core::ptr::null(),
                0,
                core::ptr::null()
            ));
            assert!(!tsfp_dispatch_guest_call_cdecl(
                0x1000,
                core::ptr::null(),
                9,
                core::ptr::null(),
                core::ptr::null_mut(),
                core::ptr::null_mut()
            ));
        }
    }
}
