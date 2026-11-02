// SPDX-License-Identifier: GPL-3.0-or-later
//! Embedding-host policy of the passive stream module: the notes and providers the host installs quiescent at startup
//! (`dsound_stream_set_*`), read back by the handlers. Accessors copy the whole record, so no handler holds this lock
//! across a callback.
use crate::ffi::*;
use crate::{lock_streams, unlock_streams};
use std::sync::Mutex;

#[derive(Clone, Copy)]
pub struct Config {
    pub completion: bool,
    pub accepted_frequency_sets: u64,
    pub pause_note: Option<PauseNote>,
    pub routing_note: Option<RoutingNote>,
    pub frequency_note: Option<FrequencyNote>,
    pub format_note: Option<FormatNote>,
    pub frequency_control_word: Option<ControlWordFn>,
    pub irql_provider: Option<IrqlProvider>,
    pub fatal_handler: Option<FatalFn>,
    pub public_route: Option<PublicRoute>,
    pub reset_route: Option<ResetRoute>,
}

static CONFIG: Mutex<Config> = Mutex::new(Config {
    completion: false,
    accepted_frequency_sets: 0,
    pause_note: None,
    routing_note: None,
    frequency_note: None,
    format_note: None,
    frequency_control_word: None,
    irql_provider: None,
    fatal_handler: None,
    public_route: None,
    reset_route: None,
});

pub fn get() -> Config {
    *CONFIG
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn update(edit: impl FnOnce(&mut Config)) {
    edit(
        &mut CONFIG
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
}

pub fn enabled() -> bool {
    // SAFETY: plain query of the Rust create policy.
    unsafe { tsfp_audio_stream_create_enabled_rust() }
}

/// Current IRQL when the provider can report it.
pub fn irql(config: &Config) -> Option<u8> {
    let provider = config.irql_provider?;
    let mut irql = 0u8;
    // SAFETY: the provider only reports the current IRQL (dsound_stream.h contract).
    unsafe { provider(&mut irql) }.then_some(irql)
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_set_enabled(value: bool) {
    lock_streams();
    // SAFETY: called with the stream lock held, as the C setter did.
    unsafe { tsfp_audio_stream_create_set_enabled_rust(value) };
    unlock_streams();
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_set_completion(value: bool, note: Option<PauseNote>) {
    update(|c| {
        c.completion = value;
        c.pause_note = if value { note } else { None };
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_set_routing_note(note: Option<RoutingNote>) {
    update(|c| c.routing_note = note);
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_frequency_set_count() -> u64 {
    get().accepted_frequency_sets
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_set_format_note(note: Option<FormatNote>) {
    update(|c| c.format_note = note);
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_set_frequency_note(
    note: Option<FrequencyNote>,
    control_word: Option<ControlWordFn>,
) {
    update(|c| {
        c.frequency_note = note;
        c.frequency_control_word = control_word;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_set_irql_provider(provider: Option<IrqlProvider>) {
    update(|c| c.irql_provider = provider);
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_set_fatal(fatal: Option<FatalFn>) {
    update(|c| c.fatal_handler = fatal);
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_set_extension(
    route: Option<PublicRoute>,
    reset: Option<ResetRoute>,
) {
    update(|c| {
        c.public_route = route;
        c.reset_route = reset;
    });
}
