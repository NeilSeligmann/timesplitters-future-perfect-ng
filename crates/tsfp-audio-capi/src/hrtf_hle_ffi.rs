//! Rust-host registration and kernel-service adapter for DirectSoundUseLightHRTF (T3959: owns the former C unit).
use std::ffi::{CString, c_char, c_void};
use std::sync::Mutex;

use tsfp_audio::adapter::Refusal;
use tsfp_audio::hrtf::{self, CriticalState, HrtfKernel};

type DsoundHandler = unsafe extern "C" fn(*mut c_void) -> u32;
type IrqlProvider = unsafe extern "C" fn(*mut u8) -> bool;
type FatalFn = unsafe extern "C" fn(u32, *const c_char);

#[repr(C)]
struct CriticalInfo {
    cs: u32,
    owner: u32,
    recursion: u32,
    adopted: bool,
    guest_lock_count: u32,
}

unsafe extern "C" {
    fn dsound_hle_register_rust(address: u32) -> bool;
    fn dsound_hle_log_line(line: *const c_char);
    fn kernel_guest_at_ro(address: u32, length: usize) -> *const c_void;
    fn kernel_guest_write_u32(address: u32, value: u32) -> bool;
    fn kernel_critsec_enter_guest(cs: u32) -> u32;
    fn kernel_critsec_leave_guest(cs: u32) -> u32;
    fn kernel_critsec_state(cs: u32, out: *mut CriticalInfo) -> bool;
    fn kernel_critsec_owner_token() -> u32;
}

type CriticalCall = unsafe extern "C" fn(u32) -> u32;

static HOOKS: Mutex<(Option<IrqlProvider>, Option<FatalFn>)> = Mutex::new((None, None));
static CRITICAL: Mutex<(Option<CriticalCall>, Option<CriticalCall>)> = Mutex::new((None, None));

/// `dsound_hrtf_set_critical_calls`: tests wrap the genuine calls; `None` restores the defaults.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hrtf_set_critical_calls(
    enter: Option<CriticalCall>,
    leave: Option<CriticalCall>,
) {
    *CRITICAL.lock().unwrap() = (enter, leave);
}

/// `dsound_use_light_hrtf`: the original 0x00406AB6 handler body, callable directly.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_use_light_hrtf() -> u32 {
    hrtf::use_light(&mut HostHrtfKernel).unwrap_or_else(|refusal| refuse(refusal))
}

/// `dsound_hrtf_set_irql_provider`: the calling thread's IRQL byte source.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hrtf_set_irql_provider(provider: Option<IrqlProvider>) {
    HOOKS.lock().unwrap().0 = provider;
}

/// `dsound_hrtf_set_fatal`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hrtf_set_fatal(fatal: Option<FatalFn>) {
    HOOKS.lock().unwrap().1 = fatal;
}

struct HostHrtfKernel;

impl HrtfKernel for HostHrtfKernel {
    fn irql(&self) -> Option<u8> {
        let provider = HOOKS.lock().unwrap().0?;
        let mut irql = 0;
        // SAFETY: the host-installed provider writes one byte to the live local.
        unsafe { provider(&mut irql) }.then_some(irql)
    }

    fn mapped(&self, address: u32, bytes: u32) -> bool {
        // SAFETY: the host's checked guest-address mapping query.
        !unsafe { kernel_guest_at_ro(address, bytes as usize) }.is_null()
    }

    fn enter(&mut self, address: u32) -> u32 {
        // SAFETY: the model calls this only for its measured critical-section address.
        match CRITICAL.lock().unwrap().0 {
            // SAFETY: the test-installed wrapper takes the critical-section address.
            Some(call) => unsafe { call(address) },
            // SAFETY: as above, the default kernel call.
            None => unsafe { kernel_critsec_enter_guest(address) },
        }
    }

    fn critical_state(&self, address: u32) -> Option<CriticalState> {
        let mut info = CriticalInfo {
            cs: 0,
            owner: 0,
            recursion: 0,
            adopted: false,
            guest_lock_count: 0,
        };
        // SAFETY: the kernel fills the live local only when the query succeeds.
        unsafe { kernel_critsec_state(address, &mut info) }.then_some(CriticalState {
            owner: info.owner,
            recursion: info.recursion,
        })
    }

    fn owner_token(&self) -> u32 {
        // SAFETY: returns the current kernel critical-section owner token.
        unsafe { kernel_critsec_owner_token() }
    }

    fn write_word(&mut self, address: u32, value: u32) -> bool {
        // SAFETY: the model emits only the eleven measured table addresses and values.
        unsafe { kernel_guest_write_u32(address, value) }
    }

    fn leave(&mut self, address: u32) -> u32 {
        // SAFETY: the model calls this only after verified passive-level ownership.
        match CRITICAL.lock().unwrap().1 {
            // SAFETY: the test-installed wrapper takes the critical-section address.
            Some(call) => unsafe { call(address) },
            // SAFETY: as above, the default kernel call.
            None => unsafe { kernel_critsec_leave_guest(address) },
        }
    }
}

fn refuse(refusal: Refusal) -> ! {
    let line = format!(
        "dsound: DirectSoundUseLightHRTF -- REFUSED: {}\n",
        refusal.0
    );
    if let Ok(line) = CString::new(line) {
        // SAFETY: a live NUL-terminated string for this synchronous call.
        unsafe { dsound_hle_log_line(line.as_ptr()) };
    }
    let fatal = HOOKS.lock().unwrap().1;
    if let (Some(fatal), Ok(reason)) = (fatal, CString::new(refusal.0)) {
        // SAFETY: the host fatal hook takes the entry and a NUL-terminated reason.
        unsafe { fatal(hrtf::ENTRY, reason.as_ptr()) };
    }
    std::process::abort()
}

unsafe extern "C" fn use_light_handler(_context: *mut c_void) -> u32 {
    hrtf::use_light(&mut HostHrtfKernel).unwrap_or_else(|refusal| refuse(refusal))
}

/// Register the measured HRTF HLE callback in Rust and expose only its telemetry row to C.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_hrtf_register_rust() -> usize {
    if !crate::movie_device_hle_ffi::tsfp_audio_hle_register_handler_rust(
        hrtf::ENTRY,
        use_light_handler as DsoundHandler,
    ) {
        return 0;
    }
    // SAFETY: DirectSound initializes the compatibility table before subsystem handlers.
    usize::from(unsafe { dsound_hle_register_rust(hrtf::ENTRY) })
}

/// `dsound_hrtf_register`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hrtf_register() -> u32 {
    tsfp_audio_hrtf_register_rust() as u32
}
