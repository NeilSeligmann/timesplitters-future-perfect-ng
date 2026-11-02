//! SetFrequency access of the stream adapter (the Rust-host branch of `access_owned` in
//! dsound_stream.c): admission, old/new rate resolution, pitch domain and the commit.
//! The caller holds the device ownership and the adapter lock.

use crate::FrequencyState;

/// Shape of the stream as the C adapter reads it from the lease records.
pub struct Request {
    pub stream: u32,
    pub serial: u64,
    pub scope_flags: u32,
    pub scope_channels: u16,
    pub scope_rate: u32,
    pub format_sets: u32,
    pub format_bytes: [u8; 20],
    pub cache_mask: u32,
    pub volume_seen: bool,
    pub discontinuity_seen: bool,
    pub completion: bool,
    pub note_installed: bool,
    pub hertz: u32,
    pub control_word: u16,
}

/// The Rust-owned frequency transaction and its admission/pitch rules.
pub trait Hooks {
    fn state(&self, stream: u32, serial: u64) -> Option<FrequencyState>;
    fn admit(&self, stereo: bool, spatial: bool, r: &Request) -> bool;
    fn pitch(&self, hertz: u32, control_word: u16) -> Option<i32>;
    /// Commits at `accepted_sets`/`old_hz`; runs the synchronous completion adapter.
    fn commit(&self, r: &Request, accepted_sets: u32, old_hz: u32, new_hz: u32) -> bool;
}

/// `Err` carries the C adapter's `access_request.error` text.
pub fn set_frequency<H: Hooks>(h: &H, r: &Request) -> Result<(), &'static str> {
    let state = h
        .state(r.stream, r.serial)
        .ok_or("Rust frequency sidecar is unavailable for this lease")?;
    let format_rate = if r.format_sets != 0 {
        u32::from_le_bytes([
            r.format_bytes[4],
            r.format_bytes[5],
            r.format_bytes[6],
            r.format_bytes[7],
        ])
    } else {
        r.scope_rate
    };
    let old_hz = if state.accepted_sets == 0 || state.current_hz == 0 {
        format_rate
    } else {
        state.current_hz
    };
    let stereo = r.scope_flags == 0 && r.scope_channels == 2 && r.cache_mask == 0;
    let spatial = r.scope_flags == 0x10 && r.scope_channels == 1 && r.cache_mask == 7;
    if !h.admit(stereo, spatial, r) {
        return Err("frequency needs a started stereo or fully configured mono PCM stream");
    }
    let hertz = if r.hertz == 0 { format_rate } else { r.hertz };
    if h.pitch(hertz, r.control_word).is_none() {
        return Err("frequency or guest control word outside measured domain");
    }
    if state.accepted_sets != u32::MAX && h.commit(r, state.accepted_sets, old_hz, hertz) {
        Ok(())
    } else {
        Err("frequency PCM/completion transaction refused")
    }
}

#[cfg(not(test))]
mod exports {
    use super::*;
    use std::ffi::{c_char, c_void};

    type Apply = unsafe extern "C" fn(*mut c_void, u32, u64, u32, u32) -> bool;

    unsafe extern "C" {
        fn tsfp_audio_stream_frequency_state_rust(
            stream: u32,
            serial: u64,
            out: *mut FrequencyState,
        ) -> bool;
        fn tsfp_audio_stream_frequency_admit_rust(
            stereo: u32,
            spatial: u32,
            completion: u32,
            handler: u32,
            volume_seen: u32,
            discontinuity_seen: u32,
        ) -> bool;
        fn tsfp_audio_stream_frequency_pitch_rust(hertz: u32, cw: u16, out: *mut i32) -> bool;
        fn tsfp_audio_stream_frequency_commit_rust(
            stream: u32,
            serial: u64,
            accepted_sets: u32,
            old_hz: u32,
            new_hz: u32,
            control_word: u16,
            context: *mut c_void,
            apply: Option<Apply>,
            pitch_out: *mut i32,
        ) -> bool;
    }

    struct Exports {
        context: *mut c_void,
        apply: Apply,
    }

    impl Hooks for Exports {
        fn state(&self, stream: u32, serial: u64) -> Option<FrequencyState> {
            let mut s = FrequencyState::default();
            unsafe { tsfp_audio_stream_frequency_state_rust(stream, serial, &mut s) }.then_some(s)
        }
        fn admit(&self, stereo: bool, spatial: bool, r: &Request) -> bool {
            unsafe {
                tsfp_audio_stream_frequency_admit_rust(
                    stereo.into(),
                    spatial.into(),
                    r.completion.into(),
                    r.note_installed.into(),
                    r.volume_seen.into(),
                    r.discontinuity_seen.into(),
                )
            }
        }
        fn pitch(&self, hertz: u32, control_word: u16) -> Option<i32> {
            let mut pitch = 0;
            unsafe { tsfp_audio_stream_frequency_pitch_rust(hertz, control_word, &mut pitch) }
                .then_some(pitch)
        }
        fn commit(&self, r: &Request, accepted_sets: u32, old_hz: u32, new_hz: u32) -> bool {
            let mut pitch = 0;
            unsafe {
                tsfp_audio_stream_frequency_commit_rust(
                    r.stream,
                    r.serial,
                    accepted_sets,
                    old_hz,
                    new_hz,
                    r.control_word,
                    self.context,
                    Some(self.apply),
                    &mut pitch,
                )
            }
        }
    }

    /// Runs SetFrequency for one owned stream. Returns true when accepted; otherwise `*error`
    /// names the refusal. `context`/`apply` are the C completion adapter's note callback.
    ///
    /// # Safety
    /// `format_bytes` points to 20 readable bytes, `error` is writable, `context` stays valid
    /// through `apply` (the caller holds the device ownership and the adapter lock).
    #[unsafe(no_mangle)]
    #[allow(clippy::too_many_arguments)]
    pub unsafe extern "C" fn tsfp_audio_stream_set_frequency_rust(
        stream: u32,
        serial: u64,
        scope_flags: u32,
        scope_channels: u32,
        scope_rate: u32,
        format_sets: u32,
        format_bytes: *const u8,
        cache_mask: u32,
        flags: u32,
        hertz: u32,
        control_word: u32,
        context: *mut c_void,
        apply: Apply,
        error: *mut *const c_char,
    ) -> bool {
        let mut bytes = [0u8; 20];
        unsafe { std::ptr::copy_nonoverlapping(format_bytes, bytes.as_mut_ptr(), 20) };
        let r = Request {
            stream,
            serial,
            scope_flags,
            scope_channels: scope_channels as u16,
            scope_rate,
            format_sets,
            format_bytes: bytes,
            cache_mask,
            volume_seen: flags & 1 != 0,
            discontinuity_seen: flags & 2 != 0,
            completion: flags & 4 != 0,
            note_installed: flags & 8 != 0,
            hertz,
            control_word: control_word as u16,
        };
        match set_frequency(&Exports { context, apply }, &r) {
            Ok(()) => true,
            Err(text) => {
                unsafe { error.write(crate::observe::error_text(text)) };
                false
            }
        }
    }
}

#[cfg(not(test))]
pub use exports::tsfp_audio_stream_set_frequency_rust;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Fake {
        state: Option<FrequencyState>,
        admitted: bool,
        pitch: Option<i32>,
        commits: Cell<Vec<(u32, u32, u32)>>,
        commit_ok: bool,
    }

    impl Fake {
        fn new() -> Self {
            Fake {
                state: Some(FrequencyState::default()),
                admitted: true,
                pitch: Some(0),
                commits: Cell::new(Vec::new()),
                commit_ok: true,
            }
        }
    }

    impl Hooks for Fake {
        fn state(&self, _: u32, _: u64) -> Option<FrequencyState> {
            self.state
        }
        fn admit(&self, stereo: bool, spatial: bool, _: &Request) -> bool {
            self.admitted && (stereo || spatial)
        }
        fn pitch(&self, _: u32, _: u16) -> Option<i32> {
            self.pitch
        }
        fn commit(&self, _: &Request, sets: u32, old: u32, new: u32) -> bool {
            let mut v = self.commits.take();
            v.push((sets, old, new));
            self.commits.set(v);
            self.commit_ok
        }
    }

    fn request() -> Request {
        let mut format_bytes = [0u8; 20];
        format_bytes[4..8].copy_from_slice(&22050u32.to_le_bytes());
        Request {
            stream: 1,
            serial: 2,
            scope_flags: 0,
            scope_channels: 2,
            scope_rate: 48000,
            format_sets: 0,
            format_bytes,
            cache_mask: 0,
            volume_seen: true,
            discontinuity_seen: true,
            completion: true,
            note_installed: true,
            hertz: 32000,
            control_word: 0x27f,
        }
    }

    #[test]
    fn commits_with_scope_rate_as_old_rate_and_ignores_unset_state() {
        let f = Fake::new();
        assert_eq!(set_frequency(&f, &request()), Ok(()));
        assert_eq!(f.commits.take(), vec![(0, 48000, 32000)]);
    }

    #[test]
    fn zero_hertz_uses_format_rate_and_state_rate_wins_for_old() {
        let mut r = request();
        r.hertz = 0;
        r.format_sets = 1;
        let mut f = Fake::new();
        f.state = Some(FrequencyState {
            accepted_sets: 3,
            current_hz: 11025,
            pitch: 0,
        });
        assert_eq!(set_frequency(&f, &r), Ok(()));
        assert_eq!(f.commits.take(), vec![(3, 11025, 22050)]);
    }

    #[test]
    fn spatial_shape_is_admitted_and_other_shapes_are_not() {
        let mut r = request();
        r.scope_flags = 0x10;
        r.scope_channels = 1;
        r.cache_mask = 7;
        assert_eq!(set_frequency(&Fake::new(), &r), Ok(()));
        r.cache_mask = 3;
        assert_eq!(
            set_frequency(&Fake::new(), &r),
            Err("frequency needs a started stereo or fully configured mono PCM stream")
        );
    }

    #[test]
    fn refusals_carry_the_c_texts_and_skip_the_commit() {
        let mut f = Fake::new();
        f.state = None;
        assert!(
            set_frequency(&f, &request())
                .unwrap_err()
                .contains("sidecar")
        );
        let mut f = Fake::new();
        f.pitch = None;
        assert!(
            set_frequency(&f, &request())
                .unwrap_err()
                .contains("measured domain")
        );
        assert!(f.commits.take().is_empty());
        let mut f = Fake::new();
        f.commit_ok = false;
        assert!(
            set_frequency(&f, &request())
                .unwrap_err()
                .contains("transaction refused")
        );
        let mut f = Fake::new();
        f.state = Some(FrequencyState {
            accepted_sets: u32::MAX,
            current_hz: 0,
            pitch: 0,
        });
        assert!(set_frequency(&f, &request()).is_err());
        assert!(f.commits.take().is_empty());
    }
}
