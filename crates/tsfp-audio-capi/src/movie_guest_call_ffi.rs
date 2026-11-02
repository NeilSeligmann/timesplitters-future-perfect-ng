//! Rust-owned stdcall trampoline for XMV guest completion callbacks.

/// Lifted guest function selected by the host's address resolver.
pub type MovieGuestFunction = unsafe extern "C" fn();

/// Small host boundary for the current guest-thread register file and lifted-function lookup.
///
/// The Rust trampoline owns frame layout, call ordering, guest-stack writing, balance checking,
/// and register restore. The host supplies only lifted-function lookup and thread-local register
/// accessors. These accessors must remain valid for the synchronous call.
#[repr(C)]
pub struct MovieGuestCallHost {
    pub host_pid: i32,
    pub lookup: Option<unsafe extern "C" fn(u32) -> Option<MovieGuestFunction>>,
    pub read_registers: Option<unsafe extern "C" fn(*mut u32) -> bool>,
    pub set_esp: Option<unsafe extern "C" fn(u32) -> bool>,
    pub get_esp: Option<unsafe extern "C" fn() -> u32>,
    pub restore_registers: Option<unsafe extern "C" fn(*const u32) -> bool>,
}

/// Run a lifted stdcall function on the current guest stack with up to four integer arguments.
///
/// This preserves the T392 call contract: the complete `[return=0, args...]` frame is written
/// before changing ESP, EAX..EBP and ESP are restored on return, and success requires the guest
/// callee to pop exactly that frame. FPU/SSE state remains the callback's responsibility.
///
/// # Safety
/// `arguments` must contain `count` words; `host` and its callbacks must remain valid through the
/// synchronous guest call. `host_pid` must identify the process owning the guest stack.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_guest_call_stdcall_rust(
    address: u32,
    arguments: *const u32,
    count: u32,
    host: *const MovieGuestCallHost,
) -> bool {
    const MAX_ARGS: usize = 4;
    if count as usize > MAX_ARGS || (count != 0 && arguments.is_null()) || host.is_null() {
        return false;
    }
    // SAFETY: guaranteed by the caller for this synchronous host boundary.
    let host = unsafe { &*host };
    let (Some(lookup), Some(read_registers), Some(set_esp), Some(get_esp), Some(restore_registers)) = (
        host.lookup,
        host.read_registers,
        host.set_esp,
        host.get_esp,
        host.restore_registers,
    ) else {
        return false;
    };
    // Resolve before touching the guest stack, matching the C oracle.
    // SAFETY: the host promises lookup is callable for this process lifetime.
    let Some(function) = (unsafe { lookup(address) }) else {
        return false;
    };

    let mut saved = [0u32; 8];
    // SAFETY: host register accessors operate on the current guest thread.
    if !unsafe { read_registers(saved.as_mut_ptr()) } {
        return false;
    }
    let top = saved[7];
    let frame_bytes = 4usize * (count as usize + 1);
    let Some(base) = top.checked_sub(frame_bytes as u32) else {
        return false;
    };
    let mut frame = [0u32; MAX_ARGS + 1];
    if count != 0 {
        // SAFETY: checked non-null above and the ABI supplies `count` argument words.
        let args = unsafe { core::slice::from_raw_parts(arguments, count as usize) };
        frame[1..=count as usize].copy_from_slice(args);
    }
    // SAFETY: FrameGuestMemory delegates to the fault-safe process-vm adapter for this guest.
    let frame_bytes =
        unsafe { core::slice::from_raw_parts(frame.as_ptr().cast::<u8>(), frame_bytes) };
    if !crate::frame_guest_memory::FrameGuestMemory::new(host.host_pid)
        .write_bytes(base, frame_bytes)
    {
        return false;
    }
    // SAFETY: the host setter targets the current guest thread's ESP.
    if !unsafe { set_esp(base) } {
        // SAFETY: restore the complete snapshot if the setter refused after partial work.
        let _ = unsafe { restore_registers(saved.as_ptr()) };
        return false;
    }
    // SAFETY: the resolver returned a live lifted function for this process.
    unsafe { function() };
    // Read balance before restoring the interrupted guest register file.
    // SAFETY: the host getter reads current guest-thread ESP.
    let balanced = unsafe { get_esp() } == top;
    // SAFETY: restore EAX..EBP and ESP exactly as the C oracle does.
    let restored = unsafe { restore_registers(saved.as_ptr()) };
    balanced && restored
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    thread_local! {
        static REGISTERS: Cell<[u32; 8]> = const { Cell::new([0; 8]) };
        static PAGE_BASE: Cell<u32> = const { Cell::new(0) };
        static OBSERVED_FRAME: Cell<[u32; 4]> = const { Cell::new([0; 4]) };
        static UNBALANCED: Cell<bool> = const { Cell::new(false) };
    }

    unsafe extern "C" fn callback() {
        let esp = REGISTERS.with(|registers| registers.get()[7]);
        // SAFETY: the test host wrote this four-word frame into its mapped page.
        let frame = unsafe { (esp as *const u32).cast::<[u32; 4]>().read_unaligned() };
        OBSERVED_FRAME.with(|observed| observed.set(frame));
        REGISTERS.with(|registers| {
            let mut value = registers.get();
            value[0] = 0xfeed_beef;
            value[1] = 0xcafe_babe;
            value[7] = esp + if UNBALANCED.with(Cell::get) { 12 } else { 16 };
            registers.set(value);
        });
    }

    unsafe extern "C" fn lookup(address: u32) -> Option<MovieGuestFunction> {
        (address == 0x0044_5071).then_some(callback)
    }

    unsafe extern "C" fn read_registers(out: *mut u32) -> bool {
        if out.is_null() {
            return false;
        }
        REGISTERS.with(|registers| {
            // SAFETY: the trampoline supplies eight writable u32 slots.
            unsafe { core::ptr::copy_nonoverlapping(registers.get().as_ptr(), out, 8) };
        });
        true
    }

    unsafe extern "C" fn set_esp(esp: u32) -> bool {
        REGISTERS.with(|registers| {
            let mut value = registers.get();
            value[7] = esp;
            registers.set(value);
        });
        true
    }

    unsafe extern "C" fn get_esp() -> u32 {
        REGISTERS.with(|registers| registers.get()[7])
    }

    unsafe extern "C" fn restore_registers(saved: *const u32) -> bool {
        if saved.is_null() {
            return false;
        }
        // SAFETY: the trampoline supplies its eight-word snapshot.
        let registers = unsafe { (saved.cast::<[u32; 8]>()).read_unaligned() };
        REGISTERS.with(|current| current.set(registers));
        true
    }

    fn host() -> MovieGuestCallHost {
        MovieGuestCallHost {
            host_pid: std::process::id() as i32,
            lookup: Some(lookup),
            read_registers: Some(read_registers),
            set_esp: Some(set_esp),
            get_esp: Some(get_esp),
            restore_registers: Some(restore_registers),
        }
    }

    #[test]
    fn stdcall_trampoline_writes_frame_restores_registers_and_checks_callee_pop() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        PAGE_BASE.with(|page_base| page_base.set(base));
        let saved = [1, 2, 3, 4, 5, 6, 7, base + 0x500];
        REGISTERS.with(|registers| registers.set(saved));
        UNBALANCED.with(|unbalanced| unbalanced.set(false));
        OBSERVED_FRAME.with(|frame| frame.set([0; 4]));
        let args = [0x1111, 0x2222, 0x3333];
        let host = host();
        // SAFETY: the mapped page, arguments, host hooks, and callback live through the call.
        let called =
            unsafe { tsfp_audio_guest_call_stdcall_rust(0x0044_5071, args.as_ptr(), 3, &host) };
        assert!(called);
        assert_eq!(
            OBSERVED_FRAME.with(Cell::get),
            [0, args[0], args[1], args[2]]
        );
        assert_eq!(REGISTERS.with(Cell::get), saved);
        // SAFETY: the callback frame remains inside the test's mapped page.
        let stack_frame = unsafe { core::slice::from_raw_parts((base + 0x4f0) as *const u32, 4) };
        assert_eq!(stack_frame, [0, args[0], args[1], args[2]]);
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn stdcall_trampoline_restores_registers_and_refuses_wrong_callee_pop() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        PAGE_BASE.with(|page_base| page_base.set(base));
        let saved = [8, 7, 6, 5, 4, 3, 2, base + 0x500];
        REGISTERS.with(|registers| registers.set(saved));
        UNBALANCED.with(|unbalanced| unbalanced.set(true));
        let args = [0xaaaa, 0xbbbb, 0xcccc];
        let host = host();
        // SAFETY: the mapped page, arguments, host hooks, and callback live through the call.
        let called =
            unsafe { tsfp_audio_guest_call_stdcall_rust(0x0044_5071, args.as_ptr(), 3, &host) };
        assert!(!called);
        assert_eq!(REGISTERS.with(Cell::get), saved);
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn stdcall_trampoline_refuses_missing_target_and_excess_arguments_without_state_change() {
        let saved = [9, 8, 7, 6, 5, 4, 3, 0x1000];
        REGISTERS.with(|registers| registers.set(saved));
        let host = host();
        // SAFETY: host hooks and argument arrays are valid for the calls.
        assert!(!unsafe {
            tsfp_audio_guest_call_stdcall_rust(0xdead_beef, core::ptr::null(), 0, &host)
        });
        let args = [0u32; 5];
        // SAFETY: five words are provided; the ABI limit rejects before reading or changing state.
        assert!(!unsafe {
            tsfp_audio_guest_call_stdcall_rust(0x0044_5071, args.as_ptr(), 5, &host)
        });
        assert_eq!(REGISTERS.with(Cell::get), saved);
    }

    #[test]
    fn stdcall_trampoline_refuses_an_unwritable_guest_stack_before_callback() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        PAGE_BASE.with(|page_base| page_base.set(base));
        let saved = [9, 8, 7, 6, 5, 4, 3, base + 0x500];
        REGISTERS.with(|registers| registers.set(saved));
        OBSERVED_FRAME.with(|frame| frame.set([0; 4]));
        let args = [0xaaaa, 0xbbbb, 0xcccc];
        let mut host = host();
        host.host_pid = i32::MAX;
        // SAFETY: the function and register hooks remain live; the invalid process id refuses
        // the guest write before ESP changes or the callback runs.
        let called =
            unsafe { tsfp_audio_guest_call_stdcall_rust(0x0044_5071, args.as_ptr(), 3, &host) };
        assert!(!called);
        assert_eq!(REGISTERS.with(Cell::get), saved);
        assert_eq!(OBSERVED_FRAME.with(Cell::get), [0; 4]);
        crate::test_guest_memory::unmap_guest_page(page);
    }
}
