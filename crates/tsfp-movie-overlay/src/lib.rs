//! The movie overlay under `--wgpu-share` (the default variant).
//!
//! The title's own frames reach the window as a shared GPU texture from the wgpu render runtime, which has no movie overlay:
//! the intro movies are pictures the C live compositor letterboxes over its front buffer and forwards to the Bevy mailbox.
//! While that compositor holds an overlay picture, the window must show the mailbox frame, not the shared texture.
//! (Bug: since the shared texture became the default the mailbox was ignored and the movies were never drawn.)

use std::sync::atomic::{AtomicBool, Ordering};

static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Where the picture sprite takes its image from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PictureSource {
    /// The shared GPU texture of the render runtime.
    Shared,
    /// The latest-wins mailbox picture of the C compositor (front plus movie overlay).
    Mailbox,
}

/// The C compositor reports, before each forwarded frame, whether an overlay picture is part of it.
pub fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::Release);
}

/// C ABI of `tsfp_bevy_movie_overlay` (`src/host/bevy_sink.h`), called on the presenter thread.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_bevy_movie_overlay(active: bool) {
    set_active(active);
}

/// True while the forwarded frame carries a movie overlay picture.
pub fn active() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

/// The source to present: the mailbox while a movie overlay is up or when there is no shared texture, otherwise the shared one.
pub fn picture_source(movie_overlay_active: bool, shared_texture_ready: bool) -> PictureSource {
    if movie_overlay_active || !shared_texture_ready {
        PictureSource::Mailbox
    } else {
        PictureSource::Shared
    }
}

/// Point `sprite_image` at the image to show (the shared one, or the mailbox one while a movie overlay is up) and say whether the
/// mailbox picture must be taken into it.
pub fn present<T: Clone + PartialEq>(
    shared: Option<&T>,
    mailbox: &T,
    sprite_image: &mut T,
) -> bool {
    let (source, want) = match (picture_source(active(), shared.is_some()), shared) {
        (PictureSource::Shared, Some(shared)) => (PictureSource::Shared, shared),
        (source, _) => (source, mailbox),
    };
    if *sprite_image != *want {
        *sprite_image = want.clone();
    }
    source == PictureSource::Mailbox
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_overlay_picture_at_the_compositor_is_presented() {
        assert_eq!(picture_source(true, true), PictureSource::Mailbox);
    }

    #[test]
    fn without_an_overlay_the_shared_texture_is_shown() {
        assert_eq!(picture_source(false, true), PictureSource::Shared);
        assert_eq!(picture_source(false, false), PictureSource::Mailbox);
    }

    #[test]
    fn the_compositor_flag_round_trips() {
        set_active(true);
        assert!(active());
        assert_eq!(picture_source(active(), true), PictureSource::Mailbox);
        set_active(false);
        assert!(!active());
        assert_eq!(picture_source(active(), true), PictureSource::Shared);
        // The sprite image follows the flag, and only the movie frame takes the mailbox picture.
        let mut image = "mailbox";
        assert!(!present(Some(&"shared"), &"mailbox", &mut image));
        assert_eq!(image, "shared");
        set_active(true);
        assert!(present(Some(&"shared"), &"mailbox", &mut image));
        assert_eq!(image, "mailbox");
        set_active(false);
    }
}
