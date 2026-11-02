//! T2211 explicit qualification: FABRICATED events through production profile,
//! router and gameplay consumers. Requires the existing isolated-storage guard.
//! Inert in normal runs; no guest-memory writes or forced success counters.
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tsfp_engine_fileio::storage_flow::{ActiveProfile, MapPayload, SampleMaps};
use tsfp_engine_fileio::storage_frontend::{Command, Target};
use tsfp_engine_fileio::vfs::status;
use tsfp_input::bindings::Action;
use tsfp_input::router::{DeviceEvent, Router};
use tsfp_input::settings::{Device, InputKind, InputSettings, SettingEdit};

struct NoSamples;
impl SampleMaps for NoSamples {
    fn load_sample(&mut self, _: u32) -> Result<MapPayload, u32> {
        Err(status::OBJECT_NAME_NOT_FOUND)
    }
}
#[derive(Default)]
struct Check {
    phase: u8,
    original: Option<ActiveProfile>,
    fresh: Option<ActiveProfile>,
    at: Option<Instant>,
    applied: u32,
}
static CHECK: OnceLock<Mutex<Check>> = OnceLock::new();
fn command(command: Command) -> Result<(), u32> {
    let completion_required = !matches!(command, Command::Open(_) | Command::Close);
    let receipt = crate::storage_live::native_command(command, &mut NoSamples)?;
    if completion_required && receipt.is_none() {
        return Err(status::INVALID_PARAMETER);
    }
    Ok(())
}

fn profile() -> Result<ActiveProfile, u32> {
    Ok(crate::storage_live::native_profile(0)
        .ok_or(status::INVALID_PARAMETER)?
        .saved)
}
fn settings(input: InputSettings) -> Result<(), u32> {
    command(Command::SaveInput { player: 0, input })
}

fn gameplay_check() -> bool {
    std::env::var("TSFP_INPUT_GAMEPLAY_CHECK").as_deref() == Ok("1")
}

// The gameplay check must not open the storage diagnostic's catalogue window at
// boot. Retain its isolation contract before any authenticated profile mutation.
fn isolated_root() -> bool {
    let Some(expected) = std::env::var_os("TSFP_STORAGE_ROUNDTRIP_ROOT") else {
        return false;
    };
    let mut args = std::env::args().skip(1);
    let mut selected = None;
    while let Some(arg) = args.next() {
        if arg == "--hdd" {
            selected = args.next().map(PathBuf::from);
        }
    }
    let Some(selected) = selected else {
        return false;
    };
    match (
        selected.canonicalize(),
        PathBuf::from(expected).canonicalize(),
    ) {
        (Ok(selected), Ok(expected)) => selected == expected,
        _ => false,
    }
}

pub fn before_tick(router: &mut Router, counters: [u32; 7]) {
    if !gameplay_check() && std::env::var("TSFP_INPUT_PROFILE_CHECK").as_deref() != Ok("1") {
        return;
    }
    let mut check = CHECK
        .get_or_init(|| Mutex::new(Check::default()))
        .lock()
        .unwrap();
    if check.phase == 255 {
        return;
    }
    if !isolated_root() {
        eprintln!(
            "tsfp-bevy: input profile diagnostic REFUSED: requires explicit matching isolated storage root"
        );
        check.phase = 255;
        return;
    }
    if let Err(error) = check.advance(router, counters) {
        eprintln!(
            "tsfp-bevy: input profile diagnostic FAILED phase={} status={error:#010x}",
            check.phase
        );
        check.phase = 255;
    }
}
impl Check {
    fn advance(&mut self, router: &mut Router, counters: [u32; 7]) -> Result<(), u32> {
        match self.phase {
            0 => {
                if gameplay_check() && counters[1].saturating_sub(counters[2]) <= 20 {
                    return Ok(());
                }
                if crate::storage_live::native_profile_defaults().is_none() {
                    return Ok(());
                }
                command(Command::Open(Target::Profile { player: 0 }))?;
                crate::storage_live::native_load_selection()?;
                let original = profile()?;
                self.original = Some(original.clone());
                command(Command::CreateProfile {
                    player: 0,
                    directory: "t2211-new-defaults".into(),
                    label: "Input qualification".into(),
                    candidate: original.profile.clone(),
                })?;
                let fresh = profile()?;
                if !fresh.native_input
                    || fresh.input != InputSettings::default()
                    || crate::ui_extensions::settings() != Some(InputSettings::default())
                {
                    return Err(status::INVALID_PARAMETER);
                }
                self.fresh = Some(fresh);
                self.at = Some(Instant::now());
                self.phase = 10;
            }
            10 if self.at.unwrap().elapsed() > Duration::from_millis(100) => {
                router.feed(DeviceEvent::MouseMotion { dx: 100.0, dy: 0.0 });
                self.at = Some(Instant::now());
                self.phase = 11;
            }
            11 if self.at.unwrap().elapsed() > Duration::from_millis(100) => {
                let fresh = self.fresh.as_ref().unwrap().clone();
                let original = self.original.as_ref().unwrap().clone();
                crate::storage_live::native_deactivate(0)?;
                command(Command::LoadProfile {
                    player: 0,
                    key: fresh.key,
                })?;
                if profile()?.input != InputSettings::default() {
                    return Err(status::INVALID_PARAMETER);
                }
                eprintln!(
                    "tsfp-bevy: input profile diagnostic: live create/authenticated reload/adopt four fresh default maps PASS (owned storage API; visible create UI unmeasured)"
                );
                command(Command::LoadProfile {
                    player: 0,
                    key: original.key,
                })?;
                let mut baseline = InputSettings::default();
                baseline
                    .apply(SettingEdit::AssignDevice {
                        player: 0,
                        device: Device::KeyboardMouse,
                    })
                    .map_err(|_| status::INVALID_PARAMETER)?;
                baseline
                    .apply(SettingEdit::Rebind {
                        kind: InputKind::Key,
                        input: "F8".into(),
                        action: Action::None,
                    })
                    .map_err(|_| status::INVALID_PARAMETER)?;
                settings(baseline)?;
                self.phase = 1;
            }
            1 if counters[1].saturating_sub(counters[2]) > 20 && !crate::ui_pages::ingame() => {
                // Stage after settings adoption; counts traverse the normal live tick.
                router.feed(DeviceEvent::MouseMotion { dx: 100.0, dy: 0.0 });
                router.feed(DeviceEvent::Key {
                    name: "F8".into(),
                    down: true,
                });
                eprintln!(
                    "tsfp-bevy: input profile diagnostic: baseline 100 counts at 0.022 deg/count; F8 unbound, gameplay hook available"
                );
                self.applied = counters[4];
                self.at = Some(Instant::now());
                self.phase = 2;
            }
            2 if self.at.unwrap().elapsed() > Duration::from_secs(1) => {
                router.feed(DeviceEvent::Key {
                    name: "F8".into(),
                    down: false,
                });
                if counters[4] <= self.applied || crate::ui_pages::ingame() {
                    return Err(status::INVALID_PARAMETER);
                }
                let mut input = profile()?.input;
                input
                    .apply_player(0, SettingEdit::SensitivityYaw(0.12))
                    .map_err(|_| status::INVALID_PARAMETER)?;
                input
                    .apply_player(
                        0,
                        SettingEdit::Rebind {
                            kind: InputKind::Key,
                            input: "F8".into(),
                            action: Action::Start,
                        },
                    )
                    .map_err(|_| status::INVALID_PARAMETER)?;
                settings(input)?;
                self.phase = 3;
                self.at = Some(Instant::now());
            }
            3 if self.at.unwrap().elapsed() > Duration::from_millis(100) => {
                router.feed(DeviceEvent::MouseMotion { dx: 100.0, dy: 0.0 });
                eprintln!(
                    "tsfp-bevy: input profile diagnostic: edited 100 counts at 0.12 deg/count (authenticated profile -> live router -> game)"
                );
                self.applied = counters[4];
                self.at = Some(Instant::now());
                self.phase = 4;
            }
            4 if self.at.unwrap().elapsed() > Duration::from_secs(1) => {
                if counters[4] <= self.applied {
                    return Err(status::INVALID_PARAMETER);
                }
                router.feed(DeviceEvent::Key {
                    name: "F8".into(),
                    down: true,
                });
                self.at = Some(Instant::now());
                self.phase = 5;
            }
            5 => {
                if crate::ui_pages::ingame() {
                    router.feed(DeviceEvent::Key {
                        name: "F8".into(),
                        down: false,
                    });
                    eprintln!(
                        "tsfp-bevy: input profile diagnostic: rebound F8 opens live pause page {:#x} PASS; baseline/edited look applied by game hook",
                        crate::ui_pages::top_builder()
                    );
                    settings(self.original.as_ref().unwrap().input.clone())?;
                    self.phase = 255;
                } else if self.at.unwrap().elapsed() > Duration::from_secs(5) {
                    return Err(status::INVALID_PARAMETER);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

pub fn after_tick(inputs: &[Option<tsfp_input::player::PlayerInput>]) {
    let Some(check) = CHECK.get() else {
        return;
    };
    let mut check = check.lock().unwrap();
    let Some(input) = inputs[0] else {
        return;
    };
    if let tsfp_input::player::Look::Delta { yaw_deg, pitch_deg } = input.look {
        if yaw_deg != 0.0 && matches!(check.phase, 2 | 4 | 11) {
            let expected = if check.phase == 4 { 12.0 } else { 2.2 };
            if (yaw_deg - expected).abs() > 0.001 {
                eprintln!(
                    "tsfp-bevy: input profile diagnostic FAILED: router yaw {yaw_deg}, expected {expected}"
                );
                check.phase = 255;
            } else {
                eprintln!(
                    "tsfp-bevy: input profile diagnostic: measured router phase={} yaw={yaw_deg} pitch={pitch_deg} -> live game mailbox",
                    check.phase
                );
            }
        }
    }
}
