//! TSFP launcher (T4332, Slint UI T4349). `ui/launcher.slint` is the view; `controller` holds all state.
//! Every UI callback becomes a `Msg`; the returned `Effect` runs here (worker thread or native dialog), and the
//! window is refreshed from the controller after each message.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod controller;
mod mod_archive;
mod pack_install;
mod settings_model;
mod updater;

slint::include_modules!();

use controller::{App, Effect, Msg, StepState, Tab};
use settings_model::Edit;
use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent};
use slint::{ComponentHandle, Model, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use tsfp_launcher_core::config::Channel;
use tsfp_launcher_core::{mods, packs};

type Handler = Rc<dyn Fn(Msg)>;

thread_local! {
    /// Main-thread entry point for messages posted by worker threads.
    static DISPATCH: RefCell<Option<Handler>> = RefCell::new(None);
}

/// Deliver `msg` on the UI thread (safe to call from any thread).
fn post(msg: Msg) {
    let _ = slint::invoke_from_event_loop(move || {
        let handler = DISPATCH.with(|d| d.borrow().clone());
        if let Some(handler) = handler {
            handler(msg);
        }
    });
}

fn run(effect: Effect) {
    match effect {
        Effect::None => {}
        Effect::Background(work) => {
            std::thread::spawn(move || work(&post));
        }
        Effect::PickDisc => {
            let picked = rfd::FileDialog::new()
                .set_title("Choose the disc image")
                .add_filter("Disc image", &["iso", "xiso", "img", "bin"])
                .pick_file();
            post(Msg::Picked(picked, true));
        }
        Effect::PickGameDir => {
            let picked = rfd::FileDialog::new()
                .set_title("Choose the game folder")
                .pick_folder();
            post(Msg::Picked(picked, false));
        }
        Effect::PickMod => {
            let picked = rfd::FileDialog::new()
                .set_title("Choose a mod archive")
                .add_filter("Mod archive", &["zip"])
                .pick_file();
            post(Msg::PickedMod(picked));
        }
    }
}

/// Replace a model's rows, touching only rows that changed so focused editors keep their state.
fn update_model<T: Clone + PartialEq + 'static>(model: &Rc<VecModel<T>>, rows: Vec<T>) {
    if model.row_count() != rows.len() {
        model.set_vec(rows);
        return;
    }
    for (i, row) in rows.into_iter().enumerate() {
        if model.row_data(i).as_ref() != Some(&row) {
            model.set_row_data(i, row);
        }
    }
}

struct Models {
    status: Rc<VecModel<SharedString>>,
    steps: Rc<VecModel<InstallStepRow>>,
    settings: Rc<VecModel<SettingRow>>,
    packs: Rc<VecModel<PackRow>>,
    mods: Rc<VecModel<ModRow>>,
    notes: Rc<VecModel<SharedString>>,
}

impl Models {
    fn attach(ui: &AppWindow) -> Self {
        let m = Models {
            status: Rc::new(VecModel::default()),
            steps: Rc::new(VecModel::default()),
            settings: Rc::new(VecModel::default()),
            packs: Rc::new(VecModel::default()),
            mods: Rc::new(VecModel::default()),
            notes: Rc::new(VecModel::default()),
        };
        ui.set_status_lines(ModelRc::from(m.status.clone()));
        ui.set_install_steps(ModelRc::from(m.steps.clone()));
        ui.set_settings(ModelRc::from(m.settings.clone()));
        ui.set_packs(ModelRc::from(m.packs.clone()));
        ui.set_mods(ModelRc::from(m.mods.clone()));
        ui.set_mod_notes(ModelRc::from(m.notes.clone()));
        m
    }
}

fn strings(v: impl IntoIterator<Item = String>) -> Vec<SharedString> {
    v.into_iter().map(SharedString::from).collect()
}

fn sync(ui: &AppWindow, models: &Models, app: &App) {
    ui.set_page(app.tab.index() as i32);
    ui.set_busy(app.busy);
    ui.set_notice(app.notice.clone().into());
    ui.set_update_version(
        app.update
            .as_ref()
            .map_or(String::new(), |o| o.version.clone())
            .into(),
    );

    let installed = app.installed();
    if installed
        && !ui.get_has_logo()
        && let Some(logo) = tsfp_launcher_core::branding::logo(&app.config.game_dir)
    {
        let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
            &logo.rgba,
            logo.width,
            logo.height,
        );
        ui.set_logo(slint::Image::from_rgba8(buffer));
        ui.set_has_logo(true);
    }
    ui.set_installed(installed);
    let play = app.play_view();
    ui.set_can_play(play.action == 1);
    ui.set_play_hint(play.hint.into());
    ui.set_play_label(play.label.into());
    ui.set_play_detail(play.detail.into());
    ui.set_play_action(play.action);
    update_model(
        &models.status,
        strings(app.status.iter().map(controller::status_text)),
    );

    // Setup
    ui.set_disc_path(app.disc.clone().into());
    let (text, tone, unknown) = match &app.identity {
        None => (String::new(), 0, false),
        Some(Ok(id)) if id.known.is_some() => (
            format!("{} ({} files)", id.verdict(), id.file_count),
            1,
            false,
        ),
        Some(Ok(id)) if id.xbe_sha256.is_some() => (id.verdict(), 2, true),
        Some(Ok(id)) => (id.verdict(), 3, false),
        Some(Err(e)) => (format!("Could not read it: {e}"), 3, false),
    };
    ui.set_identity_text(text.into());
    ui.set_identity_tone(tone);
    ui.set_identity_unknown(unknown);
    ui.set_allow_unknown(app.allow_unknown);
    ui.set_game_dir(app.config.game_dir.display().to_string().into());
    let identified = matches!(&app.identity, Some(Ok(i)) if i.xbe_sha256.is_some());
    ui.set_can_install(identified && (!unknown || app.allow_unknown) && !app.busy);
    ui.set_installing(app.installing);
    ui.set_disc_note(controller::disc_note(&app.disc).into());
    ui.set_drag_hover(app.drag_hover);
    let steps = app
        .steps
        .iter()
        .map(|s| InstallStepRow {
            label: s.label.into(),
            detail: s.detail.clone().into(),
            state: match s.state {
                StepState::Pending => 0,
                StepState::Running => 1,
                StepState::Done => 2,
                StepState::Failed => 3,
            },
            progress: s.progress,
        })
        .collect();
    update_model(&models.steps, steps);
    ui.set_install_overall(controller::overall(&app.steps));
    match app.verify_progress {
        Some((done, total)) => {
            ui.set_verify_fraction(if total == 0 {
                0.0
            } else {
                done as f32 / total as f32
            });
            ui.set_verify_detail(
                format!(
                    "{} of {} files checked",
                    controller::count(done),
                    controller::count(total)
                )
                .into(),
            );
        }
        None => ui.set_verify_fraction(-1.0),
    }

    // Settings
    ui.set_channel_index(matches!(app.config.channel, Channel::Nightly) as i32);
    ui.set_check_updates(app.config.check_updates);
    let rows = settings_model::rows(&app.settings, app.drafts())
        .into_iter()
        .map(|r| SettingRow {
            header: r.header,
            id: r.id.into(),
            label: r.label.into(),
            kind: r.kind.into(),
            checked: r.checked,
            text: r.text.into(),
            options: ModelRc::new(VecModel::from(strings(r.options))),
            selected: r.selected,
            note: r.note.into(),
        })
        .collect();
    update_model(&models.settings, rows);

    // Content
    let pack_rows = packs::CATALOG
        .iter()
        .map(|p| PackRow {
            id: p.id.into(),
            title: p.title.into(),
            description: p.description.into(),
            state: match packs::state(&app.config.game_dir, p) {
                packs::PackState::Unavailable => 0,
                packs::PackState::NotInstalled => 1,
                packs::PackState::Installed => 2,
            },
        })
        .collect();
    update_model(&models.packs, pack_rows);

    // Mods: enabled ones in load order first, then the rest by id.
    let mods_dir = app.config.game_dir.join("mods");
    ui.set_mods_folder(mods_dir.display().to_string().into());
    let (mut found, broken) = mods::scan(&mods_dir);
    let order = &app.config.enabled_mods;
    let rank = |id: &str| order.iter().position(|x| x == id).unwrap_or(usize::MAX);
    found.sort_by(|a, b| rank(&a.id).cmp(&rank(&b.id)).then_with(|| a.id.cmp(&b.id)));
    let mod_rows = found
        .iter()
        .map(|m| ModRow {
            id: m.id.clone().into(),
            name: m.name.clone().into(),
            version: m.version.clone().into(),
            description: m.description.clone().into(),
            enabled: order.contains(&m.id),
            position: rank(&m.id).saturating_add(1).min(i32::MAX as usize) as i32,
            gameplay: m.affects_gameplay,
        })
        .collect();
    update_model(&models.mods, mod_rows);
    let mut notes: Vec<String> = broken.into_iter().map(|b| format!("Broken: {b}")).collect();
    match mods::resolve(&found, order) {
        Ok(conflicts) => notes.extend(conflicts.into_iter().map(|x| {
            format!(
                "{}: {} overrides {}",
                x.file,
                x.winner,
                x.shadowed.join(", ")
            )
        })),
        Err(e) => notes.push(e),
    }
    update_model(&models.notes, strings(notes));
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    ui.set_version(env!("CARGO_PKG_VERSION").into());
    let models = Rc::new(Models::attach(&ui));
    let (app, first) = App::boot();
    let state = Rc::new(RefCell::new(app));

    let dispatch: Handler = {
        let (weak, state, models) = (ui.as_weak(), state.clone(), models.clone());
        Rc::new(move |msg| {
            let effect = state.borrow_mut().update(msg);
            if let Some(ui) = weak.upgrade() {
                sync(&ui, &models, &state.borrow());
            }
            run(effect);
        })
    };
    DISPATCH.with(|d| *d.borrow_mut() = Some(dispatch.clone()));

    macro_rules! on {
        ($setter:ident, || $msg:expr) => {{
            let d = dispatch.clone();
            ui.$setter(move || d($msg));
        }};
        ($setter:ident, |$($arg:ident),*| $msg:expr) => {{
            let d = dispatch.clone();
            ui.$setter(move |$($arg),*| d($msg));
        }};
    }
    on!(on_navigate, |i| Msg::Tab(
        Tab::ALL[(i as usize).min(Tab::ALL.len() - 1)]
    ));
    on!(on_browse_disc, || Msg::BrowseDisc);
    // Typed or pasted paths are identified once the input pauses and the path exists (no Enter or Browse needed).
    let identify_timer = Rc::new(Timer::default());
    {
        let (d, timer, state) = (dispatch.clone(), identify_timer.clone(), state.clone());
        ui.on_disc_edited(move |t| {
            d(Msg::DiscPath(t.to_string()));
            let (d, state) = (d.clone(), state.clone());
            timer.start(
                TimerMode::SingleShot,
                Duration::from_millis(500),
                move || {
                    let ready = {
                        let app = state.borrow();
                        !app.disc.is_empty()
                            && std::path::Path::new(&app.disc).exists()
                            && app.identity.is_none()
                    };
                    if ready {
                        d(Msg::Identify);
                    }
                },
            );
        });
    }
    on!(on_identify, || Msg::Identify);
    on!(on_allow_unknown_changed, |v| Msg::AllowUnknown(v));
    on!(on_browse_game_dir, || Msg::BrowseGameDir);
    on!(on_game_dir_edited, |t| Msg::GameDir(t.to_string()));
    on!(on_install, || Msg::Install);
    on!(on_verify, || Msg::Verify);
    on!(on_play, || Msg::Play);
    on!(on_build_game, || Msg::BuildGame);
    on!(on_cancel_build, || Msg::CancelBuild);
    on!(on_check_update, || Msg::UpdateCheck);
    on!(on_apply_update, || Msg::UpdateApply);
    on!(on_channel_changed, |i| Msg::Channel(if i == 1 {
        Channel::Nightly
    } else {
        Channel::Stable
    }));
    on!(on_check_updates_changed, |v| Msg::CheckUpdates(v));
    on!(on_setting_bool, |id, v| Msg::Setting(Edit::Bool(
        schema_id(&id),
        v
    )));
    on!(on_setting_choice, |id, v| Msg::Setting(Edit::Choice(
        schema_id(&id),
        v.to_string()
    )));
    on!(on_setting_text, |id, v| Msg::Setting(Edit::Text(
        schema_id(&id),
        v.to_string()
    )));
    on!(on_save, || Msg::Save);
    on!(on_pack_install, |id| Msg::PackInstall(pack_id(&id)));
    on!(on_pack_remove, |id| Msg::PackRemove(pack_id(&id)));
    on!(on_install_mod, || Msg::BrowseMod);
    on!(on_mod_toggle, |id, v| Msg::ToggleMod(id.to_string(), v));
    on!(on_mod_move, |id, delta| Msg::MoveMod(
        id.to_string(),
        delta as isize
    ));
    on!(on_mod_remove, |id| Msg::RemoveMod(id.to_string()));

    // Files dragged onto the window: winit reports hover and drop; Slint itself has no file drop.
    ui.window().on_winit_window_event(|_, event| {
        match event {
            WindowEvent::HoveredFile(_) => post(Msg::DragHover(true)),
            WindowEvent::HoveredFileCancelled => post(Msg::DragHover(false)),
            WindowEvent::DroppedFile(path) => post(Msg::Dropped(path.clone())),
            _ => {}
        }
        EventResult::Propagate
    });

    sync(&ui, &models, &state.borrow());
    for msg in first {
        dispatch(msg);
    }
    ui.run()
}

/// Map a UI string back to the schema's static id (unknown ids map to "" and are ignored by `Settings::set`).
fn schema_id(id: &str) -> &'static str {
    tsfp_settings::SCHEMA
        .iter()
        .find(|s| s.id == id)
        .map_or("", |s| s.id)
}

fn pack_id(id: &str) -> &'static str {
    packs::CATALOG
        .iter()
        .find(|p| p.id == id)
        .map_or("", |p| p.id)
}
