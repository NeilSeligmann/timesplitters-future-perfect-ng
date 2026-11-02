//! Optional sealed runtime capture. It is observation-only: errors are reported
//! to stderr and prevent publication, while host command results stay unchanged.
use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use tsfp_audio::contract::Command;
use tsfp_audio_observe::Recorder;

static NEXT_CAPTURE: AtomicU64 = AtomicU64::new(0);

pub(crate) struct LiveCapture {
    destination: PathBuf,
    partial: PathBuf,
    recorder: Option<Recorder<BufWriter<File>>>,
    failed: bool,
}

impl LiveCapture {
    pub(crate) fn from_environment(rate: u32, ticks_per_second: u64) -> Option<Self> {
        let destination = std::env::var_os("TSFP_AUDIO_CMD_LOG")?;
        if destination.is_empty() {
            return None;
        }
        match Self::create(PathBuf::from(destination), rate, ticks_per_second) {
            Ok(capture) => Some(capture),
            Err(error) => {
                eprintln!("audio capture could not start; no evidence will be produced: {error}");
                None
            }
        }
    }

    fn create(destination: PathBuf, rate: u32, ticks_per_second: u64) -> std::io::Result<Self> {
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let name = destination
            .file_name()
            .ok_or_else(|| std::io::Error::other("capture path has no file name"))?;
        let mut partial_name = name.to_os_string();
        partial_name.push(format!(
            ".partial.{}.{}",
            std::process::id(),
            NEXT_CAPTURE.fetch_add(1, Ordering::Relaxed)
        ));
        let partial = parent.join(partial_name);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)?;
        let recorder = match Recorder::new(BufWriter::new(file), rate, ticks_per_second) {
            Ok(recorder) => recorder,
            Err(error) => {
                let _ = fs::remove_file(&partial);
                return Err(error);
            }
        };
        let mut capture = Self {
            destination,
            partial,
            recorder: Some(recorder),
            failed: false,
        };
        Ok(capture)
    }

    pub(crate) fn command(&mut self, command: &Command, accepted: bool, encoded: Option<&[u8]>) {
        self.observe(|recorder| recorder.command(command, accepted, encoded));
    }

    pub(crate) fn render(&mut self, ticks: u64, capacity: usize, pcm: &[i16]) {
        self.observe(|recorder| recorder.render(ticks, capacity, pcm));
    }

    fn observe(
        &mut self,
        operation: impl FnOnce(&mut Recorder<BufWriter<File>>) -> std::io::Result<()>,
    ) {
        if self.failed {
            return;
        }
        let result = operation(self.recorder.as_mut().expect("active capture"));
        if let Err(error) = result {
            self.failed = true;
            eprintln!("audio capture failed; output will not be published: {error}");
        }
    }

    pub(crate) fn finish(mut self) {
        if self.failed {
            return;
        }
        let Some(recorder) = self.recorder.take() else {
            return;
        };
        let result = recorder
            .finish()
            .and_then(|writer| writer.into_inner().map_err(|error| error.into_error()))
            .and_then(|file| file.sync_all());
        let result = result.and_then(|_| {
            // hard_link atomically publishes and refuses to overwrite existing evidence.
            fs::hard_link(&self.partial, &self.destination)?;
            if let Err(error) = fs::remove_file(&self.partial) {
                let _ = fs::remove_file(&self.destination);
                return Err(error);
            }
            if let Some(parent) = self
                .destination
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
            {
                if let Err(error) = File::open(parent).and_then(|dir| dir.sync_all()) {
                    let _ = fs::remove_file(&self.destination);
                    return Err(error);
                }
            }
            Ok(())
        });
        if let Err(error) = result {
            eprintln!("audio capture finalization failed; output was not published: {error}");
        }
    }
}

impl Drop for LiveCapture {
    fn drop(&mut self) {
        if self.partial.exists() {
            if let Err(error) = fs::remove_file(&self.partial) {
                eprintln!("audio capture partial cleanup failed: {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_audio::{
        contract::{Command, SoundHost},
        mixer::Mixer,
    };

    fn paths() -> (PathBuf, PathBuf) {
        let id = NEXT_CAPTURE.fetch_add(1, Ordering::Relaxed);
        let destination = std::env::temp_dir().join(format!(
            "tsfp-audio-capture-{}-{id}.log",
            std::process::id()
        ));
        let partial = destination.with_file_name(format!(
            "tsfp-audio-capture-{}-{id}.log.partial.{}.{}",
            std::process::id(),
            std::process::id(),
            id
        ));
        (destination, partial)
    }

    #[test]
    fn publishes_only_flushed_complete_recordings_and_never_overwrites() {
        let (destination, _) = paths();
        let mut capture = LiveCapture::create(destination.clone(), 48000, 48000).unwrap();
        let partial = capture.partial.clone();
        assert!(!destination.exists());
        assert!(partial.exists());
        let mut mixer = Mixer::new(48000, 48000);
        let command = Command::SubmitPcm16Stereo {
            key: 1,
            ticks: 0,
            source_hz: 48000,
            pcm: (0..256).map(|i| (i * 9 - 1100) as i16).collect(),
        };
        let accepted = mixer.apply(&command);
        capture.command(&command, accepted, None);
        let pcm = mixer.render_until(128, 128);
        capture.render(128, 128, &pcm);
        capture.finish();
        assert!(destination.exists());
        assert!(!partial.exists());
        let input = fs::read_to_string(&destination).unwrap();
        assert!(input.starts_with("CAPTURE 1\nSTART 48000 48000\n"));
        assert!(input.lines().last().unwrap().starts_with("END "));

        let capture = LiveCapture::create(destination.clone(), 48000, 48000).unwrap();
        drop(capture.finish());
        assert!(destination.exists());
        assert!(!partial.exists());
        fs::remove_file(destination).unwrap();
    }

    #[test]
    fn failed_capture_is_removed_without_publishing_a_seal() {
        let (destination, _) = paths();
        let mut capture = LiveCapture::create(destination.clone(), 48000, 48000).unwrap();
        let partial = capture.partial.clone();
        capture.render(1, 1_048_577, &[]);
        capture.finish();
        assert!(!destination.exists());
        assert!(!partial.exists());
    }
}
