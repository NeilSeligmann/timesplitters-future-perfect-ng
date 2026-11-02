//! Launcher state and messages, independent of the UI toolkit (T4349). The Slint view in `main.rs` turns user
//! actions into `Msg`, runs the returned `Effect` (worker thread or native file dialog) and redraws from this state.
use crate::mod_archive;
use crate::pack_install;
use crate::settings_model::{self, Edit};
use crate::updater;
use std::path::PathBuf;
use tsfp_launcher_core::config::{self, Channel, Config};
use tsfp_launcher_core::game::{self, GameState, Repo};
use tsfp_launcher_core::identify::{self, Identity};
use tsfp_launcher_core::install::{self, Status, Step};
use tsfp_launcher_core::update::Offer;
use tsfp_launcher_core::{mods, packs};
use tsfp_settings::Settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Play,
    Setup,
    Settings,
    Content,
    Mods,
    Verify,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Play,
        Tab::Setup,
        Tab::Settings,
        Tab::Content,
        Tab::Mods,
        Tab::Verify,
    ];
    pub fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

#[derive(Clone, Debug)]
pub enum Msg {
    Tab(Tab),
    DiscPath(String),
    /// A file or folder dropped on the window (from any page).
    Dropped(PathBuf),
    /// A file is being dragged over the window (`false` when it leaves or is dropped).
    DragHover(bool),
    BrowseDisc,
    BrowseGameDir,
    Picked(Option<PathBuf>, bool),
    Identify,
    Identified(Result<Identity, String>),
    AllowUnknown(bool),
    Install,
    Progress(Step),
    InstallDone(Result<(), String>),
    Verify,
    VerifyProgress(usize, usize),
    Verified(Vec<Status>),
    GameDir(String),
    Channel(Channel),
    CheckUpdates(bool),
    UpdateCheck,
    UpdateFound(Result<Option<Offer>, String>),
    UpdateApply,
    UpdateApplied(Result<String, String>),
    ToggleMod(String, bool),
    MoveMod(String, isize),
    PackInstall(&'static str),
    PackRemove(&'static str),
    PackProgress(String),
    PackDone(Result<String, String>),
    RemoveMod(String),
    BrowseMod,
    PickedMod(Option<PathBuf>),
    ModInstalled(Result<String, String>),
    Setting(Edit),
    GameProbe,
    GameProbed(GameState),
    BuildGame,
    /// Build running: elapsed seconds and the last output line.
    BuildTick(u64, String),
    BuildDone(Result<(), String>),
    CancelBuild,
    GameExited(Option<i32>),
    Save,
    Play,
}

pub struct App {
    pub tab: Tab,
    pub config: Config,
    config_path: Option<PathBuf>,
    pub settings: Settings,
    settings_path: Option<PathBuf>,
    drafts: Vec<(&'static str, String)>,
    pub disc: String,
    pub identity: Option<Result<Identity, String>>,
    pub allow_unknown: bool,
    pub busy: bool,
    /// An install is running (busy is also set by identify, verify, packs and updates).
    pub installing: bool,
    /// Steps of the current or last install, for the progress view.
    pub steps: Vec<StepView>,
    pub drag_hover: bool,
    /// Items checked by a running Verify: (done, total).
    pub verify_progress: Option<(usize, usize)>,
    /// Repository checkout that builds and runs the game (rule 61); `None` in packaged launchers.
    pub repo: Option<Repo>,
    pub game: GameState,
    pub run: Run,
    /// Process group of a running build, for Cancel.
    build_pid: std::sync::Arc<std::sync::Mutex<Option<u32>>>,
    pub log: Vec<String>,
    pub status: Vec<Status>,
    pub notice: String,
    pub update: Option<Offer>,
}

/// Game executable shipped beside the launcher; absent on platforms the port does not run on yet.
pub fn game_executable() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let exe = dir.join(format!("tsfp-game{}", std::env::consts::EXE_SUFFIX));
    exe.is_file().then_some(exe)
}

/// Background job: runs on a worker thread and reports through the callback.
pub type Work = Box<dyn FnOnce(&dyn Fn(Msg)) + Send>;

/// Work the UI must run for a message: on a worker thread (reporting through `send`), or a native dialog.
pub enum Effect {
    None,
    Background(Work),
    PickDisc,
    PickGameDir,
    PickMod,
}

fn background(work: impl FnOnce(&dyn Fn(Msg)) + Send + 'static) -> Effect {
    Effect::Background(Box::new(work))
}

impl App {
    pub fn new(config_path: Option<PathBuf>, settings_path: Option<PathBuf>) -> Self {
        let mut notice = String::new();
        let config = match &config_path {
            Some(p) => Config::load(p, config::default_game_dir()).unwrap_or_else(|e| {
                notice = e;
                Config::with_game_dir(config::default_game_dir())
            }),
            None => Config::with_game_dir(config::default_game_dir()),
        };
        let settings = settings_path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .map_or_else(Settings::default, |b| {
                Settings::parse(&b).unwrap_or_else(|e| {
                    notice = format!("settings.json: {e}; using defaults");
                    Settings::default()
                })
            });
        let status = install::quick_status(&config.game_dir);
        let tab = if matches!(status[..], [Status::NotInstalled]) {
            Tab::Setup
        } else {
            Tab::Play
        };
        let disc = config.last_disc.clone().unwrap_or_default();
        App {
            tab,
            config,
            config_path,
            settings,
            settings_path,
            drafts: Vec::new(),
            disc,
            identity: None,
            allow_unknown: false,
            busy: false,
            installing: false,
            steps: Vec::new(),
            drag_hover: false,
            verify_progress: None,
            repo: game::default_repo(),
            game: GameState::NoRuntime,
            run: Run::Idle,
            build_pid: Default::default(),
            log: Vec::new(),
            status,
            notice,
            update: None,
        }
    }

    /// The app plus the messages to run once the window is up.
    pub fn boot() -> (Self, Vec<Msg>) {
        let dir = config::default_config_dir();
        let app = Self::new(
            dir.as_ref().map(|d| d.join("launcher.json")),
            tsfp_settings::user_settings_path(),
        );
        let mut first = app.startup_messages();
        if app.config.check_updates {
            first.push(Msg::UpdateCheck);
        }
        (app, first)
    }

    /// Identify the remembered disc again if it still exists.
    pub fn startup_messages(&self) -> Vec<Msg> {
        let remembered = !self.disc.is_empty() && std::path::Path::new(&self.disc).exists();
        let mut out = if remembered {
            vec![Msg::Identify]
        } else {
            Vec::new()
        };
        if self.repo.is_some() {
            out.push(Msg::GameProbe);
        }
        out
    }

    /// Disc image the game should read: the verified copy in the game folder, else the remembered image.
    pub fn game_disc(&self) -> Option<PathBuf> {
        install::installed_ids(&self.config.game_dir)
            .iter()
            .find_map(|id| install::disc_copy(&self.config.game_dir, id))
            .or_else(|| {
                self.config
                    .last_disc
                    .as_ref()
                    .map(PathBuf::from)
                    .filter(|p| p.is_file())
            })
    }

    fn log_path(kind: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        config::default_config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("logs")
            .join(format!("{kind}-{stamp}.log"))
    }

    /// Remember the disc for the next start; failures only show in the status line.
    fn remember_disc(&mut self, disc: String) {
        if self.config.last_disc.as_deref() == Some(disc.as_str()) {
            return;
        }
        self.config.last_disc = Some(disc);
        if let Some(p) = &self.config_path
            && let Err(e) = self.config.save(p)
        {
            self.notice = e;
        }
    }

    pub fn installed(&self) -> bool {
        self.status
            .iter()
            .any(|s| matches!(s, Status::Installed { .. }))
    }

    pub fn update(&mut self, msg: Msg) -> Effect {
        match msg {
            Msg::Tab(t) => self.tab = t,
            Msg::DiscPath(p) => {
                self.disc = clean_path(&p);
                self.identity = None;
            }
            Msg::DragHover(on) => self.drag_hover = on,
            Msg::Dropped(path) => {
                self.drag_hover = false;
                let zip = path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
                if self.tab == Tab::Mods && zip {
                    return self.update(Msg::PickedMod(Some(path)));
                }
                if self.busy {
                    self.notice = "Wait for the current task to finish.".into();
                } else {
                    self.tab = Tab::Setup;
                    return self.update(Msg::Picked(Some(path), true));
                }
            }
            Msg::BrowseDisc => return Effect::PickDisc,
            Msg::BrowseGameDir => return Effect::PickGameDir,
            Msg::Picked(Some(p), true) => {
                self.disc = p.display().to_string();
                return self.update(Msg::Identify);
            }
            Msg::Picked(Some(p), false) => {
                self.config.game_dir = p;
                self.status = install::quick_status(&self.config.game_dir);
            }
            Msg::Picked(None, _) => {}
            Msg::Identify if !self.disc.is_empty() && !self.busy => {
                self.busy = true;
                let disc = PathBuf::from(&self.disc);
                return background(move |send| {
                    send(Msg::Identified(identify::identify(&disc)));
                });
            }
            Msg::Identify => {}
            Msg::Identified(r) => {
                self.busy = false;
                if matches!(&r, Ok(id) if id.xbe_sha256.is_some()) {
                    self.remember_disc(self.disc.clone());
                }
                self.identity = Some(r);
            }
            Msg::AllowUnknown(v) => self.allow_unknown = v,
            Msg::Install if !self.busy => {
                self.busy = true;
                self.installing = true;
                self.log.clear();
                self.steps = plan(install::image_size(std::path::Path::new(&self.disc)).is_some());
                let (disc, dir, allow) = (
                    PathBuf::from(&self.disc),
                    self.config.game_dir.clone(),
                    self.allow_unknown,
                );
                return background(move |send| {
                    let mut throttle = Throttle::default();
                    let r = install::install(&disc, &dir, allow, &mut |s| {
                        // Per-item progress is thinned to ~10 updates a second; step results always pass.
                        let progress = matches!(
                            s,
                            Step::Copying { .. }
                                | Step::Checking { .. }
                                | Step::Extracting { .. }
                                | Step::Converting { .. }
                                | Step::Verifying { .. }
                        );
                        if !progress || throttle.ready() {
                            send(Msg::Progress(s.clone()))
                        }
                    });
                    send(Msg::InstallDone(r.map(|_| ())));
                });
            }
            Msg::Install => {}
            Msg::Progress(s) => advance(&mut self.steps, &s),
            Msg::InstallDone(r) => {
                self.busy = false;
                self.installing = false;
                match r {
                    Ok(()) => {
                        self.log.push("Install complete.".into());
                        // The verified copy in the game folder outlives the original image.
                        let copy = self
                            .identity
                            .as_ref()
                            .and_then(|r| r.as_ref().ok())
                            .and_then(|id| {
                                install::disc_copy(&self.config.game_dir, id.version_id())
                            });
                        if let Some(copy) = copy {
                            self.remember_disc(copy.display().to_string());
                        }
                        // The install just verified every file.
                        self.status = install::quick_status(&self.config.game_dir)
                            .into_iter()
                            .map(|s| match s {
                                Status::Installed { id, files, .. } => Status::Installed {
                                    id,
                                    files,
                                    verified: true,
                                },
                                other => other,
                            })
                            .collect();
                        self.notice = self.save_all();
                        if self.repo.is_some() {
                            return self.update(Msg::GameProbe);
                        }
                    }
                    Err(e) => {
                        if let Some(step) = self
                            .steps
                            .iter_mut()
                            .find(|s| s.state == StepState::Running)
                        {
                            step.state = StepState::Failed;
                            step.detail = e.clone();
                        }
                        self.log.push(format!("Install failed: {e}"));
                    }
                }
            }
            Msg::Verify if !self.busy => {
                self.busy = true;
                let dir = self.config.game_dir.clone();
                self.verify_progress = Some((0, 0));
                return background(move |send| {
                    let mut throttle = Throttle::default();
                    let status = install::status_progress(&dir, &mut |done, total| {
                        if throttle.ready() {
                            send(Msg::VerifyProgress(done, total));
                        }
                    });
                    send(Msg::Verified(status));
                });
            }
            Msg::Verify => {}
            Msg::VerifyProgress(done, total) => self.verify_progress = Some((done, total)),
            Msg::Verified(s) => {
                self.busy = false;
                self.verify_progress = None;
                self.status = s;
            }
            Msg::GameDir(d) => {
                self.config.game_dir = PathBuf::from(d);
                self.status = install::quick_status(&self.config.game_dir);
            }
            Msg::Channel(c) => self.config.channel = c,
            Msg::CheckUpdates(v) => self.config.check_updates = v,
            Msg::UpdateCheck => {
                let channel = self.config.channel;
                return background(move |send| send(Msg::UpdateFound(updater::check(channel))));
            }
            Msg::UpdateFound(Ok(offer)) => {
                if offer.is_none() {
                    self.notice = "Launcher is up to date.".into();
                }
                self.update = offer;
            }
            Msg::UpdateFound(Err(e)) => self.notice = format!("Update check failed: {e}"),
            Msg::UpdateApply if !self.busy => {
                if let Some(offer) = self.update.clone() {
                    self.busy = true;
                    self.notice = format!("Downloading {}...", offer.version);
                    return background(move |send| {
                        send(Msg::UpdateApplied(updater::apply(&offer)))
                    });
                }
            }
            Msg::UpdateApply => {}
            Msg::UpdateApplied(r) => {
                self.busy = false;
                match r {
                    Ok(m) => {
                        self.notice = m;
                        self.update = None;
                    }
                    Err(e) => self.notice = format!("Update failed: {e}"),
                }
            }
            Msg::ToggleMod(id, on) => {
                self.config.enabled_mods.retain(|m| m != &id);
                if on {
                    self.config.enabled_mods.push(id);
                }
            }
            Msg::PackInstall(id) if !self.busy => {
                if let Some(url) = packs::CATALOG
                    .iter()
                    .find(|p| p.id == id)
                    .and_then(|p| p.manifest_url)
                {
                    self.busy = true;
                    let dir = self.config.game_dir.clone();
                    return background(move |send| {
                        let r = pack_install::install(&dir, url, &|m| send(Msg::PackProgress(m)));
                        send(Msg::PackDone(r));
                    });
                }
            }
            Msg::PackInstall(_) => {}
            Msg::PackRemove(id) => {
                self.notice = match packs::uninstall(&self.config.game_dir, id) {
                    Ok(()) => format!("Removed {id}."),
                    Err(e) => e,
                };
            }
            Msg::PackProgress(m) => self.notice = m,
            Msg::PackDone(Ok(m)) => {
                self.busy = false;
                self.notice = m;
            }
            Msg::PackDone(Err(e)) => {
                self.busy = false;
                self.notice = format!("Pack install failed: {e}");
            }
            Msg::MoveMod(id, delta) => {
                mods::move_in_order(&mut self.config.enabled_mods, &id, delta)
            }
            Msg::RemoveMod(id) => {
                self.config.enabled_mods.retain(|m| m != &id);
                self.notice = match mods::uninstall(&self.config.game_dir.join("mods"), &id) {
                    Ok(()) => format!("Removed {id}."),
                    Err(e) => e,
                };
            }
            Msg::BrowseMod => return Effect::PickMod,
            Msg::PickedMod(Some(p)) => {
                let dir = self.config.game_dir.join("mods");
                return background(move |send| {
                    send(Msg::ModInstalled(
                        mod_archive::install(&p, &dir)
                            .map(|m| format!("Installed {} {}.", m.name, m.version)),
                    ))
                });
            }
            Msg::PickedMod(None) => {}
            Msg::ModInstalled(r) => {
                self.notice = r.unwrap_or_else(|e| format!("Mod install failed: {e}"))
            }
            Msg::Setting(e) => settings_model::apply(&mut self.settings, &mut self.drafts, e),
            Msg::Save => self.notice = self.save_all(),
            Msg::Play => {
                if let Some(exe) = game_executable() {
                    self.notice = match std::process::Command::new(&exe)
                        .arg("--game-dir")
                        .arg(&self.config.game_dir)
                        .spawn()
                    {
                        Ok(_) => "Game started.".into(),
                        Err(e) => format!("{}: {e}", exe.display()),
                    };
                } else if let (Some(repo), GameState::Ready, Run::Idle) =
                    (self.repo.clone(), &self.game, &self.run)
                {
                    let log = Self::log_path("play");
                    match game::play(&repo, self.game_disc().as_deref(), &log) {
                        Ok(mut child) => {
                            self.run = Run::Playing { log: log.clone() };
                            self.notice = format!("Game started. Log: {}", log.display());
                            return background(move |send| {
                                send(Msg::GameExited(child.wait().ok().and_then(|s| s.code())))
                            });
                        }
                        Err(e) => self.notice = format!("Could not start the game: {e}"),
                    }
                }
            }
            Msg::GameExited(code) => {
                let log = match std::mem::replace(&mut self.run, Run::Idle) {
                    Run::Playing { log } => log,
                    _ => PathBuf::new(),
                };
                self.notice = match code {
                    Some(0) => "Game closed.".into(),
                    other => format!(
                        "The game stopped (exit {}): {}. Log: {}",
                        other.map_or("signal".into(), |c| c.to_string()),
                        game::last_line(&log),
                        log.display()
                    ),
                };
                return self.update(Msg::GameProbe);
            }
            Msg::GameProbe => {
                if let Some(repo) = self.repo.clone() {
                    let disc = self.game_disc();
                    return background(move |send| {
                        send(Msg::GameProbed(game::probe(&repo, disc.as_deref())))
                    });
                }
            }
            Msg::GameProbed(state) => self.game = state,
            Msg::BuildGame => {
                let (Some(repo), GameState::NeedsBuild { args, .. }, Run::Idle) =
                    (self.repo.clone(), self.game.clone(), &self.run)
                else {
                    return Effect::None;
                };
                let log = Self::log_path("build");
                match game::build(&repo, &args, &log) {
                    Ok(mut child) => {
                        *self.build_pid.lock().unwrap_or_else(|e| e.into_inner()) =
                            Some(child.id());
                        self.run = Run::Building {
                            log: log.clone(),
                            seconds: 0,
                            line: String::new(),
                        };
                        let pid = self.build_pid.clone();
                        return background(move |send| {
                            let start = std::time::Instant::now();
                            let status = loop {
                                match child.try_wait() {
                                    Ok(Some(status)) => break Ok(status),
                                    Ok(None) => {
                                        send(Msg::BuildTick(
                                            start.elapsed().as_secs(),
                                            game::last_line(&log),
                                        ));
                                        std::thread::sleep(std::time::Duration::from_millis(500));
                                    }
                                    Err(e) => break Err(e.to_string()),
                                }
                            };
                            *pid.lock().unwrap_or_else(|e| e.into_inner()) = None;
                            send(Msg::BuildDone(match status {
                                Ok(s) if s.success() => Ok(()),
                                Ok(s) => Err(format!(
                                    "build.sh failed (exit {}): {}",
                                    s.code().map_or("signal".into(), |c| c.to_string()),
                                    game::last_line(&log)
                                )),
                                Err(e) => Err(e),
                            }));
                        });
                    }
                    Err(e) => self.notice = format!("Could not start the build: {e}"),
                }
            }
            Msg::BuildTick(seconds, line) => {
                if let Run::Building {
                    seconds: s,
                    line: l,
                    ..
                } = &mut self.run
                {
                    *s = seconds;
                    if !line.is_empty() {
                        *l = line;
                    }
                }
            }
            Msg::CancelBuild => {
                if let Some(pid) = *self.build_pid.lock().unwrap_or_else(|e| e.into_inner()) {
                    game::stop(pid);
                    self.notice = "Stopping the build…".into();
                }
            }
            Msg::BuildDone(r) => {
                let log = match std::mem::replace(&mut self.run, Run::Idle) {
                    Run::Building { log, .. } => log,
                    _ => PathBuf::new(),
                };
                self.notice = match r {
                    Ok(()) => "Build finished.".into(),
                    Err(e) => format!("{e}. Log: {}", log.display()),
                };
                return self.update(Msg::GameProbe);
            }
        }
        Effect::None
    }

    fn save_all(&self) -> String {
        let mut errs = Vec::new();
        if let Some(p) = &self.config_path
            && let Err(e) = self.config.save(p)
        {
            errs.push(e);
        }
        if let Some(p) = &self.settings_path
            && let Err(e) = self.settings.save(p)
        {
            errs.push(format!("{}: {e}", p.display()));
        }
        // The game reads only active.json; mods enabled in the config but no longer installed are left out.
        let mods_dir = self.config.game_dir.join("mods");
        let (installed, _) = mods::scan(&mods_dir);
        let order: Vec<String> = self
            .config
            .enabled_mods
            .iter()
            .filter(|id| installed.iter().any(|m| &m.id == *id))
            .cloned()
            .collect();
        if (!order.is_empty() || mods_dir.join("active.json").exists())
            && let Err(e) = mods::write_active(&mods_dir, &installed, &order)
        {
            errs.push(e);
        }
        if errs.is_empty() {
            "Saved.".into()
        } else {
            errs.join("; ")
        }
    }
}

impl App {
    pub fn drafts(&self) -> &[(&'static str, String)] {
        &self.drafts
    }
}

/// Lets a progress message through at most every 100 ms.
#[derive(Default)]
struct Throttle(Option<std::time::Instant>);

impl Throttle {
    fn ready(&mut self) -> bool {
        let now = std::time::Instant::now();
        if self
            .0
            .is_some_and(|t| now.duration_since(t) < std::time::Duration::from_millis(100))
        {
            return false;
        }
        self.0 = Some(now);
        true
    }
}

/// What the launcher is running for the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Run {
    Idle,
    Building {
        log: PathBuf,
        seconds: u64,
        line: String,
    },
    Playing {
        log: PathBuf,
    },
}

/// Play card: label, explanation, detail line and button action
/// (0 disabled, 1 play, 2 build, 3 go to setup, 4 cancel build).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayView {
    pub label: String,
    pub hint: String,
    pub detail: String,
    pub action: i32,
}

impl App {
    pub fn play_view(&self) -> PlayView {
        let view = |label: &str, hint: String, action: i32| PlayView {
            label: label.into(),
            hint,
            detail: String::new(),
            action,
        };
        let setup = || {
            view(
                "Set up the game",
                "Import your own copy of the game to get started.".into(),
                3,
            )
        };
        if game_executable().is_some() {
            return if self.installed() {
                view("▶  Play", "Everything is installed and verified.".into(), 1)
            } else {
                setup()
            };
        }
        match (&self.run, &self.game, &self.repo) {
            (Run::Building { seconds, line, log }, _, _) => PlayView {
                label: "Cancel build".into(),
                hint: format!("Building the game… {}:{:02}", seconds / 60, seconds % 60),
                detail: if line.is_empty() {
                    log.display().to_string()
                } else {
                    line.clone()
                },
                action: 4,
            },
            (Run::Playing { log }, _, _) => PlayView {
                label: "Running…".into(),
                hint: "The game is running.".into(),
                detail: log.display().to_string(),
                action: 0,
            },
            (_, GameState::Ready, _) => {
                view("▶  Play", "Ready: your own build of the game.".into(), 1)
            }
            (_, GameState::NeedsBuild { reason, .. }, _) => view(
                "Build the game",
                format!(
                    "{reason} Building compiles the game from your disc on this computer (several minutes)."
                ),
                2,
            ),
            (_, GameState::NoDisc, _) => view(
                "Set up the game",
                "Choose your disc image in Setup; the game reads it to run.".into(),
                3,
            ),
            (_, GameState::Error(e), _) => view("Play", format!("Cannot start the game: {e}"), 0),
            (_, GameState::NoRuntime, Some(_)) => view("Play", "Checking the game build…".into(), 0),
            (_, GameState::NoRuntime, None) if !self.installed() => setup(),
            (_, GameState::NoRuntime, None) => view(
                "Play",
                "This launcher does not include the game yet: build and run it from a repository checkout.".into(),
                0,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Pending,
    Running,
    Done,
    Failed,
}

/// One row of the install progress view. `progress` is 0..1, or negative while the amount of work is unknown.
#[derive(Clone, Debug, PartialEq)]
pub struct StepView {
    pub key: &'static str,
    pub label: &'static str,
    pub state: StepState,
    pub detail: String,
    pub progress: f32,
}

fn plan(copy_image: bool) -> Vec<StepView> {
    let mut keys = vec![("identify", "Identify the disc")];
    if copy_image {
        keys.push(("copy", "Copy the disc image into the game folder"));
    }
    keys.extend([
        ("check", "Check an existing install"),
        ("extract", "Extract game files"),
        ("convert", "Convert assets to open formats"),
        ("verify", "Verify every file"),
    ]);
    let mut steps: Vec<StepView> = keys
        .into_iter()
        .map(|(key, label)| StepView {
            key,
            label,
            state: StepState::Pending,
            detail: String::new(),
            progress: -1.0,
        })
        .collect();
    steps[0].state = StepState::Running;
    steps
}

fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

pub fn count(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Apply one install event to the step list: update the running step and start the next when one finishes.
fn advance(steps: &mut [StepView], event: &Step) {
    let (key, done, detail, progress) = match event {
        Step::Identified(v) => ("identify", true, v.clone(), 1.0),
        Step::Copying { done, total } => {
            let half = total / 2;
            let detail = if *done <= half {
                format!("Copying {} of {}", gb(*done), gb(half))
            } else {
                format!("Checking the copy: {} of {}", gb(done - half), gb(half))
            };
            ("copy", false, detail, *done as f32 / (*total).max(1) as f32)
        }
        Step::Copied { bytes, reused } => {
            let what = if *reused {
                "Existing copy still matches"
            } else {
                "Copied and checked"
            };
            ("copy", true, format!("{what} ({})", gb(*bytes)), 1.0)
        }
        Step::Checking { done, total } => (
            "check",
            false,
            format!("{} of {} checked", count(*done), count(*total)),
            *done as f32 / (*total).max(1) as f32,
        ),
        Step::Checked {
            existing,
            ok,
            files,
        } => {
            if *ok {
                // Everything verified: the remaining steps are not needed.
                for s in steps.iter_mut() {
                    if matches!(s.key, "extract" | "convert" | "verify") {
                        s.state = StepState::Done;
                        s.progress = 1.0;
                        s.detail = "Skipped: the existing install is complete".into();
                    }
                }
            }
            let detail = match (existing, ok) {
                (false, _) => "No earlier install of this disc".to_string(),
                (true, true) => format!("{} files verified, nothing to extract", count(*files)),
                (true, false) => "Incomplete or damaged: extracting again".to_string(),
            };
            ("check", true, detail, 1.0)
        }
        Step::Extracting { done, total } => (
            "extract",
            false,
            format!("{} of {} files", count(*done), count(*total)),
            *done as f32 / (*total).max(1) as f32,
        ),
        Step::Extracted(n) => ("extract", true, format!("{} files", count(*n)), 1.0),
        Step::Converting { done, total } => (
            "convert",
            false,
            format!("{} of {} source files", count(*done), count(*total)),
            *done as f32 / (*total).max(1) as f32,
        ),
        Step::Converted { outputs, lossless } => (
            "convert",
            true,
            format!(
                "{} assets, {} converted losslessly",
                count(*outputs),
                count(*lossless)
            ),
            1.0,
        ),
        Step::Verifying { done, total } => (
            "verify",
            false,
            format!("{} of {} checked", count(*done), count(*total)),
            *done as f32 / (*total).max(1) as f32,
        ),
        Step::Verified(n) => ("verify", true, format!("{} files verified", count(*n)), 1.0),
    };
    let Some(i) = steps.iter().position(|s| s.key == key) else {
        return;
    };
    let step = &mut steps[i];
    step.detail = detail;
    step.progress = progress;
    step.state = if done {
        StepState::Done
    } else {
        StepState::Running
    };
    if done
        && let Some(next) = steps.get_mut(i + 1)
        && next.state == StepState::Pending
    {
        next.state = StepState::Running;
    }
}

/// Overall install progress 0..1 from the step list.
pub fn overall(steps: &[StepView]) -> f32 {
    if steps.is_empty() {
        return 0.0;
    }
    let sum: f32 = steps
        .iter()
        .map(|s| match s.state {
            StepState::Done => 1.0,
            StepState::Running => s.progress.max(0.0),
            _ => 0.0,
        })
        .sum();
    sum / steps.len() as f32
}

/// Pasted paths: trim, drop surrounding quotes, accept `file://` URIs with percent escapes.
pub fn clean_path(raw: &str) -> String {
    let t = raw.trim().trim_matches(|c| c == '"' || c == '\'');
    let Some(rest) = t.strip_prefix("file://") else {
        return t.to_string();
    };
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(&rest[i + 1..i + 3], 16)
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    let path = String::from_utf8_lossy(&out).into_owned();
    // file:///C:/... on Windows
    if cfg!(windows) && path.starts_with('/') && path.as_bytes().get(2) == Some(&b':') {
        return path[1..].to_string();
    }
    path
}

/// What Setup says will be stored for the chosen disc.
pub fn disc_note(disc: &str) -> String {
    if disc.is_empty() {
        return String::new();
    }
    match install::image_size(std::path::Path::new(disc)) {
        Some(bytes) => format!(
            "The disc image ({}) is copied into the game folder, so future updates can read the original disc again.",
            gb(bytes)
        ),
        None => "The extracted files are kept in the game folder, so future updates can read them again.".into(),
    }
}

pub fn status_text(st: &Status) -> String {
    match st {
        Status::NotInstalled => "Not installed".to_string(),
        Status::Installed {
            id,
            files,
            verified: true,
        } => format!("{id}: {} files verified", count(*files)),
        Status::Installed { id, files, .. } => {
            format!(
                "{id}: installed ({} files). Verify re-checks every file.",
                count(*files)
            )
        }
        Status::Damaged { id, reason } => {
            format!("{id}: damaged ({reason}). Run Setup again to repair.")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn app(dir: &Path) -> App {
        let mut a = App::new(
            Some(dir.join("launcher.json")),
            Some(dir.join("settings.json")),
        );
        a.config.game_dir = dir.join("game");
        a
    }

    fn scratch(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("tsfp-launcher-gui-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn mod_toggle_keeps_load_order_and_save_writes_both_files() {
        let d = scratch("save");
        let mut a = app(&d);
        let _ = a.update(Msg::ToggleMod("a".into(), true));
        let _ = a.update(Msg::ToggleMod("b".into(), true));
        let _ = a.update(Msg::ToggleMod("a".into(), false));
        let _ = a.update(Msg::ToggleMod("a".into(), true));
        assert_eq!(a.config.enabled_mods, ["b", "a"]);
        let _ = a.update(Msg::Setting(Edit::Choice(
            "window_mode",
            "borderless".into(),
        )));
        let _ = a.update(Msg::Save);
        assert_eq!(a.notice, "Saved.");
        let b = app(&d);
        assert_eq!(b.config.enabled_mods, ["b", "a"]);
        assert_eq!(
            b.settings
                .get("window_mode")
                .and_then(serde_json::Value::as_str),
            Some("borderless")
        );
    }

    #[test]
    fn install_is_refused_while_busy() {
        let d = scratch("busy");
        let mut a = app(&d);
        a.busy = true;
        let _ = a.update(Msg::Install);
        let _ = a.update(Msg::Verify);
        assert!(a.log.is_empty() && a.busy);
    }

    #[test]
    fn fresh_install_opens_on_setup() {
        let d = scratch("fresh");
        let a = App::new(Some(d.join("launcher.json")), None);
        assert!(!a.installed());
        let mut cfg = Config::with_game_dir(d.join("empty"));
        cfg.channel = Channel::Nightly;
        cfg.save(&d.join("launcher.json")).unwrap();
        let a = App::new(Some(d.join("launcher.json")), None);
        assert_eq!((a.tab, a.config.channel), (Tab::Setup, Channel::Nightly));
    }
}

#[cfg(test)]
mod step_tests {
    use super::*;

    #[test]
    fn image_install_walks_every_step_with_progress() {
        let mut steps = plan(true);
        assert_eq!(steps.len(), 6);
        assert_eq!(steps[0].state, StepState::Running);
        advance(&mut steps, &Step::Identified("recognised".into()));
        advance(
            &mut steps,
            &Step::Copying {
                done: 3_000_000_000,
                total: 8_000_000_000,
            },
        );
        assert_eq!(steps[1].detail, "Copying 3.0 GB of 4.0 GB");
        advance(
            &mut steps,
            &Step::Copying {
                done: 6_000_000_000,
                total: 8_000_000_000,
            },
        );
        assert_eq!(steps[1].detail, "Checking the copy: 2.0 GB of 4.0 GB");
        assert!((steps[1].progress - 0.75).abs() < 1e-6);
        advance(
            &mut steps,
            &Step::Copied {
                bytes: 4_000_000_000,
                reused: false,
            },
        );
        advance(
            &mut steps,
            &Step::Checked {
                existing: false,
                ok: false,
                files: 0,
            },
        );
        advance(
            &mut steps,
            &Step::Extracting {
                done: 1204,
                total: 3410,
            },
        );
        assert_eq!(steps[3].detail, "1,204 of 3,410 files");
        assert_eq!(steps[3].state, StepState::Running);
        let mid = overall(&steps);
        assert!(mid > 0.5 && mid < 0.7, "{mid}");
        advance(&mut steps, &Step::Extracted(3410));
        advance(
            &mut steps,
            &Step::Converted {
                outputs: 9000,
                lossless: 8000,
            },
        );
        advance(&mut steps, &Step::Verified(12410));
        assert!(steps.iter().all(|s| s.state == StepState::Done));
        assert_eq!(overall(&steps), 1.0);
    }

    #[test]
    fn verified_existing_install_skips_the_rest() {
        let mut steps = plan(false);
        advance(&mut steps, &Step::Identified("ok".into()));
        advance(
            &mut steps,
            &Step::Checked {
                existing: true,
                ok: true,
                files: 42,
            },
        );
        assert!(steps.iter().all(|s| s.state == StepState::Done));
        assert!(steps[2].detail.starts_with("Skipped"));
    }

    #[test]
    fn pasted_paths_are_cleaned() {
        assert_eq!(
            clean_path("  \"/home/p/My Game.iso\"\n"),
            "/home/p/My Game.iso"
        );
        assert_eq!(
            clean_path("file:///home/p/My%20Game.iso"),
            "/home/p/My Game.iso"
        );
        assert_eq!(clean_path("'/a/b'"), "/a/b");
        assert_eq!(clean_path("file:///x%2"), "/x%2");
    }

    #[test]
    fn dropping_a_zip_on_mods_installs_it_and_anything_else_goes_to_setup() {
        let d = std::env::temp_dir().join(format!("tsfp-drop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let mut a = App::new(None, None);
        a.config.game_dir = d.join("game");
        a.tab = Tab::Mods;
        assert!(matches!(
            a.update(Msg::Dropped(d.join("m.zip"))),
            Effect::Background(_)
        ));
        assert_eq!(a.tab, Tab::Mods);
        a.update(Msg::DragHover(true));
        assert!(a.drag_hover);
        assert!(matches!(
            a.update(Msg::Dropped(d.join("disc.iso"))),
            Effect::Background(_)
        ));
        assert_eq!(
            (a.tab, a.disc.as_str(), a.drag_hover),
            (Tab::Setup, d.join("disc.iso").to_str().unwrap(), false)
        );
    }

    fn identifies(a: &App) -> bool {
        a.startup_messages()
            .iter()
            .any(|m| matches!(m, Msg::Identify))
    }

    #[test]
    fn identified_disc_is_remembered_and_reopened() {
        let d = std::env::temp_dir().join(format!("tsfp-remember-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("disc")).unwrap();
        std::fs::write(d.join("disc/default.xbe"), b"x").unwrap();
        let cfg = d.join("launcher.json");
        let mut a = App::new(Some(cfg.clone()), None);
        assert!(!identifies(&a));
        a.disc = d.join("disc").display().to_string();
        let ident = tsfp_launcher_core::identify::identify(&d.join("disc")).unwrap();
        a.update(Msg::Identified(Ok(ident)));
        let b = App::new(Some(cfg.clone()), None);
        assert_eq!(b.disc, d.join("disc").display().to_string());
        assert!(identifies(&b));
        std::fs::remove_dir_all(d.join("disc")).unwrap();
        assert!(!identifies(&App::new(Some(cfg), None)));
    }
}

#[cfg(test)]
mod play_tests {
    use super::*;

    fn app() -> App {
        let mut a = App::new(None, None);
        a.repo = Some(Repo {
            root: PathBuf::from("/repo"),
        });
        a
    }

    #[test]
    fn play_card_follows_the_game_state() {
        let mut a = app();
        assert_eq!(a.play_view().action, 0);
        a.update(Msg::GameProbed(GameState::NeedsBuild {
            reason: "The game sources changed since the last build.".into(),
            args: vec![],
        }));
        let v = a.play_view();
        assert_eq!((v.label.as_str(), v.action), ("Build the game", 2));
        assert!(v.hint.starts_with("The game sources changed"));
        a.update(Msg::GameProbed(GameState::Ready));
        assert_eq!(a.play_view().action, 1);
        a.update(Msg::GameProbed(GameState::NoDisc));
        assert_eq!(a.play_view().action, 3);
        a.run = Run::Building {
            log: PathBuf::from("/l"),
            seconds: 75,
            line: "Compiling tsfp-bevy".into(),
        };
        let v = a.play_view();
        assert_eq!(
            (v.action, v.hint.as_str(), v.detail.as_str()),
            (4, "Building the game… 1:15", "Compiling tsfp-bevy")
        );
        a.update(Msg::BuildTick(80, String::new()));
        assert!(a.play_view().hint.ends_with("1:20"));
        assert_eq!(a.play_view().detail, "Compiling tsfp-bevy");
    }

    #[test]
    fn without_a_checkout_the_message_is_honest() {
        let mut a = App::new(None, None);
        a.repo = None;
        a.status = vec![Status::Installed {
            id: "xbox-usa".into(),
            files: 1,
            verified: true,
        }];
        assert!(a.play_view().hint.contains("does not include the game yet"));
        assert_eq!(a.play_view().action, 0);
        // Building or playing needs a checkout.
        assert!(matches!(a.update(Msg::BuildGame), Effect::None));
    }
}
