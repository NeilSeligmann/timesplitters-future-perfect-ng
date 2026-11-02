use crate::invalid;
use std::io::{self, Write};
use tsfp_audio::{adpcm, contract::Command};

fn hex(w: &mut dyn Write, bytes: impl IntoIterator<Item = u8>) -> io::Result<()> {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    // Bounded staging avoids one underlying write per audio sample.
    let mut block = [0; 4096];
    let mut used = 0;
    for byte in bytes {
        block[used] = DIGITS[(byte >> 4) as usize];
        block[used + 1] = DIGITS[(byte & 15) as usize];
        used += 2;
        if used == block.len() {
            w.write_all(&block)?;
            used = 0;
        }
    }
    w.write_all(&block[..used])
}

pub(super) fn command(
    w: &mut dyn Write,
    command: &Command,
    accepted: bool,
    encoded: Option<&[u8]>,
) -> io::Result<()> {
    use Command::*;
    let ok = u8::from(accepted);
    if encoded.is_some() && !matches!(command, PlayBuffer { .. }) {
        return Err(invalid("unexpected encoded buffer"));
    }
    match command {
        SubmitAdpcm {
            key,
            ticks,
            source_hz,
            mono,
            data,
        } => {
            write!(w, "SA {key} {ticks} {source_hz} {} {ok} ", u8::from(*mono))?;
            hex(w, data.iter().copied())?;
        }
        SubmitPcm16Stereo {
            key,
            ticks,
            source_hz,
            pcm,
        } => {
            if pcm.len() % 2 != 0 {
                return Err(invalid("PCM is not stereo"));
            }
            write!(w, "SP {key} {ticks} {source_hz} {ok} ")?;
            hex(w, pcm.iter().flat_map(|s| s.to_le_bytes()))?;
        }
        CutStream { key, ticks } => write!(w, "CUT {key} {ticks} {ok}")?,
        SetStreamRunning {
            key,
            ticks,
            running,
        } => write!(w, "RUN {key} {ticks} {} {ok}", u8::from(*running))?,
        SetStreamFrequency {
            key,
            ticks,
            source_hz,
        } => write!(w, "FRQ {key} {ticks} {source_hz} {ok}")?,
        SetStreamVolume { key, millibels } => write!(w, "VOL {key} {millibels} {ok}")?,
        ResetStreamVolume { key } => write!(w, "RVOL {key} {ok}")?,
        PlayBuffer {
            key,
            serial,
            ticks,
            frequency,
            pcm,
            paused,
            looping,
            loop_start,
            loop_end,
            volume,
        } => {
            let encoded = encoded.ok_or_else(|| invalid("PB requires original ADPCM"))?;
            if encoded.is_empty()
                || encoded.len() > 36 * 32768
                || encoded.len() % 36 != 0
                || adpcm::decode(encoded, 1).as_deref() != Some(pcm.as_slice())
            {
                return Err(invalid("PB source differs from decoded command"));
            }
            write!(
                w,
                "PB {key} {serial} {ticks} {frequency} {} {} {loop_start} {loop_end} {volume} {ok} ",
                u8::from(*paused),
                u8::from(*looping)
            )?;
            hex(w, encoded.iter().copied())?;
        }
        PauseBuffer {
            key,
            serial,
            ticks,
            paused,
        } => write!(w, "PAB {key} {serial} {ticks} {} {ok}", u8::from(*paused))?,
        StopBuffer { key, serial, ticks } => write!(w, "STB {key} {serial} {ticks} {ok}")?,
        FrequencyBuffer {
            key,
            serial,
            ticks,
            frequency,
        } => write!(w, "FRB {key} {serial} {ticks} {frequency} {ok}")?,
        VolumeBuffer {
            key,
            serial,
            ticks,
            volume,
        } => write!(w, "VLB {key} {serial} {ticks} {volume} {ok}")?,
        LoopBuffer {
            key,
            serial,
            ticks,
            loop_start,
            loop_end,
        } => write!(w, "LPB {key} {serial} {ticks} {loop_start} {loop_end} {ok}")?,
        ResetBuffers => {
            if !accepted {
                return Err(invalid("RST cannot encode refusal"));
            }
            write!(w, "RST")?;
        }
        RouteStream {
            key,
            serial,
            ticks,
            routing: r,
        } => {
            write!(
                w,
                "RT {key} {serial} {ticks} {} {} {} {} {ok}",
                r.volume, r.headroom, r.aggregate, r.count
            )?;
            for value in r.bins {
                write!(w, " {value}")?;
            }
            for value in r.bin_volume {
                write!(w, " {value}")?;
            }
        }
        FrequencyStream {
            key,
            serial,
            ticks,
            base_hz,
            old_hz,
            new_hz,
        } => write!(
            w,
            "FQ {key} {serial} {ticks} {base_hz} {old_hz} {new_hz} {ok}"
        )?,
        FormatStream {
            key,
            serial,
            ticks,
            old_hz,
            new_hz,
        } => write!(w, "FS {key} {serial} {ticks} {old_hz} {new_hz} {ok}")?,
        EnableHeadroom { enabled } => write!(w, "HE {} {ok}", u8::from(*enabled))?,
        BindHeadroom { identity } => write!(w, "HB {identity} {ok}")?,
        SetHeadroom {
            identity,
            ticks,
            bin,
            headroom,
        } => write!(w, "HS {identity} {ticks} {bin} {headroom} {ok}")?,
    }
    writeln!(w)
}
