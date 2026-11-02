//! Audio conversion: Xbox ADPCM (IMA, 36 byte mono / 72 byte stereo blocks) and PS2 VAG to
//! lossless FLAC of decoded PCM, plus a JSON sidecar with the key, loop region and provenance.
//!
//! The Xbox decoder is a port of `src/audio/dsound_adpcm.c` (T1159, the layout public
//! IMA-Xbox decoders use, not yet compared with a lifted retail decoder). Sample rate and
//! channel count are not stored in the Xbox sound banks that were inspected (raw members
//! without header), so they are INFERRED (see docs/t-asset-importer.md).
use crate::Result;
use crate::json::quote;

pub const XADPCM_MONO_BLOCK: usize = 36;
pub const XADPCM_FRAMES: usize = 64;

/// Cheap structural test: whole blocks and every step index header in range.
pub fn is_xadpcm(b: &[u8], channels: usize) -> bool {
    let block = XADPCM_MONO_BLOCK * channels;
    !b.is_empty()
        && b.len() % block == 0
        && b.chunks(block).all(|blk| {
            (0..channels).all(|c| {
                let i = i16::from_le_bytes([blk[c * 4 + 2], blk[c * 4 + 3]]);
                (0..=88).contains(&i)
            })
        })
}

/// Decode headerless Xbox ADPCM to interleaved PCM16. Returns frames (samples per channel).
pub fn decode_xadpcm(b: &[u8], channels: usize) -> Result<Vec<i16>> {
    if !(1..=2).contains(&channels) || !is_xadpcm(b, channels) {
        return Err("xadpcm: not whole blocks with valid step indexes".into());
    }
    tsfp_audio::adpcm::decode(b, channels)
        .ok_or_else(|| "xadpcm: xbadpcm rejected validated input".to_string())
}

/// Encode interleaved PCM16 to FLAC without resampling, normalization, or other DSP.
fn flac(pcm: &[i16], channels: u16, rate: u32) -> Result<Vec<u8>> {
    let channels = usize::from(channels);
    if !(1..=2).contains(&channels)
        || pcm.is_empty()
        || pcm.len() % channels != 0
        || rate == 0
        || rate > 655_350
    {
        return Err("flac: invalid PCM shape".into());
    }
    let frames = pcm.len() / channels;
    if frames as u64 >= (1u64 << 36) {
        return Err("flac: sample count exceeds STREAMINFO limit".into());
    }

    const BLOCK: usize = 256;
    let mut output = Vec::new();
    output
        .try_reserve(pcm.len().saturating_mul(2).saturating_add(128))
        .map_err(|_| "flac: output allocation failed")?;
    output.extend_from_slice(b"fLaC");
    // Last metadata block, STREAMINFO (34 bytes). A zero MD5 means unavailable by spec.
    output.extend_from_slice(&[0x80, 0, 0, 34]);
    let final_block = frames % BLOCK;
    let min_block = if frames > BLOCK && final_block != 0 {
        final_block as u16
    } else {
        frames.min(BLOCK) as u16
    };
    let max_block = BLOCK.min(frames) as u16;
    output.extend_from_slice(&min_block.to_be_bytes());
    output.extend_from_slice(&max_block.to_be_bytes());
    output.extend_from_slice(&[0; 6]); // min/max frame sizes are unknown
    let stream_info =
        (u64::from(rate) << 44) | ((channels as u64 - 1) << 41) | (15u64 << 36) | frames as u64;
    output.extend_from_slice(&stream_info.to_be_bytes());
    output.extend_from_slice(&[0; 16]);

    for first in (0..frames).step_by(BLOCK) {
        let count = (frames - first).min(BLOCK);
        let frame_start = output.len();
        output.extend_from_slice(&[0xff, 0xf9, 0x60, if channels == 1 { 0x08 } else { 0x18 }]);
        write_flac_utf8(&mut output, first as u64);
        output.push((count - 1) as u8);
        let header_crc = flac_crc8(&output[frame_start..]);
        output.push(header_crc);

        for channel in 0..channels {
            output.push(0x02); // verbatim subframe, no wasted bits
            for frame in first..first + count {
                output.extend_from_slice(&pcm[frame * channels + channel].to_be_bytes());
            }
        }
        let frame_crc = flac_crc16(&output[frame_start..]);
        output.extend_from_slice(&frame_crc.to_be_bytes());
    }
    Ok(output)
}

fn write_flac_utf8(output: &mut Vec<u8>, value: u64) {
    let width = if value < (1 << 7) {
        1
    } else if value < (1 << 11) {
        2
    } else if value < (1 << 16) {
        3
    } else if value < (1 << 21) {
        4
    } else if value < (1 << 26) {
        5
    } else if value < (1 << 31) {
        6
    } else {
        7
    };
    if width == 1 {
        output.push(value as u8);
        return;
    }
    let continuation_bits = 6 * (width - 1);
    for index in (0..width - 1).rev() {
        output.push(0x80 | ((value >> (index * 6)) as u8 & 0x3f));
    }
    let prefix = 0xffu8 << (8 - width);
    output.push(prefix | ((value >> continuation_bits) as u8 & (0x7f >> width)));
    let start = output.len() - width;
    output[start..].rotate_left(1);
}

fn flac_crc8(bytes: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &byte in bytes {
        crc ^= byte;
        for _ in 0..8 {
            crc = if crc & 0x80 == 0 {
                crc << 1
            } else {
                (crc << 1) ^ 0x07
            };
        }
    }
    crc
}

fn flac_crc16(bytes: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &byte in bytes {
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 == 0 {
                crc << 1
            } else {
                (crc << 1) ^ 0x8005
            };
        }
    }
    crc
}

/// Loop region in sample frames, end exclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loop {
    pub start: u64,
    pub end: u64,
}

pub struct Decoded {
    pub flac: Vec<u8>,
    pub sidecar: String,
    pub frames: u64,
    pub channels: u16,
    pub rate: u32,
    codec: &'static str,
    loop_points: Option<Loop>,
    provenance: String,
}

impl Decoded {
    /// Attach the PAK sound key after name recovery. Unknown names remain null rather than
    /// being guessed from unrelated strings.
    pub fn with_sound_key(mut self, key: u32, name: Option<&str>) -> Self {
        self.sidecar = sidecar(
            self.codec,
            self.rate,
            self.channels,
            self.frames,
            self.loop_points,
            &self.provenance,
            Some((key, name)),
        );
        self
    }
}

fn sidecar(
    codec: &str,
    rate: u32,
    channels: u16,
    frames: u64,
    lp: Option<Loop>,
    how: &str,
    sound_key: Option<(u32, Option<&str>)>,
) -> String {
    let l = lp.map_or("null".to_string(), |l| {
        format!("{{\"start\":{},\"end\":{}}}", l.start, l.end)
    });
    let key = sound_key.map_or("null".to_string(), |(key, name)| {
        let name = name.map_or("null".to_string(), quote);
        format!("{{\"crc32\":\"{key:08x}\",\"name\":{name}}}")
    });
    format!(
        "{{\"format\":\"tsfp-audio\",\"version\":2,\"source_codec\":{},\"sample_rate\":{rate},\"channels\":{channels},\"frames\":{frames},\"sound_key\":{key},\"loop\":{l},\"provenance\":{}}}\n",
        quote(codec),
        quote(how)
    )
}

/// Convert a headerless Xbox ADPCM member with caller supplied (inferred) parameters. The
/// loop is unknown for raw members: the sidecar says `null`, never a guess.
pub fn convert_xadpcm(b: &[u8], channels: usize, rate: u32, how: &str) -> Result<Decoded> {
    let pcm = decode_xadpcm(b, channels)?;
    let frames = (pcm.len() / channels) as u64;
    Ok(Decoded {
        flac: flac(&pcm, channels as u16, rate)?,
        sidecar: sidecar("xbox-adpcm", rate, channels as u16, frames, None, how, None),
        frames,
        channels: channels as u16,
        rate,
        codec: "xbox-adpcm",
        loop_points: None,
        provenance: how.to_string(),
    })
}

/// A RIFF WAVE with format tag 0x0069 (Xbox ADPCM) carries its own rate, channels and an
/// optional `smpl` loop. Returns None when `b` is not that.
pub fn convert_riff_xadpcm(b: &[u8]) -> Option<Result<Decoded>> {
    if b.len() < 12 || &b[..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return None;
    }
    let (mut fmt, mut data, mut lp) = (None, None, None);
    let mut o = 12;
    while o + 8 <= b.len() {
        let id = &b[o..o + 4];
        let n = u32::from_le_bytes(b[o + 4..o + 8].try_into().unwrap()) as usize;
        let body = b.get(o + 8..(o + 8 + n).min(b.len()))?;
        match id {
            b"fmt " if body.len() >= 16 => {
                fmt = Some((
                    u16::from_le_bytes([body[0], body[1]]),
                    u16::from_le_bytes([body[2], body[3]]),
                    u32::from_le_bytes(body[4..8].try_into().unwrap()),
                ))
            }
            b"data" => data = Some(body),
            b"smpl" if body.len() >= 60 => {
                let w = |i: usize| u32::from_le_bytes(body[i..i + 4].try_into().unwrap()) as u64;
                if w(28) > 0 {
                    // first loop: start at +44, end (inclusive) at +48
                    lp = Some(Loop {
                        start: w(44),
                        end: w(48) + 1,
                    });
                }
            }
            _ => {}
        }
        o += 8 + n + (n & 1);
    }
    let (tag, ch, rate) = fmt?;
    if tag != 0x69 {
        return None;
    }
    Some((|| {
        let mut d = convert_xadpcm(
            data.ok_or("riff: no data chunk")?,
            ch as usize,
            rate,
            "rate, channels and loop read from the RIFF header (MEASURED from file)",
        )?;
        d.loop_points = lp;
        d.provenance = "RIFF fmt/smpl chunks".to_string();
        d.sidecar = sidecar("xbox-adpcm", rate, ch, d.frames, lp, &d.provenance, None);
        Ok(d)
    })())
}

const VAG_COEF: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];

/// PS2 VAG (`VAGp`, mono): PSX SPU ADPCM, 16 byte frames of 28 samples. Loop start is the
/// frame flagged 4 (or 6), loop end the frame flagged 3. Public format description, not
/// compared against a PS2 reference decoder (INFERRED correctness, structure MEASURED).
pub fn convert_vag(b: &[u8]) -> Option<Result<Decoded>> {
    if b.len() < 0x30 || &b[..4] != b"VAGp" {
        return None;
    }
    Some((|| {
        let rate = u32::from_be_bytes(b[16..20].try_into().unwrap());
        if rate == 0 || rate > 96000 {
            return Err(format!("vag: implausible sample rate {rate}"));
        }
        let (mut h1, mut h2) = (0i32, 0i32);
        let mut pcm = Vec::new();
        let (mut ls, mut le) = (None, None);
        for (i, f) in b[0x30..].chunks_exact(16).enumerate() {
            let (shift, filter) = ((f[0] & 15) as u32, (f[0] >> 4) as usize);
            let (c0, c1) = *VAG_COEF.get(filter).ok_or("vag: bad filter")?;
            let flag = f[1];
            if flag == 7 && i > 0 && f[2..].iter().all(|x| *x == 0) {
                break;
            }
            if flag & 4 != 0 && ls.is_none() {
                ls = Some(pcm.len() as u64);
            }
            if flag == 3 {
                le = Some(pcm.len() as u64 + 28);
            }
            for k in 0..28 {
                let byte = f[2 + k / 2];
                let n = if k % 2 == 0 { byte & 15 } else { byte >> 4 } as i32;
                let raw = ((n << 12) as i16 as i32) >> shift.min(12);
                let s = (raw + ((h1 * c0 + h2 * c1) >> 6)).clamp(-32768, 32767);
                h2 = h1;
                h1 = s;
                pcm.push(s as i16);
            }
            if flag == 1 {
                break;
            }
        }
        let lp = match (ls, le) {
            (Some(start), Some(end)) if end > start => Some(Loop { start, end }),
            _ => None,
        };
        let frames = pcm.len() as u64;
        Ok(Decoded {
            flac: flac(&pcm, 1, rate)?,
            sidecar: sidecar(
                "ps2-vag",
                rate,
                1,
                frames,
                lp,
                "rate from VAGp header, loop from frame flags",
                None,
            ),
            frames,
            channels: 1,
            rate,
            codec: "ps2-vag",
            loop_points: lp,
            provenance: "rate from VAGp header, loop from frame flags".to_string(),
        })
    })())
}
