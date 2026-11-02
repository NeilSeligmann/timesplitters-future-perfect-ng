//! Launcher state, messages and view. Long work (identify, install, verify) runs on a worker thread and
//! streams messages back, so the window never blocks.
use crate::mod_archive;
use crate::pack_install;
use crate::settings_view::{self, Edit};
use crate::updater;
use iced::futures::channel::mpsc;
use iced::widget::{
    Space, button, checkbox, column, container, pick_list, row, scrollable, text, text_input,
    toggler,
};
use iced::{Element, Length, Task, Theme};
use std::path::PathBuf;
use tsfp_launcher_core::config::{self, Channel, Config};
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
    fn label(self) -> &'static str {
        match self {
            Tab::Play => "Play",
            Tab::Setup => "Setup",
            Tab::Settings => "Settings",
            Tab::Content => "Content",
            Tab::Mods => "Mods",
            Tab::Verify => "Verify",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Msg {
    Tab(Tab),
    DiscPath(String),
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
    Verified(Vec<Status>),
    GameDir(String),
    Channel(ChannelChoice),
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
    Save,
    Play,
}

/// `pick_list` needs `Display`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelChoice(pub Channel);
impl std::fmt::Display for ChannelChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            Channel::Stable => "Stable",
            Channel::Nightly => "Nightly",
        })
    }
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

/// Run `work` on a thread; it reports through `send` and the stream ends when it returns.
fn background(work: impl FnOnce(&dyn Fn(Msg)) + Send + 'static) -> Task<Msg> {
    let (tx, rx) = mpsc::unbounded();
    std::thread::spawn(move || {
        let send = |m: Msg| {
            let _ = tx.unbounded_send(m);
        };
        work(&send);
    });
    Task::run(rx, |m| m)
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
        let status = install::status(&config.game_dir);
        let tab = if matches!(status[..], [Status::NotInstalled]) {
            Tab::Setup
        } else {
            Tab::Play
        };
        App {
            tab,
            config,
            config_path,
            settings,
            settings_path,
            drafts: Vec::new(),
            disc: String::new(),
            identity: None,
            allow_unknown: false,
            busy: false,
            log: Vec::new(),
            status,
            notice,
            update: None,
        }
    }

    pub fn boot() -> (Self, Task<Msg>) {
        let dir = config::default_config_dir();
        let app = Self::new(
            dir.as_ref().map(|d| d.join("launcher.json")),
            tsfp_settings::user_settings_path(),
        );
        let task = if app.config.check_updates {
            Task::done(Msg::UpdateCheck)
        } else {
            Task::none()
        };
        (app, task)
    }

    pub fn theme(&self) -> Theme {
        Theme::TokyoNight
    }

    pub fn installed(&self) -> bool {
        self.status
            .iter()
            .any(|s| matches!(s, Status::Installed { .. }))
    }

    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Tab(t) => self.tab = t,
            Msg::DiscPath(p) => {
                self.disc = p;
                self.identity = None;
            }
            Msg::BrowseDisc | Msg::BrowseGameDir => {
                let disc = matches!(msg, Msg::BrowseDisc);
                return Task::perform(
                    async move {
                        let d = rfd::AsyncFileDialog::new();
                        let picked = if disc {
                            d.set_title("Choose the disc image")
                                .add_filter("Disc image", &["iso", "xiso", "img", "bin"])
                                .pick_file()
                                .await
                        } else {
                            d.set_title("Choose the game folder").pick_folder().await
                        };
                        picked.map(|h| h.path().to_path_buf())
                    },
                    move |p| Msg::Picked(p, disc),
                );
            }
            Msg::Picked(Some(p), true) => {
                self.disc = p.display().to_string();
                return self.update(Msg::Identify);
            }
            Msg::Picked(Some(p), false) => {
                self.config.game_dir = p;
                self.status = install::status(&self.config.game_dir);
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
                self.identity = Some(r);
            }
            Msg::AllowUnknown(v) => self.allow_unknown = v,
            Msg::Install if !self.busy => {
                self.busy = true;
                self.log.clear();
                let (disc, dir, allow) = (
                    PathBuf::from(&self.disc),
                    self.config.game_dir.clone(),
                    self.allow_unknown,
                );
                return background(move |send| {
                    let r = install::install(&disc, &dir, allow, &mut |s| {
                        send(Msg::Progress(s.clone()))
                    });
                    send(Msg::InstallDone(r.map(|_| ())));
                });
            }
            Msg::Install => {}
            Msg::Progress(s) => self.log.push(describe(&s)),
            Msg::InstallDone(r) => {
                self.busy = false;
                match r {
                    Ok(()) => {
                        self.log.push("Install complete.".into());
                        self.status = install::status(&self.config.game_dir);
                        self.notice = self.save_all();
                    }
                    Err(e) => self.log.push(format!("Install failed: {e}")),
                }
            }
            Msg::Verify if !self.busy => {
                self.busy = true;
                let dir = self.config.game_dir.clone();
                return background(move |send| send(Msg::Verified(install::status(&dir))));
            }
            Msg::Verify => {}
            Msg::Verified(s) => {
                self.busy = false;
                self.status = s;
            }
            Msg::GameDir(d) => {
                self.config.game_dir = PathBuf::from(d);
                self.status = install::status(&self.config.game_dir);
            }
            Msg::Channel(c) => self.config.channel = c.0,
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
            Msg::BrowseMod => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Choose a mod archive")
                            .add_filter("Mod archive", &["zip"])
                            .pick_file()
                            .await
                            .map(|h| h.path().to_path_buf())
                    },
                    Msg::PickedMod,
                );
            }
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
            Msg::Setting(e) => settings_view::apply(&mut self.settings, &mut self.drafts, e),
            Msg::Save => self.notice = self.save_all(),
            Msg::Play => {
                self.notice = match game_executable() {
                    Some(exe) => match std::process::Command::new(&exe)
                        .arg("--game-dir")
                        .arg(&self.config.game_dir)
                        .spawn()
                    {
                        Ok(_) => "Game started.".into(),
                        Err(e) => format!("{}: {e}", exe.display()),
                    },
                    None => "The game is not available for this platform yet.".into(),
                };
            }
        }
        Task::none()
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

    pub fn view(&self) -> Element<'_, Msg> {
        let nav = Tab::ALL
            .iter()
            .fold(column![].spacing(4).width(150), |c, t| {
                let b = button(text(t.label()).width(Length::Fill)).width(Length::Fill);
                c.push(if *t == self.tab {
                    b.style(button::primary)
                } else {
                    b.style(button::text).on_press(Msg::Tab(*t))
                })
            });
        let body = match self.tab {
            Tab::Play => self.play(),
            Tab::Setup => self.setup(),
            Tab::Settings => self.settings_tab(),
            Tab::Content => self.content(),
            Tab::Mods => self.mods(),
            Tab::Verify => self.verify(),
        };
        let main = column![
            scrollable(container(body).padding(16)).height(Length::Fill),
            text(&self.notice).size(13),
        ]
        .spacing(8)
        .width(Length::Fill);
        row![container(nav).padding(12), main]
            .spacing(8)
            .padding(8)
            .into()
    }

    fn play(&self) -> Element<'_, Msg> {
        let can_play = self.installed() && game_executable().is_some();
        let hint = if !self.installed() {
            "Run Setup first."
        } else if game_executable().is_none() {
            "The game is not available for this platform yet."
        } else {
            ""
        };
        let banner: Element<'_, Msg> = match &self.update {
            Some(o) => row![
                text(format!("Launcher {} is available.", o.version)),
                button("Update").on_press_maybe((!self.busy).then_some(Msg::UpdateApply)),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center)
            .into(),
            None => Space::new().height(0).into(),
        };
        column![
            banner,
            text("TimeSplitters: Future Perfect NG").size(28),
            text(format!("Launcher {}", env!("CARGO_PKG_VERSION"))).size(13),
            status_list(&self.status),
            button(text("Play").size(22))
                .padding([10, 40])
                .on_press_maybe(can_play.then_some(Msg::Play)),
            text(hint),
        ]
        .spacing(14)
        .into()
    }

    fn setup(&self) -> Element<'_, Msg> {
        let ident: Element<'_, Msg> = match &self.identity {
            None => {
                text("Choose your own copy of the game: an Xbox disc image or an extracted folder.")
                    .into()
            }
            Some(Ok(id)) => text(format!(
                "{} ({} files, {})",
                id.verdict(),
                id.file_count,
                id.layout
            ))
            .into(),
            Some(Err(e)) => text(format!("Could not read it: {e}")).into(),
        };
        let identified = matches!(&self.identity, Some(Ok(i)) if i.xbe_sha256.is_some());
        let unknown = matches!(&self.identity, Some(Ok(i)) if i.known.is_none());
        let can_install = identified && (!unknown || self.allow_unknown) && !self.busy;
        let mut c = column![
            text("1. Disc image").size(20),
            row![
                text_input("Path to disc image or folder", &self.disc)
                    .on_input(Msg::DiscPath)
                    .on_submit(Msg::Identify),
                button("Browse").on_press_maybe((!self.busy).then_some(Msg::BrowseDisc)),
            ]
            .spacing(8),
            ident,
        ]
        .spacing(10);
        if unknown {
            c = c.push(
                checkbox(self.allow_unknown)
                    .label("Install anyway (untested dump)")
                    .on_toggle(Msg::AllowUnknown),
            );
        }
        c = c.push(text("2. Game folder").size(20)).push(
            row![
                text_input("Game folder", &self.config.game_dir.display().to_string())
                    .on_input(Msg::GameDir),
                button("Browse").on_press_maybe((!self.busy).then_some(Msg::BrowseGameDir)),
            ]
            .spacing(8),
        );
        c = c
            .push(text("3. Install").size(20))
            .push(button("Extract and verify").on_press_maybe(can_install.then_some(Msg::Install)));
        for line in &self.log {
            c = c.push(text(line).size(13));
        }
        c.push(text("4. Optional: high-quality packs in the Content tab.").size(14))
            .into()
    }

    fn settings_tab(&self) -> Element<'_, Msg> {
        column![
            text("Launcher").size(20),
            row![
                text("Update channel").width(Length::Fixed(240.0)),
                pick_list(
                    [
                        ChannelChoice(Channel::Stable),
                        ChannelChoice(Channel::Nightly)
                    ],
                    Some(ChannelChoice(self.config.channel)),
                    Msg::Channel
                ),
            ]
            .spacing(12),
            row![
                text("Check for updates").width(Length::Fixed(240.0)),
                toggler(self.config.check_updates).on_toggle(Msg::CheckUpdates),
                button("Check now").on_press(Msg::UpdateCheck),
            ]
            .spacing(12),
            text("Game (shared with the in-game menu)").size(20),
            settings_view::view(&self.settings, &self.drafts, Msg::Setting),
            button("Save").on_press(Msg::Save),
        ]
        .spacing(10)
        .into()
    }

    fn content(&self) -> Element<'_, Msg> {
        packs::CATALOG
            .iter()
            .fold(
                column![text("Optional content").size(20)].spacing(10),
                |c, p| {
                    let state = match packs::state(&self.config.game_dir, p) {
                        packs::PackState::Unavailable => "coming soon",
                        packs::PackState::NotInstalled => "not installed",
                        packs::PackState::Installed => "installed",
                    };
                    let action: Element<'_, Msg> = match packs::state(&self.config.game_dir, p) {
                        packs::PackState::Unavailable => Space::new().into(),
                        packs::PackState::NotInstalled => button("Install")
                            .on_press_maybe((!self.busy).then_some(Msg::PackInstall(p.id)))
                            .into(),
                        packs::PackState::Installed => row![
                            button("Repair")
                                .on_press_maybe((!self.busy && p.manifest_url.is_some()).then_some(Msg::PackInstall(p.id))),
                            button("Remove").style(button::danger).on_press(Msg::PackRemove(p.id)),
                        ]
                        .spacing(6)
                        .into(),
                    };
                    c.push(
                        row![
                            column![
                                text(format!("{} ({state})", p.title)).size(16),
                                text(p.description).size(13),
                            ]
                            .width(Length::Fill),
                            action,
                        ]
                        .align_y(iced::Alignment::Center),
                    )
                },
            )
            .push(text("Packs replace original assets and are labelled as differing from the original game.").size(12))
            .into()
    }

    fn mods(&self) -> Element<'_, Msg> {
        let (mut found, broken) = mods::scan(&self.config.game_dir.join("mods"));
        // Enabled mods in load order first, then the rest by id.
        let rank = |id: &str| {
            self.config
                .enabled_mods
                .iter()
                .position(|x| x == id)
                .unwrap_or(usize::MAX)
        };
        found.sort_by(|a, b| rank(&a.id).cmp(&rank(&b.id)).then_with(|| a.id.cmp(&b.id)));
        let mut c = column![
            row![
                text("Mods").size(20).width(Length::Fill),
                button("Install mod...").on_press(Msg::BrowseMod),
            ]
            .align_y(iced::Alignment::Center),
            text("Later mods in the load order win when two mods replace the same file.").size(12),
            text(format!(
                "Folder: {}",
                self.config.game_dir.join("mods").display()
            ))
            .size(12),
        ]
        .spacing(8);
        if found.is_empty() && broken.is_empty() {
            c = c.push(text("No mods installed."));
        }
        for m in found {
            let on = self.config.enabled_mods.contains(&m.id);
            let tag = if m.affects_gameplay {
                " [changes gameplay]"
            } else {
                ""
            };
            let id = m.id.clone();
            let position = self
                .config
                .enabled_mods
                .iter()
                .position(|x| x == &m.id)
                .map_or(String::new(), |i| format!("#{} ", i + 1));
            c = c.push(
                row![
                    checkbox(on)
                        .label(format!(
                            "{position}{} {}{tag}: {}",
                            m.name, m.version, m.description
                        ))
                        .on_toggle(move |v| Msg::ToggleMod(id.clone(), v))
                        .width(Length::Fill),
                    button("Up").on_press_maybe(on.then(|| Msg::MoveMod(m.id.clone(), -1))),
                    button("Down").on_press_maybe(on.then(|| Msg::MoveMod(m.id.clone(), 1))),
                    button("Remove")
                        .style(button::danger)
                        .on_press(Msg::RemoveMod(m.id.clone())),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            );
        }
        for b in broken {
            c = c.push(text(format!("Broken: {b}")).size(12));
        }
        let (found, _) = mods::scan(&self.config.game_dir.join("mods"));
        match mods::resolve(&found, &self.config.enabled_mods) {
            Ok(conflicts) => {
                for x in conflicts {
                    c = c.push(
                        text(format!(
                            "{}: {} overrides {}",
                            x.file,
                            x.winner,
                            x.shadowed.join(", ")
                        ))
                        .size(12),
                    );
                }
            }
            Err(e) => c = c.push(text(e).size(12)),
        }
        c.push(Space::new().height(8))
            .push(button("Save").on_press(Msg::Save))
            .into()
    }

    fn verify(&self) -> Element<'_, Msg> {
        column![
            text("Verify installed files").size(20),
            text("Checks every installed file against its recorded SHA-256."),
            button(if self.busy {
                "Working..."
            } else {
                "Verify now"
            })
            .on_press_maybe((!self.busy).then_some(Msg::Verify)),
            status_list(&self.status),
        ]
        .spacing(10)
        .into()
    }
}

fn describe(s: &Step) -> String {
    match s {
        Step::Identified(v) => format!("Disc: {v}"),
        Step::Extracted(n) => format!("Extracted {n} files"),
        Step::Converted { outputs, lossless } => {
            format!("Converted {outputs} assets ({lossless} lossless)")
        }
        Step::Verified(n) => format!("Verified {n} files"),
    }
}

fn status_list(s: &[Status]) -> Element<'_, Msg> {
    s.iter()
        .fold(column![].spacing(4), |c, st| {
            c.push(text(match st {
                Status::NotInstalled => "Not installed.".to_string(),
                Status::Installed { id, files } => {
                    format!("{id}: installed, {files} files verified")
                }
                Status::Damaged { id, reason } => {
                    format!("{id}: damaged ({reason}). Run Setup again to repair.")
                }
            }))
        })
        .into()
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
