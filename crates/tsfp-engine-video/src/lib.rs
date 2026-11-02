//! Game-side movie playback boundary (T2390).
//!
//! This crate defines the contract around the retained XMV implementation. It
//! deliberately owns no decoder, codec, audio mixer, or renderer internals.
//! Implementations adapt those services to the typed request/session API below.

use std::time::Duration;

/// Original title retry policy: five attempts, sleeping 50 ms between failures.
pub const ORIGINAL_OPEN_POLICY: OpenPolicy = OpenPolicy {
    attempts: 5,
    retry_delay: Duration::from_millis(50),
};

/// Stable identity for one playback, independent of decoder or audio handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlaybackId(u64);

impl PlaybackId {
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Owned path request. This cannot retain the original title's temporary path
/// scratch; path interpretation and case policy remain with the backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoviePath(Box<str>);

impl MoviePath {
    pub fn new(path: impl Into<Box<str>>) -> Result<Self, InvalidMoviePath> {
        let path = path.into();
        if path.is_empty() || path.contains('\0') {
            return Err(InvalidMoviePath);
        }
        Ok(Self(path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidMoviePath;

/// Caller-owned game policy for one playback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovieRequest {
    pub playback: PlaybackId,
    pub path: MoviePath,
    pub skippable: bool,
}

/// Open retry contract. Exhaustion is an error and must follow the caller's
/// disc-error route; it must never be converted into an empty or skipped movie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenPolicy {
    pub attempts: u8,
    pub retry_delay: Duration,
}

impl OpenPolicy {
    pub const fn is_valid(self) -> bool {
        self.attempts > 0
    }
}

/// Decoded dimensions and stream count reported by the opened movie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovieDescriptor {
    pub width: u32,
    pub height: u32,
    pub audio_streams: u32,
}

/// Host clock sample used by frame polling and audio synchronization.
///
/// `monotonic_ticks` is in `ticks_per_second`; `display_sequence` identifies
/// the host display opportunity. Decoder pacing can use either or both. This
/// does not impose a render cap or claim that every display tick has a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaybackClock {
    pub monotonic_ticks: u64,
    pub ticks_per_second: u64,
    pub display_sequence: u64,
}

impl PlaybackClock {
    pub const fn is_valid(self) -> bool {
        self.ticks_per_second != 0
    }
}

/// Successful synchronization result. Zero streams is valid for silent movies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioSync {
    pub streams_started: u32,
    pub clock: PlaybackClock,
}

/// Information available while a decoded picture is leased to the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovieFrameInfo {
    pub sequence: u64,
    pub width: u32,
    pub height: u32,
}

/// Borrowed ownership token for one decoded picture.
///
/// A session must keep the picture valid for the token's lifetime. Since the
/// token is returned from a mutable borrow of the session, the next frame poll
/// cannot occur until the caller drops or consumes it.
pub trait FrameLease {
    fn info(&self) -> MovieFrameInfo;
}

/// Scheduling and stream termination are distinct from an error.
#[derive(Debug, PartialEq, Eq)]
pub enum FramePoll<F> {
    /// No picture is due yet. Audio work and clock advancement may still occur.
    Pending,
    /// One picture is ready; its lease must end before polling again.
    Frame(F),
    /// Natural end of stream, or termination after its pending work drained.
    Ended,
}

/// Factory boundary for opening a movie path into a stable playback session.
pub trait MovieOpener {
    type Session: MovieSession<Error = Self::Error>;
    type Error;

    /// Honor `policy` and return an error after exhaustion. A failed open does
    /// not publish a session, audio stream, or decoder identity.
    fn open(
        &mut self,
        request: MovieRequest,
        policy: OpenPolicy,
    ) -> Result<Self::Session, Self::Error>;
}

/// Game-side movie lifecycle contract implemented by a decoder/audio adapter.
pub trait MovieSession: Sized {
    type Error;
    type Frame<'session>: FrameLease
    where
        Self: 'session;

    fn playback_id(&self) -> PlaybackId;
    fn descriptor(&self) -> MovieDescriptor;

    /// Start enabled movie streams against the decoder's playback clock.
    /// Implementations preserve stream identity and ordered completion data;
    /// stream setup failure is an error, never a fabricated valid stream.
    fn synchronize_audio(&mut self, at: PlaybackClock) -> Result<AudioSync, Self::Error>;

    /// Poll at the supplied host clock. Return `Pending` until a frame is due;
    /// return `Ended` only after the decoder reports drained end-of-stream.
    fn poll_frame<'session>(
        &'session mut self,
        at: PlaybackClock,
    ) -> Result<FramePoll<Self::Frame<'session>>, Self::Error>;

    /// Request termination. This is not an end-of-stream result: callers keep
    /// polling and servicing audio until `Ended` or an error is returned.
    fn request_termination(&mut self) -> Result<(), Self::Error>;

    /// Explicitly flush/drain audio, wait for outstanding I/O, and release the
    /// decoder. Consuming the session prevents duplicate close. The caller must
    /// release any frame lease first; Rust enforces this through its borrow.
    fn close(self) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Event {
        OpenAttempt(u8, OpenPolicy),
        AudioSync(PlaybackClock),
        PollPending(PlaybackClock),
        FrameLeased(u64),
        FrameReleased(u64),
        TerminationRequested,
        Ended(PlaybackClock),
        Closed,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum ProbeError {
        OpenExhausted(u8),
        InvalidClock,
    }

    struct XmvContractProbe {
        id: PlaybackId,
        events: Rc<RefCell<Vec<Event>>>,
        frames: u64,
        terminated: bool,
        fail_open: bool,
    }

    struct ProbeFrame<'a> {
        sequence: u64,
        events: &'a RefCell<Vec<Event>>,
    }

    impl FrameLease for ProbeFrame<'_> {
        fn info(&self) -> MovieFrameInfo {
            MovieFrameInfo {
                sequence: self.sequence,
                width: 640,
                height: 480,
            }
        }
    }

    impl Drop for ProbeFrame<'_> {
        fn drop(&mut self) {
            self.events
                .borrow_mut()
                .push(Event::FrameReleased(self.sequence));
        }
    }

    impl MovieOpener for XmvContractProbe {
        type Session = Self;
        type Error = ProbeError;

        fn open(
            &mut self,
            request: MovieRequest,
            policy: OpenPolicy,
        ) -> Result<Self::Session, Self::Error> {
            if !policy.is_valid() {
                return Err(ProbeError::OpenExhausted(0));
            }
            for attempt in 1..=policy.attempts {
                self.events
                    .borrow_mut()
                    .push(Event::OpenAttempt(attempt, policy));
                if !self.fail_open {
                    self.id = request.playback;
                    return Ok(Self {
                        id: request.playback,
                        ..self.clone_probe()
                    });
                }
            }
            Err(ProbeError::OpenExhausted(policy.attempts))
        }
    }

    impl XmvContractProbe {
        fn new(fail_open: bool) -> Self {
            Self {
                id: PlaybackId::new(1).unwrap(),
                events: Rc::new(RefCell::new(Vec::new())),
                frames: 0,
                terminated: false,
                fail_open,
            }
        }

        fn clone_probe(&self) -> Self {
            Self {
                id: self.id,
                events: Rc::clone(&self.events),
                frames: self.frames,
                terminated: self.terminated,
                fail_open: self.fail_open,
            }
        }
    }

    impl MovieSession for XmvContractProbe {
        type Error = ProbeError;
        type Frame<'session> = ProbeFrame<'session>;

        fn playback_id(&self) -> PlaybackId {
            self.id
        }

        fn descriptor(&self) -> MovieDescriptor {
            MovieDescriptor {
                width: 640,
                height: 480,
                audio_streams: 1,
            }
        }

        fn synchronize_audio(&mut self, at: PlaybackClock) -> Result<AudioSync, Self::Error> {
            if !at.is_valid() {
                return Err(ProbeError::InvalidClock);
            }
            self.events.borrow_mut().push(Event::AudioSync(at));
            Ok(AudioSync {
                streams_started: 1,
                clock: at,
            })
        }

        fn poll_frame<'session>(
            &'session mut self,
            at: PlaybackClock,
        ) -> Result<FramePoll<Self::Frame<'session>>, Self::Error> {
            if !at.is_valid() {
                return Err(ProbeError::InvalidClock);
            }
            if self.terminated && self.frames == 1 {
                self.events.borrow_mut().push(Event::Ended(at));
                return Ok(FramePoll::Ended);
            }
            if self.frames == 0 {
                self.events.borrow_mut().push(Event::PollPending(at));
                self.frames = 1;
                return Ok(FramePoll::Pending);
            }
            self.events
                .borrow_mut()
                .push(Event::FrameLeased(self.frames));
            Ok(FramePoll::Frame(ProbeFrame {
                sequence: self.frames,
                events: &self.events,
            }))
        }

        fn request_termination(&mut self) -> Result<(), Self::Error> {
            self.terminated = true;
            self.events.borrow_mut().push(Event::TerminationRequested);
            Ok(())
        }

        fn close(self) -> Result<(), Self::Error> {
            self.events.borrow_mut().push(Event::Closed);
            Ok(())
        }
    }

    fn request() -> MovieRequest {
        MovieRequest {
            playback: PlaybackId::new(42).unwrap(),
            path: MoviePath::new("d:\\xmv\\sample.xmv").unwrap(),
            skippable: true,
        }
    }

    fn clock(sequence: u64) -> PlaybackClock {
        PlaybackClock {
            monotonic_ticks: sequence * 10,
            ticks_per_second: 100,
            display_sequence: sequence,
        }
    }

    #[test]
    fn native_xmv_contract_open_pacing_audio_stop_and_close() {
        let mut opener = XmvContractProbe::new(false);
        let session = opener.open(request(), ORIGINAL_OPEN_POLICY).unwrap();
        assert_eq!(session.playback_id().get(), 42);
        assert_eq!(
            session.descriptor(),
            MovieDescriptor {
                width: 640,
                height: 480,
                audio_streams: 1
            }
        );

        let mut session = session;
        assert_eq!(
            session.synchronize_audio(clock(0)).unwrap(),
            AudioSync {
                streams_started: 1,
                clock: clock(0)
            }
        );
        assert!(matches!(
            session.poll_frame(clock(1)).unwrap(),
            FramePoll::Pending
        ));
        {
            let FramePoll::Frame(frame) = session.poll_frame(clock(2)).unwrap() else {
                panic!("frame due")
            };
            assert_eq!(
                frame.info(),
                MovieFrameInfo {
                    sequence: 1,
                    width: 640,
                    height: 480
                }
            );
        }
        session.request_termination().unwrap();
        assert!(matches!(
            session.poll_frame(clock(3)).unwrap(),
            FramePoll::Ended
        ));
        session.close().unwrap();

        let events = opener.events.borrow();
        assert_eq!(events[0], Event::OpenAttempt(1, ORIGINAL_OPEN_POLICY));
        assert_eq!(events[1], Event::AudioSync(clock(0)));
        assert_eq!(events[2], Event::PollPending(clock(1)));
        assert_eq!(events[3], Event::FrameLeased(1));
        assert_eq!(events[4], Event::FrameReleased(1));
        assert_eq!(events[5], Event::TerminationRequested);
        assert_eq!(events[6], Event::Ended(clock(3)));
        assert_eq!(events[7], Event::Closed);
    }

    #[test]
    fn open_exhaustion_is_an_error_after_original_attempt_count() {
        let mut opener = XmvContractProbe::new(true);
        assert!(matches!(
            opener.open(request(), ORIGINAL_OPEN_POLICY),
            Err(ProbeError::OpenExhausted(5))
        ));
        let events = opener.events.borrow();
        assert_eq!(events.len(), 5);
        assert_eq!(events[0], Event::OpenAttempt(1, ORIGINAL_OPEN_POLICY));
        assert_eq!(events[4], Event::OpenAttempt(5, ORIGINAL_OPEN_POLICY));
    }

    #[test]
    fn path_is_owned_and_rejects_empty_or_nul_values() {
        assert_eq!(MoviePath::new("movie.xmv").unwrap().as_str(), "movie.xmv");
        assert_eq!(MoviePath::new(""), Err(InvalidMoviePath));
        assert_eq!(MoviePath::new("movie\0.xmv"), Err(InvalidMoviePath));
        assert_eq!(PlaybackId::new(0), None);
        assert!(
            !PlaybackClock {
                monotonic_ticks: 0,
                ticks_per_second: 0,
                display_sequence: 0
            }
            .is_valid()
        );
    }
}
