// SPDX-License-Identifier: GPL-3.0-or-later
//! SDL3 presenter-thread adapter. The caller must initialize SDL's gamepad
//! subsystem and keep this provider on the thread that pumps SDL events.

use crate::{
    Device,
    provider::{Backend, UnmappedJoystick},
};
use sdl3::sys::{error, events, gamepad, guid, joystick, properties, stdinc};
use std::{
    ffi::{CStr, c_int},
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
};

/// Raw values in controller_config.c's button order, before profile mapping.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sample {
    pub buttons: [bool; 15],
    pub axes: [i16; 6],
}

/// SDL objects and event pumping must stay on the presenter thread.
#[derive(Default)]
pub struct SdlBackend {
    _thread_bound: PhantomData<Rc<()>>,
}

pub struct Handle(NonNull<gamepad::SDL_Gamepad>);

fn guid_string(guid: guid::SDL_GUID) -> String {
    guid.data.iter().map(|b| format!("{b:02x}")).collect()
}

fn error() -> String {
    // SDL owns this thread-local string; copy it before the next SDL call.
    unsafe {
        let p = error::SDL_GetError();
        if p.is_null() {
            "unknown SDL error".into()
        } else {
            CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }
}

impl Backend for SdlBackend {
    type Handle = Handle;
    type State = Sample;
    type Error = String;

    fn prepare(&mut self) {
        unsafe {
            events::SDL_PumpEvents();
            gamepad::SDL_UpdateGamepads();
        }
    }

    fn devices(&mut self) -> Vec<Device> {
        unsafe {
            let mut count = 0;
            let ids = gamepad::SDL_GetGamepads(&mut count);
            if ids.is_null() {
                return Vec::new();
            }
            let mut devices = Vec::new();
            for &id in std::slice::from_raw_parts(ids, count.max(0) as usize) {
                let guid = guid_string(gamepad::SDL_GetGamepadGUIDForID(id));
                devices.push(Device { id: id.0, guid });
            }
            stdinc::SDL_free(ids.cast());
            devices
        }
    }

    fn connected(&mut self, handle: &Handle) -> bool {
        unsafe { gamepad::SDL_GamepadConnected(handle.0.as_ptr()) }
    }

    fn name(&mut self, handle: &Handle) -> String {
        unsafe {
            let name = gamepad::SDL_GetGamepadName(handle.0.as_ptr());
            if name.is_null() {
                "Unnamed controller".into()
            } else {
                CStr::from_ptr(name).to_string_lossy().into_owned()
            }
        }
    }

    fn unmapped(&mut self) -> Vec<UnmappedJoystick> {
        unsafe {
            let mut count = 0;
            let ids = joystick::SDL_GetJoysticks(&mut count);
            if ids.is_null() {
                return Vec::new();
            }
            let mut result = Vec::new();
            for &id in std::slice::from_raw_parts(ids, count.max(0) as usize) {
                if gamepad::SDL_IsGamepad(id) {
                    continue;
                }
                let name = joystick::SDL_GetJoystickNameForID(id);
                let name = if name.is_null() {
                    "unnamed".into()
                } else {
                    CStr::from_ptr(name).to_string_lossy().into_owned()
                };
                let guid = guid_string(joystick::SDL_GetJoystickGUIDForID(id));
                result.push(UnmappedJoystick { name, guid });
            }
            stdinc::SDL_free(ids.cast());
            result
        }
    }

    fn open(&mut self, id: u32, port: usize) -> Result<(Handle, bool), String> {
        unsafe {
            let pad = NonNull::new(gamepad::SDL_OpenGamepad(joystick::SDL_JoystickID(id)))
                .ok_or_else(|| format!("cannot open gamepad: {}", error()))?;
            let _ = gamepad::SDL_SetGamepadPlayerIndex(pad.as_ptr(), port as c_int);
            let rumble = properties::SDL_GetBooleanProperty(
                gamepad::SDL_GetGamepadProperties(pad.as_ptr()),
                c"SDL.joystick.cap.rumble".as_ptr(),
                false,
            );
            Ok((Handle(pad), rumble))
        }
    }

    fn close(&mut self, handle: Handle) {
        unsafe { gamepad::SDL_CloseGamepad(handle.0.as_ptr()) }
    }

    fn sample(&mut self, handle: &Handle, _port: usize) -> Sample {
        // SDL button enum: south/east/west/north, back/guide/start,
        // sticks, shoulders, d-pad; the C profile contract uses another order.
        const BUTTONS: [gamepad::SDL_GamepadButton; 15] = [
            gamepad::SDL_GAMEPAD_BUTTON_SOUTH,
            gamepad::SDL_GAMEPAD_BUTTON_EAST,
            gamepad::SDL_GAMEPAD_BUTTON_WEST,
            gamepad::SDL_GAMEPAD_BUTTON_NORTH,
            gamepad::SDL_GAMEPAD_BUTTON_LEFT_SHOULDER,
            gamepad::SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER,
            gamepad::SDL_GAMEPAD_BUTTON_START,
            gamepad::SDL_GAMEPAD_BUTTON_BACK,
            gamepad::SDL_GAMEPAD_BUTTON_LEFT_STICK,
            gamepad::SDL_GAMEPAD_BUTTON_RIGHT_STICK,
            gamepad::SDL_GAMEPAD_BUTTON_DPAD_UP,
            gamepad::SDL_GAMEPAD_BUTTON_DPAD_DOWN,
            gamepad::SDL_GAMEPAD_BUTTON_DPAD_LEFT,
            gamepad::SDL_GAMEPAD_BUTTON_DPAD_RIGHT,
            gamepad::SDL_GAMEPAD_BUTTON_GUIDE,
        ];
        let mut sample = Sample::default();
        unsafe {
            for (out, button) in sample.buttons.iter_mut().zip(BUTTONS) {
                *out = gamepad::SDL_GetGamepadButton(handle.0.as_ptr(), button);
            }
            for (axis, out) in sample.axes.iter_mut().enumerate() {
                *out = gamepad::SDL_GetGamepadAxis(
                    handle.0.as_ptr(),
                    gamepad::SDL_GamepadAxis::new(axis as c_int),
                );
            }
        }
        sample
    }

    fn rumble(&mut self, handle: &Handle, left: u16, right: u16) -> Result<(), String> {
        let duration = if left != 0 || right != 0 { u32::MAX } else { 0 };
        if unsafe { gamepad::SDL_RumbleGamepad(handle.0.as_ptr(), left, right, duration) } {
            Ok(())
        } else {
            Err(format!("rumble failed: {}", error()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdl3::sys::init;

    #[test]
    fn initialized_backend_can_enumerate_and_pump() {
        unsafe {
            assert!(
                init::SDL_InitSubSystem(init::SDL_INIT_GAMEPAD),
                "{}",
                error()
            );
        }
        let mut backend = SdlBackend::default();
        backend.prepare();
        let _ = backend.devices();
        unsafe {
            init::SDL_QuitSubSystem(init::SDL_INIT_GAMEPAD);
        }
    }
}
