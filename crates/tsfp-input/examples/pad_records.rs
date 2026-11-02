// SPDX-License-Identifier: GPL-3.0-or-later
//! T2160: pad route through the whole stack. Reads a pad script body on stdin (`N [NAME=V ...]`, each entry a full state, no
//! comments, the format `tools/t2116_route_build.py` expands to), plays it as a gamepad through the router, `PlayerInput` and the
//! shim, and prints per poll the pad record words `held previous pressed released LX LY RX RY` as hex.
//! Compared with an independent Python conversion by `tools/t2160_shim_check.py`.
use std::io::Read;
use tsfp_input::router::{DeviceEvent, PadAxis, PadButton, Router, RouterConfig};
use tsfp_input::shim::{PadRecord, xinput_from_player};

const BUTTONS: [(&str, PadButton); 12] = [
    ("A", PadButton::South),
    ("B", PadButton::East),
    ("X", PadButton::West),
    ("Y", PadButton::North),
    ("BACK", PadButton::Select),
    ("START", PadButton::Start),
    ("LTHUMB", PadButton::LeftThumb),
    ("RTHUMB", PadButton::RightThumb),
    ("BLACK", PadButton::LeftBumper),
    ("WHITE", PadButton::RightBumper),
    ("UP", PadButton::DPadUp),
    ("DOWN", PadButton::DPadDown),
];
const MORE: [(&str, PadButton); 2] = [
    ("LEFT", PadButton::DPadLeft),
    ("RIGHT", PadButton::DPadRight),
];

fn main() {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .expect("read stdin");
    let mut router = Router::new(RouterConfig::default());
    router.feed(DeviceEvent::PadConnected(0));
    let mut record = PadRecord::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let mut words = line.split_whitespace();
        let polls: u32 = words
            .next()
            .and_then(|word| word.parse().ok())
            .expect("poll count");
        let mut down = [false; 14];
        let mut sticks = [0.0f32; 4];
        let mut triggers = [0.0f32; 2];
        for word in words {
            let (name, value) = word.split_once('=').unwrap_or((word, "255")); // a bare name is a full press
            let value: i32 = value.parse().expect("integer value");
            if let Some(at) = BUTTONS
                .iter()
                .chain(MORE.iter())
                .position(|(n, _)| *n == name)
            {
                down[at] = value > 0x3C;
                continue;
            }
            match name {
                "LX" => sticks[0] = value as f32 / 32767.0,
                "LY" => sticks[1] = value as f32 / 32767.0,
                "RX" => sticks[2] = value as f32 / 32767.0,
                "RY" => sticks[3] = value as f32 / 32767.0,
                "LT" => triggers[0] = (value * 128) as f32 / 32767.0,
                "RT" => triggers[1] = (value * 128) as f32 / 32767.0,
                other => panic!("unknown pad name {other}"),
            }
        }
        for (at, (_, button)) in BUTTONS.iter().chain(MORE.iter()).enumerate() {
            router.feed(DeviceEvent::PadButton {
                pad: 0,
                button: *button,
                value: f32::from(u8::from(down[at])),
            });
        }
        for (axis, value) in [
            PadAxis::LeftX,
            PadAxis::LeftY,
            PadAxis::RightX,
            PadAxis::RightY,
        ]
        .into_iter()
        .zip(sticks)
        {
            router.feed(DeviceEvent::PadAxis {
                pad: 0,
                axis,
                value,
            });
        }
        router.feed(DeviceEvent::PadAxis {
            pad: 0,
            axis: PadAxis::LeftTrigger,
            value: triggers[0],
        });
        router.feed(DeviceEvent::PadAxis {
            pad: 0,
            axis: PadAxis::RightTrigger,
            value: triggers[1],
        });
        for _ in 0..polls {
            let input = router.tick()[0].expect("pad slot");
            record.update(&xinput_from_player(&input));
            let w = &record.words;
            println!(
                "{:x} {:x} {:x} {:x} {:x} {:x} {:x} {:x}",
                w[3], w[2], w[4], w[5], w[7], w[8], w[9], w[10]
            );
        }
    }
}
