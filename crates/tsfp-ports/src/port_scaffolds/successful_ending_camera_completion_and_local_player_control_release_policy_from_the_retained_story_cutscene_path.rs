// SPDX-License-Identifier: GPL-3.0-or-later
//! T2488: typed successful-ending camera completion and control restoration.
//!
//! Static contract only. The source route has not been reached in a runtime trace; the caller
//! must keep this selector off until the owner-recording replay and 180-second gate pass.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallbackHandle(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BindingHandle(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerContext {
    pub camera: Option<CameraId>,
    pub cinematic_flags: u32,
    pub finish_callback: Option<CallbackHandle>,
    pub update_callback: Option<CallbackHandle>,
    /// Saved bindings supplied by the camera and inventory owners.
    pub camera_binding: Option<BindingHandle>,
    pub inventory_binding: Option<BindingHandle>,
    pub mode: u32,
    pub timer: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryMission {
    pub segment_index: u32,
    pub segment_count: u32,
    pub first_segment_flags: u32,
    pub attract_mode: bool,
    pub active_camera: Option<CameraId>,
    pub local_player: Option<PlayerContext>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseOutcome {
    /// The typed contract was incomplete; retain the lifted path as authority.
    Unsupported,
    /// One or more measured guards did not permit release yet.
    NotReady,
    /// The measured release policy ran for the matching local camera context.
    Released,
}

/// Backend boundary for effects owned by camera, inventory, callback and input systems.
pub trait StoryControlReleaseBackend {
    fn invoke_finish(&mut self, callback: CallbackHandle);
    fn restore_camera_binding(&mut self, binding: Option<BindingHandle>);
    fn restore_inventory_binding(&mut self, binding: Option<BindingHandle>);
}

/// Runs the measured final-segment guard and ordered local-context cleanup.
///
/// Returns `Unsupported` if the active camera or typed local context is unavailable. A context
/// without a camera or with another camera is simply not the matching context and is left alone.
pub fn release_successful_ending<B: StoryControlReleaseBackend>(
    mission: &mut StoryMission,
    backend: &mut B,
) -> ReleaseOutcome {
    if mission.segment_index < mission.segment_count
        || mission.first_segment_flags & 1 != 0
        || mission.attract_mode
    {
        return ReleaseOutcome::NotReady;
    }
    let Some(active_camera) = mission.active_camera else {
        return ReleaseOutcome::Unsupported;
    };
    let Some(context) = mission.local_player.as_mut() else {
        return ReleaseOutcome::Unsupported;
    };
    if context.camera != Some(active_camera) {
        return ReleaseOutcome::NotReady;
    }

    context.cinematic_flags &= !0x0200_0000;
    if let Some(callback) = context.finish_callback {
        backend.invoke_finish(callback);
        context.finish_callback = None;
    }
    context.update_callback = None;
    backend.restore_camera_binding(context.camera_binding);
    backend.restore_inventory_binding(context.inventory_binding);
    if context.mode != 0x10 {
        context.mode = 2;
        context.timer = 0;
    }
    ReleaseOutcome::Released
}

/// Candidate dispatch roots are retained for trace routing; no raw guest state is read here.
pub const LANE_ROOT_CANDIDATES: &[u32] = &[0x000C8640, 0x000C8920, 0x000D4BE0];

pub fn lane_root_selected(va: u32) -> bool {
    LANE_ROOT_CANDIDATES.contains(&va)
}

fn enabled_from(value: Option<&[u8]>) -> bool {
    value == Some(b"rust")
}

/// Default-off A/B switch. `TSFP_STORY_CONTROL_RELEASE=rust` opts into the typed policy.
pub fn enabled() -> bool {
    #[cfg(not(test))]
    unsafe {
        unsafe extern "C" {
            fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
        }
        let value = getenv(c"TSFP_STORY_CONTROL_RELEASE".as_ptr());
        !value.is_null() && enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
    #[cfg(test)]
    {
        false
    }
}

/// `None` means use the retained lifted implementation.
pub fn selected_release<B: StoryControlReleaseBackend>(
    mission: &mut StoryMission,
    backend: &mut B,
) -> Option<ReleaseOutcome> {
    enabled().then(|| release_successful_ending(mission, backend))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Log(Vec<(u8, Option<u32>)>);
    impl StoryControlReleaseBackend for Log {
        fn invoke_finish(&mut self, callback: CallbackHandle) {
            self.0.push((0, Some(callback.0)));
        }
        fn restore_camera_binding(&mut self, binding: Option<BindingHandle>) {
            self.0.push((1, binding.map(|v| v.0)));
        }
        fn restore_inventory_binding(&mut self, binding: Option<BindingHandle>) {
            self.0.push((2, binding.map(|v| v.0)));
        }
    }

    fn lifted_body(
        mission: &mut StoryMission,
        backend: &mut impl StoryControlReleaseBackend,
    ) -> ReleaseOutcome {
        // Branch-level transcription of the retained 0xc8640/0xd4be0 expressions.
        if mission.segment_index < mission.segment_count
            || mission.first_segment_flags & 1 != 0
            || mission.attract_mode
        {
            return ReleaseOutcome::NotReady;
        }
        let Some(camera) = mission.active_camera else {
            return ReleaseOutcome::Unsupported;
        };
        let Some(context) = mission.local_player.as_mut() else {
            return ReleaseOutcome::Unsupported;
        };
        if context.camera.is_none() || context.camera != Some(camera) {
            return ReleaseOutcome::NotReady;
        }
        context.cinematic_flags &= 0xfdff_ffff;
        match context.finish_callback {
            Some(callback) => {
                backend.invoke_finish(callback);
                context.finish_callback = None;
            }
            None => {}
        }
        context.update_callback = None;
        backend.restore_camera_binding(context.camera_binding);
        backend.restore_inventory_binding(context.inventory_binding);
        if context.mode == 0x10 {
            return ReleaseOutcome::Released;
        }
        context.mode = 2;
        context.timer = 0;
        ReleaseOutcome::Released
    }

    #[test]
    fn selector_is_default_off_and_requires_exact_opt_in() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"1")));
        assert!(!enabled_from(Some(b"Rust")));
        assert!(enabled_from(Some(b"rust")));
        assert!(!enabled());
        let mut mission = StoryMission {
            segment_index: 0,
            segment_count: 0,
            first_segment_flags: 0,
            attract_mode: false,
            active_camera: None,
            local_player: None,
        };
        assert_eq!(selected_release(&mut mission, &mut Log::default()), None);
    }

    #[test]
    fn weak_tier1_matches_lifted_transcription_for_3000_fabricated_states() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..3000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let id = |shift| ((seed >> shift) as u32).wrapping_add(1);
            let context = PlayerContext {
                camera: (seed & 1 != 0).then_some(CameraId(id(8))),
                cinematic_flags: id(12),
                finish_callback: (seed & 2 != 0).then_some(CallbackHandle(id(16))),
                update_callback: (seed & 4 != 0).then_some(CallbackHandle(id(20))),
                camera_binding: (seed & 8 != 0).then_some(BindingHandle(id(24))),
                inventory_binding: (seed & 16 != 0).then_some(BindingHandle(id(28))),
                mode: (seed >> 32) as u32 & 0x1f,
                timer: (seed >> 40) as u32,
            };
            let input = StoryMission {
                segment_index: (seed >> 3) as u32 % 4,
                segment_count: (seed >> 5) as u32 % 4,
                first_segment_flags: (seed >> 7) as u32,
                attract_mode: seed & 32 != 0,
                active_camera: (seed & 64 != 0).then_some(CameraId(id(36))),
                local_player: (seed & 128 != 0).then_some(context),
            };
            let (mut rust, mut lifted) = (input, input);
            let (mut rust_log, mut lifted_log) = (Log::default(), Log::default());
            let got = release_successful_ending(&mut rust, &mut rust_log);
            let expected = lifted_body(&mut lifted, &mut lifted_log);
            assert_eq!(
                (got, rust, rust_log.0),
                (expected, lifted, lifted_log.0),
                "seed={seed:#018x}"
            );
        }
    }

    #[test]
    fn semantic_mutations_are_detected_by_contract_witnesses() {
        let base = StoryMission {
            segment_index: 2,
            segment_count: 2,
            first_segment_flags: 0,
            attract_mode: false,
            active_camera: Some(CameraId(1)),
            local_player: Some(PlayerContext {
                camera: Some(CameraId(1)),
                cinematic_flags: 0x0200_0001,
                finish_callback: Some(CallbackHandle(7)),
                update_callback: Some(CallbackHandle(8)),
                camera_binding: Some(BindingHandle(9)),
                inventory_binding: Some(BindingHandle(10)),
                mode: 3,
                timer: 55,
            }),
        };
        let mut state = base;
        let mut log = Log::default();
        assert_eq!(
            release_successful_ending(&mut state, &mut log),
            ReleaseOutcome::Released
        );
        let context = state.local_player.unwrap();
        assert_eq!(context.cinematic_flags & 0x0200_0000, 0, "clear-bit mutant");
        assert_eq!(
            log.0,
            [(0, Some(7)), (1, Some(9)), (2, Some(10))],
            "callback/order mutant"
        );
        assert_eq!(
            (context.mode, context.timer),
            (2, 0),
            "missing-transition mutant"
        );

        // Boundary mutant: ignoring the final-segment comparison must release this state.
        let mut before_final = base;
        before_final.segment_index = 1;
        let mut untouched = Log::default();
        assert_eq!(
            release_successful_ending(&mut before_final, &mut untouched),
            ReleaseOutcome::NotReady
        );
        assert!(untouched.0.is_empty());
    }
}
