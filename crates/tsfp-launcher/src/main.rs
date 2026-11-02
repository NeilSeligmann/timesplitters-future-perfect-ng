//! TSFP launcher (T4332). A thin iced view over `tsfp-launcher-core`; decision docs/decisions/launcher.md.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod mod_archive;
mod settings_view;
mod updater;

fn main() -> iced::Result {
    iced::application(app::App::boot, app::App::update, app::App::view)
        .title("TimeSplitters: Future Perfect NG")
        .theme(app::App::theme)
        .window_size((960.0, 640.0))
        .run()
}
