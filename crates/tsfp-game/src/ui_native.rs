// SPDX-License-Identifier: GPL-3.0-or-later
//! Trace-host C ABI for the shared native UI diagnostics runner.
use tsfp_ui_input::native_page::GuestUi;

/// The C host's guest access table (src/host/main.c `host_ui_guest_api`).
#[repr(C)]
pub struct HostUiGuestApi {
    read32: extern "C" fn(u32, *mut u32) -> u32,
    write32: extern "C" fn(u32, u32) -> u32,
    call: extern "C" fn(u32, *const u32, u32, *mut u32) -> u32,
    call_st0: extern "C" fn(u32, *const u32, u32, *mut f64) -> u32,
}

struct Api<'a>(&'a HostUiGuestApi);

impl GuestUi for Api<'_> {
    fn read32(&self, va: u32) -> Option<u32> {
        let mut value = 0;
        ((self.0.read32)(va, &mut value) != 0).then_some(value)
    }
    fn write32(&mut self, va: u32, value: u32) -> bool {
        (self.0.write32)(va, value) != 0
    }
    fn call(&mut self, va: u32, args: &[u32]) -> Option<u32> {
        let mut result = 0;
        ((self.0.call)(va, args.as_ptr(), args.len() as u32, &mut result) != 0).then_some(result)
    }
    fn call_st0(&mut self, va: u32, args: &[u32]) -> Option<f64> {
        let mut st0 = 0.0;
        ((self.0.call_st0)(va, args.as_ptr(), args.len() as u32, &mut st0) != 0).then_some(st0)
    }
}

/// Frame boundary on the guest thread (weak symbol in the C host).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_ui_frame_service(api: *const HostUiGuestApi) {
    // SAFETY: the C host passes a pointer to its static API table.
    let Some(api) = (unsafe { api.as_ref() }) else { return };
    tsfp_ui_input::native_script::service(&mut Api(api));
}
