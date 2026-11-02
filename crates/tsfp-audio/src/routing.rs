//! Stream routing and frequency pitch (T2046 slice 9b): the Rust port of `dsound_stream_routing.h` and
//! `dsound_stream_frequency_pitch` in `dsound_stream_frequency.c`, plus the [`StreamHooks`] trait that stands
//! for the C completion-model globals (`routing_note`, `frequency_note`, `format_note`, the x87 control word).
//!
//! Evidence class: Tier 1 against the C source. The pitch is computed in f64 where the C uses 80-bit
//! `log2l`/`llrintl`; `tests/routing.rs` checks that no rate in 8000..=96000 is within 1e-6 of a rounding tie,
//! so the two agree on the whole domain (INFERRED from that margin, the C was not run).

pub const ROUTE_SLOTS: usize = 8;
pub const ROUTE_BINS: usize = 32;

/// Oracle `dsound_stream_routing`: settings +20 headroom, +1C aggregate volume, +24 route count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Routing {
    pub volume: u32,
    pub headroom: u32,
    pub aggregate: u32,
    pub count: u32,
    pub bins: [u8; ROUTE_SLOTS],
    pub bin_volume: [u32; ROUTE_BINS],
}

impl Default for Routing {
    /// Oracle `dsound_stream_routing_init`: headroom 600, aggregate -600, stereo bins 0 and 1.
    fn default() -> Self {
        let mut bins = [0u8; ROUTE_SLOTS];
        bins[1] = 1;
        Self {
            volume: 0,
            headroom: 600,
            aggregate: 0u32.wrapping_sub(600),
            count: 2,
            bins,
            bin_volume: [0; ROUTE_BINS],
        }
    }
}

impl Routing {
    /// Oracle `dsound_stream_routing_volume`, all arithmetic modulo 2^32.
    pub fn set_volume(&mut self, volume: u32) {
        self.aggregate = self
            .aggregate
            .wrapping_add(volume.wrapping_sub(self.volume));
        self.volume = volume;
    }
    /// Oracle `dsound_stream_routing_headroom`.
    pub fn set_headroom(&mut self, headroom: u32) {
        self.aggregate = self
            .aggregate
            .wrapping_add(self.headroom.wrapping_sub(headroom));
        self.headroom = headroom;
    }
    /// Oracle `dsound_stream_routing_bins`: byte bin identity, <= 8 routes, speaker bins 0..=5, `defaults`
    /// restores (0,0),(1,0). Transactional: `false` leaves `self` untouched.
    pub fn set_bins(&mut self, pairs: &[(u32, u32)], defaults: bool) -> bool {
        let normal = [(0u32, 0u32), (1, 0)];
        let pairs: &[(u32, u32)] = if defaults { &normal } else { pairs };
        if pairs.len() > ROUTE_SLOTS || pairs.iter().any(|p| (p.0 as u8) > 5) {
            return false;
        }
        for (i, p) in pairs.iter().enumerate() {
            let bin = p.0 as u8;
            self.bins[i] = bin;
            self.bin_volume[usize::from(bin)] = p.1;
        }
        self.count = pairs.len() as u32;
        true
    }
    /// Oracle `dsound_stream_routing_volumes`: full DWORD bin, < 32. Transactional.
    pub fn set_bin_volumes(&mut self, pairs: &[(u32, u32)]) -> bool {
        if pairs.len() > ROUTE_BINS || pairs.iter().any(|p| p.0 as usize >= ROUTE_BINS) {
            return false;
        }
        for p in pairs {
            self.bin_volume[p.0 as usize] = p.1;
        }
        true
    }
    /// Oracle `dsound_stream_routing_attenuation` (original 40CBC0): NEG, SHL 6, unsigned DIV 100, cap 0FFF.
    pub fn attenuation(&self, slot: u32) -> u16 {
        if slot >= self.count
            || slot as usize >= ROUTE_SLOTS
            || self.bins[slot as usize] as usize >= ROUTE_BINS
        {
            return 4095;
        }
        let sum = self
            .aggregate
            .wrapping_add(self.bin_volume[usize::from(self.bins[slot as usize])]);
        let value = (0u32.wrapping_sub(sum) << 6) / 100;
        value.min(4095) as u16
    }
}

/// Oracle `dsound_stream_frequency_pitch` (original 406AD5/40C083): hertz times the guest float32 reciprocal,
/// float32, then 4096*log2 rounded to nearest even. Only control words 003F/027F/037F and 8000..=96000 Hz.
pub fn frequency_pitch(hertz: u32, control_word: u16) -> Option<i32> {
    if !(8000..=96000).contains(&hertz) || !matches!(control_word, 0x003F | 0x027F | 0x037F) {
        return None;
    }
    if hertz == 48000 {
        return Some(0);
    }
    let reciprocal = f32::from_bits(0x37AE_C33E);
    let normalized = (f64::from(hertz) * f64::from(reciprocal)) as f32;
    let scaled = 4096.0 * f64::from(normalized).log2();
    Some(scaled.round_ties_even() as i32)
}

/// The completion-model callbacks the stream setters consult (oracle `routing_note`, `frequency_note`,
/// `format_note`, `frequency_control_word`). A `*_installed` answer of false is the C NULL pointer.
pub trait StreamHooks {
    fn frequency_installed(&self) -> bool {
        false
    }
    fn routing_installed(&self) -> bool {
        false
    }
    fn format_installed(&self) -> bool {
        false
    }
    /// The guest x87 control word, `None` when unavailable.
    fn control_word(&mut self) -> Option<u16> {
        None
    }
    /// (stream, serial, old rate, new rate) accepted?
    fn frequency_note(&mut self, _stream: u32, _serial: u64, _old: u32, _new: u32) -> bool {
        false
    }
    /// (stream, serial, candidate routing) accepted?
    fn routing_note(&mut self, _stream: u32, _serial: u64, _candidate: &Routing) -> bool {
        false
    }
    /// (stream, serial, old rate, new rate, format address) accepted?
    fn format_note(
        &mut self,
        _stream: u32,
        _serial: u64,
        _old: u32,
        _new: u32,
        _format: u32,
    ) -> bool {
        false
    }
}

/// No completion model attached.
pub struct NoStreamHooks;
impl StreamHooks for NoStreamHooks {}
