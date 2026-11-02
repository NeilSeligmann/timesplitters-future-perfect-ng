// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline model of the presentation sink's stretch rate controller.

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Controller {
    pub fill: usize,
    pub need: usize,
    pub smooth_window: usize,
    pub ema: f64,
    pub ema_valid: bool,
    pub low: usize,
    pub floor: u32,
    pub ratio: u32,
    pub engage: u32,
    pub leave: u32,
    pub down: u32,
    pub up: u32,
    pub engagements: u64,
    pub panic_pulls: u64,
    pub max_step: u64,
}

impl Controller {
    pub fn update(&mut self, pull: usize) -> u32 {
        let panic = self.need.wrapping_mul(2);
        let fill = if self.smooth_window != 0 && self.ema_valid && self.fill >= panic {
            self.ema
        } else {
            self.fill as f64
        };
        let mut wanted = 65536;
        if fill < self.low as f64 {
            let above = if fill > self.need as f64 {
                fill - self.need as f64
            } else {
                0.0
            };
            let span = if self.low > self.need {
                (self.low - self.need) as f64
            } else {
                1.0
            };
            wanted = ((above * 65536.0 / span) as u32).max(self.floor);
        }
        let current = self.ratio;
        if current == 65536 && wanted < 65536 && (wanted <= self.engage || self.fill < panic) {
            self.engagements = self.engagements.wrapping_add(1);
        }
        if self.fill < panic {
            self.panic_pulls = self.panic_pulls.wrapping_add(1);
        }
        if self.fill < panic || self.down >= 65536 {
            if wanted < current {
                self.ratio = current - (current - wanted) / 2;
            } else {
                self.ratio = current.wrapping_add(wanted.wrapping_sub(current).wrapping_add(7) / 8);
            }
        } else {
            let scale = if pull > 0 { pull as u64 } else { 1024 };
            let down = ((self.down as u64).wrapping_mul(scale) / 1024) as u32;
            let up = ((self.up as u64).wrapping_mul(scale) / 1024) as u32;
            if current == 65536 && wanted > self.engage {
                wanted = 65536;
            }
            self.ratio = if wanted < current {
                if current - wanted > down.max(1) {
                    current - down.max(1)
                } else {
                    wanted
                }
            } else if wanted - current > up.max(1) {
                current.wrapping_add(up.max(1))
            } else {
                wanted
            };
            self.max_step = self.max_step.max(self.ratio.abs_diff(current) as u64);
        }
        let recovered = if self.down >= 65536 {
            self.ratio >= wanted
        } else {
            wanted == 65536
        };
        if self.ratio > self.leave && recovered {
            self.ratio = 65536;
        }
        self.ratio
    }
}
