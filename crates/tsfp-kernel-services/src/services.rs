// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    ACCESS_VIOLATION, Answer, Effect, GuestMemory, INSUFFICIENT_RESOURCES, INVALID_PARAMETER,
    KernelServices, ORDINALS, SUCCESS, crypto, debug, settings::Settings,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvPack {
    Composite,
    Svideo,
    Hdtv,
}
impl AvPack {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "composite" => Some(Self::Composite),
            "svideo" => Some(Self::Svideo),
            "hdtv" => Some(Self::Hdtv),
            _ => None,
        }
    }
    pub fn code(self) -> u32 {
        match self {
            Self::Composite => 1,
            Self::Svideo => 6,
            Self::Hdtv => 4,
        }
    }
    pub fn smc(self) -> u32 {
        match self {
            Self::Composite => 6,
            Self::Svideo => 4,
            Self::Hdtv => 1,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayRequest {
    pub register_base: u32,
    pub mode: u32,
    pub format: u32,
    pub pitch: u32,
    pub frame_buffer: u32,
    pub calls: u32,
}
#[derive(Debug, Default)]
pub struct AvState {
    pub pack: Option<AvPack>,
    pub standard: u32,
    pub widescreen: bool,
    pub saved_data: u32,
    pub display: Option<DisplayRequest>,
    pub fabricated: u32,
    pub refused: u32,
    pub ignored: u32,
}
impl AvState {
    pub fn reset(&mut self) {
        let widescreen = self.widescreen;
        *self = Self {
            widescreen,
            ..Self::default()
        };
    }
    pub fn configure(&mut self, pack: AvPack, region: u32) -> bool {
        let standard = if region & 1 != 0 {
            0x100
        } else if region & 2 != 0 {
            0x200
        } else if region & 4 != 0 {
            0x300
        } else {
            return false;
        };
        self.pack = Some(pack);
        self.standard = standard;
        true
    }
    pub fn flags(&self) -> u32 {
        let mut flags = if self.standard == 0x300 {
            0x800000
        } else {
            0x400000
        };
        if self.pack == Some(AvPack::Hdtv) && self.standard != 0x300 {
            flags |= 0x80000;
        }
        if self.widescreen {
            flags |= 0x10000;
        }
        flags
    }
    pub fn capabilities(&self) -> u32 {
        self.pack
            .map_or(0, |p| p.code() | self.standard | self.flags())
    }
    pub fn smc(&self) -> u32 {
        self.pack.map_or(0, AvPack::smc)
    }
    pub fn install_settings(&self, settings: &mut Settings) -> usize {
        if self.pack.is_none() {
            return 0;
        }
        usize::from(settings.set(0x103, &self.standard.to_le_bytes()))
            + usize::from(settings.set(8, &(self.flags() & 0x90000).to_le_bytes()))
    }
    pub fn configure_from_image(
        &mut self,
        pack: AvPack,
        base: u32,
        memory: &dyn GuestMemory,
    ) -> bool {
        if memory.word(base) != Some(0x48454258) {
            return false;
        }
        let (Some(headers), Some(cert)) = (
            memory.word(base.wrapping_add(0x108)),
            memory.word(base.wrapping_add(0x118)),
        ) else {
            return false;
        };
        let offset = cert.wrapping_sub(base);
        if offset > headers || headers - offset < 0xa4 {
            return false;
        }
        let Some(region) = memory.word(cert.wrapping_add(0xa0)) else {
            return false;
        };
        self.configure(pack, region)
    }
    fn refuse(&mut self, status: u32) -> Answer {
        self.refused = self.refused.wrapping_add(1);
        Answer::effect(status, Effect::Refused("AV request"))
    }
    fn call(&mut self, ordinal: u32, args: &[u32], memory: &mut dyn GuestMemory) -> Answer {
        match ordinal {
            1 => Answer::status(self.saved_data),
            4 => {
                self.saved_data = args[0];
                Answer::status(SUCCESS)
            }
            3 => {
                let calls = self.display.as_ref().map_or(1, |d| d.calls.wrapping_add(1));
                self.display = Some(DisplayRequest {
                    register_base: args[0],
                    mode: args[2],
                    format: args[3],
                    pitch: args[4],
                    frame_buffer: args[5],
                    calls,
                });
                Answer::status(0)
            }
            2 => {
                let (option, param, result) = (args[1], args[2], args[3]);
                let measured = match option {
                    6 | 0xf | 0x10 | 0xe => param == 0,
                    9 => param <= 1,
                    0xb => param == 0 || param == 5,
                    _ => false,
                };
                if !measured {
                    return self.refuse(INVALID_PARAMETER);
                }
                let value = match option {
                    6 if self.pack.is_none() => return self.refuse(0xc0000001),
                    6 => self.capabilities(),
                    0xf => 0,
                    0x10 => 1,
                    _ => {
                        if result != 0 {
                            return self.refuse(INVALID_PARAMETER);
                        }
                        self.ignored = self.ignored.wrapping_add(1);
                        return Answer::status(SUCCESS);
                    }
                };
                if !memory.put_word(result, value) {
                    return self.refuse(INVALID_PARAMETER);
                }
                self.fabricated = self.fabricated.wrapping_add(1);
                Answer::effect(SUCCESS, Effect::Fabricated("AV encoder/capability answer"))
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Default)]
pub struct HalState {
    registrations: Vec<u32>,
    pub duplicate: u32,
    pub unmatched: u32,
    pub overflow: u32,
    pub firmware_returns: u32,
    pub last_firmware_routine: u32,
    pub phy_initializations: u32,
    pub phy_queries: u32,
}
impl HalState {
    pub fn registrations(&self) -> &[u32] {
        &self.registrations
    }
    fn register(&mut self, address: u32, flag: u32) {
        if address == 0 {
            return;
        }
        let found = self.registrations.iter().position(|&p| p == address);
        if flag as u8 != 0 {
            if found.is_some() {
                self.duplicate = self.duplicate.wrapping_add(1);
            } else if self.registrations.len() == 64 {
                self.overflow = self.overflow.wrapping_add(1);
            } else {
                self.registrations.push(address);
            }
        } else if let Some(index) = found {
            self.registrations.remove(index);
        } else {
            self.unmatched = self.unmatched.wrapping_add(1);
        }
    }
}
#[derive(Debug, Default)]
pub struct FpState {
    saves: Vec<(u64, u32)>,
    pub anomalies: u32,
}
impl FpState {
    pub fn depth(&self, thread: u64) -> usize {
        self.saves.iter().filter(|&&(t, _)| t == thread).count()
    }
    pub fn total(&self) -> usize {
        self.saves.len()
    }
    fn call(&mut self, ordinal: u32, area: u32, thread: u64, memory: &dyn GuestMemory) -> Answer {
        let found = self
            .saves
            .iter()
            .rposition(|&(t, a)| t == thread && a == area);
        let anomaly;
        let status;
        if ordinal == 142 {
            if !memory.readable(area, 32) {
                anomaly = true;
                status = ACCESS_VIOLATION;
            } else if self.saves.len() == 256 {
                anomaly = true;
                status = INSUFFICIENT_RESOURCES;
            } else {
                anomaly = found.is_some();
                status = SUCCESS;
                self.saves.push((thread, area));
            }
        } else if let Some(index) = found {
            anomaly = Some(index) != self.saves.iter().rposition(|&(t, _)| t == thread);
            self.saves.remove(index);
            status = SUCCESS;
        } else {
            anomaly = true;
            status = SUCCESS;
        }
        if anomaly {
            self.anomalies = self.anomalies.wrapping_add(1);
            Answer::effect(status, Effect::FpAnomaly)
        } else {
            Answer::status(status)
        }
    }
}
#[derive(Debug, Default)]
pub struct RustKernelServices {
    pub crypto: crypto::CryptoCounters,
    pub settings: Settings,
    pub av: AvState,
    pub hal: HalState,
    pub fp: FpState,
    pub debug_prints: u32,
    pub debug_refused: u32,
}
impl RustKernelServices {
    pub fn reset(&mut self) {
        let widescreen = self.av.widescreen;
        *self = Self::default();
        self.av.widescreen = widescreen;
    }
}
impl KernelServices for RustKernelServices {
    fn call(
        &mut self,
        ordinal: u32,
        args: &[u32],
        thread: u64,
        memory: &mut dyn GuestMemory,
    ) -> Option<Answer> {
        let &(_, arity) = ORDINALS.iter().find(|&&(o, _)| o == ordinal)?;
        // Terminal services stay terminal even when their argument frame is unreadable.
        if ordinal == 95 {
            return Some(Answer::effect(
                0,
                Effect::BugCheck {
                    code: args.first().copied(),
                },
            ));
        }
        if ordinal == 49 {
            let routine = args.first().copied().unwrap_or(0);
            self.hal.firmware_returns = self.hal.firmware_returns.wrapping_add(1);
            self.hal.last_firmware_routine = routine;
            return Some(Answer::effect(
                0,
                Effect::Firmware {
                    routine,
                    pending_notifications: self.hal.registrations.len(),
                },
            ));
        }
        if args.len() < arity {
            if (1..=4).contains(&ordinal) {
                self.av.refused = self.av.refused.wrapping_add(1);
            }
            if ordinal == 8 {
                self.debug_refused = self.debug_refused.wrapping_add(1);
            }
            return Some(Answer::effect(
                if matches!(ordinal, 8 | 47 | 139 | 252) {
                    0
                } else {
                    INVALID_PARAMETER
                },
                Effect::Refused("missing argument"),
            ));
        }
        Some(match ordinal {
            1..=4 => self.av.call(ordinal, args, memory),
            8 => {
                if args[0] == 0 {
                    self.debug_refused = self.debug_refused.wrapping_add(1);
                    Answer::status(0)
                } else {
                    let mut text = debug::format(memory, args[0], args, 512);
                    // C logs with %s: embedded NUL from a source string ends the displayed message.
                    if let Some(nul) = text.iter().position(|&b| b == 0) {
                        text.truncate(nul);
                    }
                    while matches!(text.last(), Some(b'\r' | b'\n')) {
                        text.pop();
                    }
                    self.debug_prints = self.debug_prints.wrapping_add(1);
                    Answer::effect(0, Effect::Debug(text))
                }
            }
            24 => {
                let (status, fabricated) = self.settings.query(args, false, memory);
                if fabricated {
                    Answer::effect(status, Effect::Fabricated("unknown EEPROM setting zeros"))
                } else {
                    Answer::status(status)
                }
            }
            47 => {
                self.hal.register(args[0], args[1]);
                Answer::status(0)
            }
            139 | 142 => self.fp.call(ordinal, args[0], thread, memory),
            252 => {
                self.hal.phy_queries = self.hal.phy_queries.wrapping_add(1);
                Answer::effect(0, Effect::Fabricated("PHY disconnected"))
            }
            253 => {
                self.hal.phy_initializations = self.hal.phy_initializations.wrapping_add(1);
                Answer::status(0xc000000e)
            }
            335..=340 | 346 => {
                let mut answer =
                    Answer::status(crypto::call(ordinal, args, memory, &mut self.crypto));
                if ordinal == 340 && args[1] > 64 && answer.eax == SUCCESS {
                    answer.effects.push(Effect::Fabricated(
                        "unverified Xbox oversized HMAC key truncation",
                    ));
                }
                answer
            }
            _ => unreachable!(),
        })
    }
}
