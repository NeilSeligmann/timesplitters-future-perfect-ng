//! Rust-host publication of XMV movie packet completion words.
use crate::effects_guest_copy_ffi::tsfp_effects_guest_copy_write_rust;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// Host boundary used to enter a guest completion callback after Rust publishes its words.
pub type MovieCallbackRunner = unsafe extern "C" fn(
    callback: u32,
    stream_context: u32,
    packet_context: u32,
    status: u32,
) -> bool;

/// Host services that preserve the movie-object lock boundary around completion publication.
pub type MovieCompletionBeforeUnlock = unsafe extern "C" fn(size: u32, status: u32, context: u32);
pub type MovieCompletionUnlock = unsafe extern "C" fn();

static MOVIE_CALLBACK_RUNNER: Mutex<Option<MovieCallbackRunner>> = Mutex::new(None);
static MOVIE_COMPLETIONS: AtomicU64 = AtomicU64::new(0);

/// Record and read Rust-host movie completions without routing diagnostic state through C.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_completion_count_rust() -> u64 {
    MOVIE_COMPLETIONS.load(Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_completion_count_reset_rust() {
    MOVIE_COMPLETIONS.store(0, Ordering::Relaxed);
}

pub(crate) fn movie_completion_count_increment_rust() {
    MOVIE_COMPLETIONS.fetch_add(1, Ordering::Relaxed);
}

/// Install the process-lifetime synchronous guest callback runner for the Rust audio host.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_completion_set_runner_rust(
    runner: Option<MovieCallbackRunner>,
) -> bool {
    let Ok(mut registered) = MOVIE_CALLBACK_RUNNER.lock() else {
        return false;
    };
    *registered = runner;
    true
}

/// Probe one XMV completion word for writability without changing its guest-visible value.
///
/// # Safety
/// `address` must belong to the process identified by `host_pid`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_movie_completion_word_probe_rust(
    host_pid: i32,
    address: u32,
) -> bool {
    if host_pid <= 0 || address == 0 || u64::from(address) + 4 > (1u64 << 32) {
        return false;
    }
    let mut old = [0u8; 4];
    // SAFETY: the guest-copy boundary faults safely on unreadable spans.
    if !unsafe {
        crate::effects_guest_copy_ffi::tsfp_effects_guest_copy_read_rust(
            host_pid,
            address,
            old.as_mut_ptr(),
            old.len(),
        )
    } {
        return false;
    }
    // SAFETY: write back the exact bytes just read to verify guest writability without mutation.
    unsafe { tsfp_effects_guest_copy_write_rust(host_pid, old.as_ptr(), address, old.len()) }
}

/// Publish a movie Process packet's initial completed-size and pending-status words.
/// The writes remain separate and ordered, matching the C facade's short-circuit behavior.
///
/// # Safety
/// Both guest spans must belong to the process identified by `host_pid`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_movie_pending_publish_rust(
    host_pid: i32,
    completed: u32,
    status: u32,
) -> bool {
    if host_pid <= 0 {
        return false;
    }
    let size_bytes = 0u32.to_le_bytes();
    // SAFETY: the process_vm boundary validates the destination and refuses inaccessible pages.
    if !unsafe {
        tsfp_effects_guest_copy_write_rust(
            host_pid,
            size_bytes.as_ptr(),
            completed,
            size_bytes.len(),
        )
    } {
        return false;
    }
    let status_bytes = tsfp_audio::movie::PENDING_STATUS.to_le_bytes();
    // SAFETY: the process_vm boundary validates the destination and refuses inaccessible pages.
    unsafe {
        tsfp_effects_guest_copy_write_rust(
            host_pid,
            status_bytes.as_ptr(),
            status,
            status_bytes.len(),
        )
    }
}

/// Write the completed size, then status, stopping after the first refusal as the movie C facade does.
///
/// # Safety
/// Both guest spans must belong to the process identified by `host_pid`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_movie_completion_publish_rust(
    host_pid: i32,
    completed: u32,
    size: u32,
    status: u32,
    status_value: u32,
) -> bool {
    if host_pid <= 0 {
        return false;
    }
    let size_bytes = size.to_le_bytes();
    // SAFETY: the process_vm boundary validates the destination and refuses inaccessible pages.
    if !unsafe {
        tsfp_effects_guest_copy_write_rust(
            host_pid,
            size_bytes.as_ptr(),
            completed,
            size_bytes.len(),
        )
    } {
        return false;
    }
    let status_bytes = status_value.to_le_bytes();
    // SAFETY: the process_vm boundary validates the destination and refuses inaccessible pages.
    unsafe {
        tsfp_effects_guest_copy_write_rust(
            host_pid,
            status_bytes.as_ptr(),
            status,
            status_bytes.len(),
        )
    }
}

/// Enter a guest completion callback through the host boundary after Rust publishes its words.
///
/// The caller publishes size and status first, then releases its object lock before calling this
/// function. The current hybrid host supplies its guest dispatcher; a Rust-only host can supply
/// its native dispatcher through the same contract.
///
/// # Safety
/// The registered runner must remain callable for the process lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_movie_completion_call_guest_rust(
    callback: u32,
    stream_context: u32,
    packet_context: u32,
    status: u32,
) -> bool {
    // Copy the function pointer and release the registration lock before guest code can re-enter.
    let Ok(registered) = MOVIE_CALLBACK_RUNNER.lock() else {
        return false;
    };
    let runner = *registered;
    drop(registered);
    let Some(runner) = runner else {
        return false;
    };
    // SAFETY: the host guarantees a supplied callback runner is callable.
    unsafe { runner(callback, stream_context, packet_context, status) }
}

/// Publish a packet completion, run the host's locked bookkeeping, unlock the movie object,
/// then synchronously enter the registered guest callback.
///
/// The hooks are required host services: `before_unlock` runs even when publication is refused,
/// matching the C facade's completion counter/trace order; `unlock` runs exactly once before any
/// guest callback can re-enter movie audio. The callback runner is looked up only after unlocking.
///
/// # Safety
/// The guest spans belong to `host_pid`; both hooks are valid for this call; the registered
/// callback runner remains callable for the process lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_movie_completion_deliver_rust(
    host_pid: i32,
    completed: u32,
    size: u32,
    status_address: u32,
    status: u32,
    before_unlock: MovieCompletionBeforeUnlock,
    unlock: MovieCompletionUnlock,
    callback: u32,
    stream_context: u32,
    packet_context: u32,
) -> bool {
    // SAFETY: the caller supplies both guest spans in its process and holds the movie-object lock.
    let published = unsafe {
        tsfp_audio_movie_completion_publish_rust(host_pid, completed, size, status_address, status)
    };
    // SAFETY: the host requires bookkeeping and mutex release even on publication failure.
    unsafe { before_unlock(size, status, packet_context) };
    // SAFETY: this releases the caller-held movie-object lock exactly once.
    unsafe { unlock() };
    if !published {
        return false;
    }
    // SAFETY: publication is complete and the object lock is released before guest re-entry.
    unsafe {
        tsfp_audio_movie_completion_call_guest_rust(
            callback,
            stream_context,
            packet_context,
            status,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static CALLBACK_HOOK_STATE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    #[test]
    fn rust_host_completion_count_is_readable_and_resettable() {
        tsfp_audio_movie_completion_count_reset_rust();
        movie_completion_count_increment_rust();
        assert_eq!(tsfp_audio_movie_completion_count_rust(), 1);
        tsfp_audio_movie_completion_count_reset_rust();
        assert_eq!(tsfp_audio_movie_completion_count_rust(), 0);
    }

    unsafe extern "C" fn callback_observes_published_words(
        callback: u32,
        stream_context: u32,
        packet_context: u32,
        status: u32,
    ) -> bool {
        if callback != 0x0044_5071
            || status != 0x8000_4004
            || CALLBACK_HOOK_STATE.load(std::sync::atomic::Ordering::SeqCst) != 3
        {
            return false;
        }
        // SAFETY: the test passes live addresses in its mapped page as callback contexts.
        let size = unsafe { (stream_context as *const u32).read_unaligned() };
        // SAFETY: the test passes live addresses in its mapped page as callback contexts.
        let published_status = unsafe { (packet_context as *const u32).read_unaligned() };
        size == 0x1234_5678
            && published_status == status
            && tsfp_audio_movie_completion_set_runner_rust(None)
    }

    #[test]
    fn completion_word_probe_preserves_unaligned_guest_bytes_and_refuses_unmapped_words() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        let word = base + 0x301;
        // SAFETY: this byte span is within the writable page owned by the test.
        unsafe { (word as *mut u8).copy_from_nonoverlapping([0x13, 0x57, 0x9B, 0xDF].as_ptr(), 4) };
        // SAFETY: the test page is the identity-mapped guest process supplied below.
        assert!(unsafe {
            tsfp_audio_movie_completion_word_probe_rust(std::process::id() as i32, word)
        });
        // SAFETY: the four bytes remain within the still-mapped test page.
        unsafe {
            assert_eq!(
                std::slice::from_raw_parts(word as *const u8, 4),
                &[0x13, 0x57, 0x9B, 0xDF]
            )
        };
        // SAFETY: process_vm safely refuses the unmapped final dword.
        assert!(!unsafe {
            tsfp_audio_movie_completion_word_probe_rust(std::process::id() as i32, 0xFFFF_FFFC)
        });
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn publishes_unaligned_initial_size_then_pending_status() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        let completed = base + 0x301;
        let status = base + 0x327;
        // SAFETY: the test owns a writable page containing both guest output spans.
        unsafe { (base as *mut u8).write_bytes(0xCC, 0x400) };
        // SAFETY: the test's low-address page remains mapped for the call.
        assert!(unsafe {
            tsfp_audio_movie_pending_publish_rust(std::process::id() as i32, completed, status)
        });
        // SAFETY: both unaligned dwords are in the owned writable page.
        unsafe {
            assert_eq!((completed as *const u32).read_unaligned(), 0);
            assert_eq!(
                (status as *const u32).read_unaligned(),
                tsfp_audio::movie::PENDING_STATUS
            );
        }
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn failed_initial_size_write_leaves_pending_status_untouched() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        let status = base + 0x320;
        // SAFETY: status is writable in the mapped low-address page.
        unsafe { (status as *mut u32).write_unaligned(0xAABB_CCDD) };
        // SAFETY: process_vm safely refuses the unmapped completion destination.
        assert!(!unsafe {
            tsfp_audio_movie_pending_publish_rust(std::process::id() as i32, 0xFFFF_FFFC, status)
        });
        // SAFETY: the writable status word remains mapped and untouched after short circuit.
        assert_eq!(
            unsafe { (status as *const u32).read_unaligned() },
            0xAABB_CCDD
        );
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn status_refusal_keeps_initial_zero_size_published() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        let completed = base + 0x300;
        // SAFETY: the completion word is writable in the low-address page.
        unsafe { (completed as *mut u32).write_unaligned(0xAABB_CCDD) };
        // SAFETY: process_vm writes the first span and safely refuses the unmapped status span.
        assert!(!unsafe {
            tsfp_audio_movie_pending_publish_rust(std::process::id() as i32, completed, 0xFFFF_FFFC)
        });
        // SAFETY: the completion word remains mapped after the second-span refusal.
        assert_eq!(unsafe { (completed as *const u32).read_unaligned() }, 0);
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn publishes_unaligned_size_then_status() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        let completed = base + 0x301;
        let status = base + 0x327;
        // SAFETY: the test owns a writable page containing both guest output spans.
        unsafe { (base as *mut u8).write_bytes(0xCC, 0x400) };
        // SAFETY: the test's low-address page remains mapped for the call.
        assert!(unsafe {
            tsfp_audio_movie_completion_publish_rust(
                std::process::id() as i32,
                completed,
                0x1234_5678,
                status,
                0x8000_4004,
            )
        });
        // SAFETY: both guest words remain mapped; unaligned reads match the ABI's byte writes.
        unsafe {
            assert_eq!((completed as *const u32).read_unaligned(), 0x1234_5678);
            assert_eq!((status as *const u32).read_unaligned(), 0x8000_4004);
        }
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn callback_runner_receives_completion_context() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        let completed = base + 0x301;
        let status_address = base + 0x327;
        CALLBACK_HOOK_STATE.store(0, std::sync::atomic::Ordering::SeqCst);
        assert!(tsfp_audio_movie_completion_set_runner_rust(Some(
            callback_observes_published_words,
        )));
        unsafe extern "C" fn before_unlock(size: u32, status: u32, _: u32) {
            if size == 0x1234_5678 && status == 0x8000_4004 {
                CALLBACK_HOOK_STATE.store(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
        unsafe extern "C" fn unlock() {
            CALLBACK_HOOK_STATE.fetch_or(2, std::sync::atomic::Ordering::SeqCst);
        }
        // SAFETY: the test owns both words and supplies valid no-op lock hooks.
        let delivered = unsafe {
            tsfp_audio_movie_completion_deliver_rust(
                std::process::id() as i32,
                completed,
                0x1234_5678,
                status_address,
                0x8000_4004,
                before_unlock,
                unlock,
                0x0044_5071,
                completed,
                status_address,
            )
        };
        assert!(delivered);
        assert_eq!(
            CALLBACK_HOOK_STATE.load(std::sync::atomic::Ordering::SeqCst),
            3
        );
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn failed_size_write_does_not_attempt_status_write() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        let status = base + 0x320;
        // SAFETY: status is writable in the mapped low-address page.
        unsafe { (status as *mut u32).write_unaligned(0xAABB_CCDD) };
        // SAFETY: process_vm safely refuses the unmapped completion destination.
        assert!(!unsafe {
            tsfp_audio_movie_completion_publish_rust(
                std::process::id() as i32,
                0xFFFF_FFFC,
                0x1122_3344,
                status,
                0x8000_4004,
            )
        });
        // SAFETY: the writable status word remains mapped and untouched after short circuit.
        assert_eq!(
            unsafe { (status as *const u32).read_unaligned() },
            0xAABB_CCDD
        );
        crate::test_guest_memory::unmap_guest_page(page);
    }

    #[test]
    fn status_write_refusal_keeps_already_published_size() {
        let (page, base) = crate::test_guest_memory::map_guest_page();
        let completed = base + 0x300;
        // SAFETY: the completion word is writable in the mapped low-address page.
        unsafe { (completed as *mut u32).write_unaligned(0xAABB_CCDD) };
        // SAFETY: process_vm writes the first span and safely refuses the unmapped status span.
        assert!(!unsafe {
            tsfp_audio_movie_completion_publish_rust(
                std::process::id() as i32,
                completed,
                0x1122_3344,
                0xFFFF_FFFC,
                0x8000_4004,
            )
        });
        // SAFETY: the completion word remains mapped after the second-span refusal.
        assert_eq!(
            unsafe { (completed as *const u32).read_unaligned() },
            0x1122_3344
        );
        crate::test_guest_memory::unmap_guest_page(page);
    }
}
