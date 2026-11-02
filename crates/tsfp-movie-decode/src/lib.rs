//! Runtime movie decode for T3938, via FFmpeg (`ffmpeg-next`) on the RAW extracted `.xmv` files
//! (docs/decisions/asset-extraction-lossless.md). Decoding is only a library call: the
//! game-visible movie interface (open, next frame, end of movie, timing) stays in the caller and is
//! driven by [`Movie::info`] and the per-frame media timestamps.
//!
//! An override movie (replacement directory, other codec) goes through the same path; the caller
//! labels it as different from the original (rules 50/51).

use std::path::Path;
use std::sync::Once;

use ffmpeg::format::{Pixel, context::Input};
use ffmpeg::software::scaling::{Context as Scaler, Flags};
use ffmpeg::util::frame::video::Video;
use ffmpeg_next as ffmpeg;

#[derive(Debug)]
pub enum Error {
    Ffmpeg(ffmpeg::Error),
    NoVideo,
}

impl From<ffmpeg::Error> for Error {
    fn from(e: ffmpeg::Error) -> Self {
        Error::Ffmpeg(e)
    }
}

/// Stream facts the movie interface needs before the first frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub fps_num: i32,
    pub fps_den: i32,
    pub audio_streams: usize,
}

/// One decoded picture: tightly packed RGB24 and its media time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub index: u64,
    pub vt_ns: u64,
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

pub struct Movie {
    input: Input,
    video_stream: usize,
    decoder: ffmpeg::decoder::Video,
    scaler: Scaler,
    info: Info,
    time_base: (i32, i32),
    next_index: u64,
    flushed: bool,
}

fn init() -> Result<(), Error> {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = ffmpeg::init();
        ffmpeg::util::log::set_level(ffmpeg::util::log::Level::Error);
    });
    Ok(())
}

impl Movie {
    pub fn open(path: &Path) -> Result<Self, Error> {
        init()?;
        let input = ffmpeg::format::input(path)?;
        let stream = input
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or(Error::NoVideo)?;
        let video_stream = stream.index();
        let tb = stream.time_base();
        let rate = stream.avg_frame_rate();
        let decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?
            .decoder()
            .video()?;
        let (width, height) = (decoder.width(), decoder.height());
        let scaler = Scaler::get(
            decoder.format(),
            width,
            height,
            Pixel::RGB24,
            width,
            height,
            Flags::BILINEAR,
        )?;
        let audio_streams = input
            .streams()
            .filter(|s| s.parameters().medium() == ffmpeg::media::Type::Audio)
            .count();
        let info = Info {
            width,
            height,
            fps_num: rate.numerator(),
            fps_den: rate.denominator(),
            audio_streams,
        };
        Ok(Movie {
            input,
            video_stream,
            decoder,
            scaler,
            info,
            time_base: (tb.numerator(), tb.denominator()),
            next_index: 0,
            flushed: false,
        })
    }

    pub fn info(&self) -> Info {
        self.info
    }

    /// The next picture in presentation order, or `None` at the end of the movie.
    pub fn next_frame(&mut self) -> Result<Option<Frame>, Error> {
        let mut decoded = Video::empty();
        loop {
            if self.decoder.receive_frame(&mut decoded).is_ok() {
                return self.convert(&decoded).map(Some);
            }
            if self.flushed {
                return Ok(None);
            }
            let mut sent = false;
            for (stream, packet) in self.input.packets() {
                if stream.index() == self.video_stream {
                    self.decoder.send_packet(&packet)?;
                    sent = true;
                    break;
                }
            }
            if !sent {
                self.decoder.send_eof()?;
                self.flushed = true;
            }
        }
    }

    fn convert(&mut self, decoded: &Video) -> Result<Frame, Error> {
        let mut rgb = Video::empty();
        self.scaler.run(decoded, &mut rgb)?;
        let (w, h) = (self.info.width as usize, self.info.height as usize);
        let stride = rgb.stride(0);
        let data = rgb.data(0);
        let mut out = Vec::with_capacity(w * h * 3);
        for row in 0..h {
            out.extend_from_slice(&data[row * stride..row * stride + w * 3]);
        }
        let index = self.next_index;
        self.next_index += 1;
        let pts = decoded.pts().unwrap_or(0).max(0) as i128;
        let (num, den) = (self.time_base.0 as i128, self.time_base.1.max(1) as i128);
        Ok(Frame {
            index,
            vt_ns: (pts * num * 1_000_000_000 / den) as u64,
            width: self.info.width,
            height: self.info.height,
            rgb: out,
        })
    }
}

/// The whole first audio stream of a movie as interleaved signed 16 bit PCM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Audio {
    pub channels: u16,
    pub sample_rate: u32,
    pub samples: Vec<i16>,
}

impl Audio {
    /// Sample frames (one sample per channel).
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels.max(1))
    }
}

fn push_s16(out: &mut Vec<i16>, bytes: &[u8]) {
    out.extend(
        bytes
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]])),
    );
}

/// Decodes the first audio stream; `Ok(None)` for a silent movie.
pub fn decode_audio(path: &Path) -> Result<Option<Audio>, Error> {
    use ffmpeg::format::{Sample, sample::Type};
    init()?;
    let mut input = ffmpeg::format::input(path)?;
    let Some(stream) = input.streams().best(ffmpeg::media::Type::Audio) else {
        return Ok(None);
    };
    let index = stream.index();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?
        .decoder()
        .audio()?;
    let (channels, rate) = (decoder.channels(), decoder.rate());
    let out_format = Sample::I16(Type::Packed);
    let mut resampler = None;
    let mut samples = Vec::new();
    let mut drain = |decoder: &mut ffmpeg::decoder::Audio,
                     resampler: &mut Option<ffmpeg::software::resampling::Context>|
     -> Result<(), Error> {
        let mut frame = ffmpeg::util::frame::Audio::empty();
        while decoder.receive_frame(&mut frame).is_ok() {
            let n = frame.samples() * usize::from(channels);
            if frame.format() == out_format {
                push_s16(&mut samples, &frame.data(0)[..n * 2]);
                continue;
            }
            let r = match resampler {
                Some(r) => r,
                None => resampler.insert(ffmpeg::software::resampling::Context::get(
                    frame.format(),
                    frame.channel_layout(),
                    frame.rate(),
                    out_format,
                    frame.channel_layout(),
                    frame.rate(),
                )?),
            };
            let mut converted = ffmpeg::util::frame::Audio::empty();
            r.run(&frame, &mut converted)?;
            let n = converted.samples() * usize::from(channels);
            push_s16(&mut samples, &converted.data(0)[..n * 2]);
        }
        Ok(())
    };
    for (stream, packet) in input.packets() {
        if stream.index() == index {
            decoder.send_packet(&packet)?;
            drain(&mut decoder, &mut resampler)?;
        }
    }
    decoder.send_eof()?;
    drain(&mut decoder, &mut resampler)?;
    Ok(Some(Audio {
        channels,
        sample_rate: rate,
        samples,
    }))
}

/// Where a movie is read from. The original (extracted, raw XMV) path keeps the original frame count and
/// timing; a replacement found in the override directory is a different movie and is labelled so (rules 50/51).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Original(std::path::PathBuf),
    Override(std::path::PathBuf),
}

impl Source {
    pub fn path(&self) -> &Path {
        match self {
            Source::Original(p) | Source::Override(p) => p,
        }
    }
}

/// Resolves `name` (a movie file name such as `attract.xmv`). An override file with the same stem and
/// any extension in `override_dir` wins; otherwise the original under `original_dir` is used.
pub fn resolve(original_dir: &Path, override_dir: Option<&Path>, name: &str) -> Source {
    let stem = Path::new(name).file_stem();
    if let (Some(dir), Some(stem)) = (override_dir, stem)
        && let Ok(entries) = std::fs::read_dir(dir)
    {
        let mut found: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.file_stem() == Some(stem))
            .collect();
        found.sort();
        if let Some(first) = found.into_iter().next() {
            return Source::Override(first);
        }
    }
    Source::Original(original_dir.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_an_error() {
        assert!(Movie::open(Path::new("/nonexistent/none.xmv")).is_err());
    }

    #[test]
    fn override_wins_by_stem_else_original() {
        let base = std::env::temp_dir().join(format!("tsfp-movie-resolve-{}", std::process::id()));
        let (orig, over) = (base.join("o"), base.join("v"));
        std::fs::create_dir_all(&over).unwrap();
        std::fs::write(over.join("attract.webm"), b"x").unwrap();
        assert_eq!(
            resolve(&orig, Some(&over), "attract.xmv"),
            Source::Override(over.join("attract.webm"))
        );
        assert_eq!(
            resolve(&orig, Some(&over), "frd.xmv"),
            Source::Original(orig.join("frd.xmv"))
        );
        assert_eq!(resolve(&orig, None, "frd.xmv").path(), orig.join("frd.xmv"));
        std::fs::remove_dir_all(&base).unwrap();
    }

    /// Decodes every `.xmv` under `TSFP_XMV_DIR` (owner disc extraction); skipped when unset.
    #[test]
    fn decodes_owner_movies_when_present() {
        let Some(dir) = std::env::var_os("TSFP_XMV_DIR") else {
            return;
        };
        let mut seen = 0;
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            if e.path().extension().is_some_and(|x| x == "xmv") {
                let mut m = Movie::open(&e.path()).unwrap();
                let mut frames = 0u64;
                let mut last = 0;
                while let Some(f) = m.next_frame().unwrap() {
                    assert!(f.vt_ns >= last);
                    last = f.vt_ns;
                    frames += 1;
                }
                assert!(frames > 0, "{:?}", e.path());
                let audio = decode_audio(&e.path()).unwrap().unwrap();
                eprintln!(
                    "audio ch={} rate={} frames={} secs={:.3}",
                    audio.channels,
                    audio.sample_rate,
                    audio.frames(),
                    audio.frames() as f64 / f64::from(audio.sample_rate)
                );
                assert!(audio.frames() > 0);
                // T3912 container witness: header size and duration agree with the decoded movie.
                let data = std::fs::read(e.path()).unwrap();
                let (h, spans) = tsfp_xmv_container::walk_packets(&data).unwrap();
                assert_eq!((h.width, h.height), (m.info().width, m.info().height));
                let frame_ms = last / 1_000_000 / frames.saturating_sub(1).max(1);
                let decoded_ms = last / 1_000_000 + frame_ms;
                eprintln!(
                    "duration header={} decoded~{decoded_ms} packets={}",
                    h.duration_ms,
                    spans.len()
                );
                // Duration agreement is reported, not asserted: see docs/t3938-video-runtime-decode.md.

                eprintln!(
                    "{:?} frames={frames} last_ns={last} {:?}",
                    e.file_name(),
                    m.info()
                );
                seen += 1;
            }
        }
        assert!(seen > 0);
    }
}
