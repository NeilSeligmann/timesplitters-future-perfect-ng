//! Early ATA identity setup, before guest identity publication/import patching.
//!
//! The backend owns physical mounting and the temporary backing capability. The
//! hybrid backend is a TEMPORARY-SHIM to C state until T3879 FIO-3 retires it.

pub trait Backend {
    type Backing;

    fn mount(&mut self) -> bool;
    fn acquire(&mut self) -> Option<Self::Backing>;
    fn probe(&mut self, backing: &Self::Backing) -> u32;
    fn release(&mut self, backing: Self::Backing);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MountRefused;

struct Acquired<'a, B: Backend> {
    backend: &'a mut B,
    backing: Option<B::Backing>,
}

impl<B: Backend> Drop for Acquired<'_, B> {
    fn drop(&mut self) {
        if let Some(backing) = self.backing.take() {
            self.backend.release(backing);
        }
    }
}

/// Mount the selected partition, acquire its backing, then probe and release it.
///
/// A failed mount is distinct from an unavailable backing. Every probe status,
/// including refusal, is returned after releasing the acquired backing exactly
/// once. The caller owns later identity publication and failure unmounting.
pub fn prepare(backend: &mut impl Backend, no_backing: u32) -> Result<u32, MountRefused> {
    if !backend.mount() {
        return Err(MountRefused);
    }
    let Some(backing) = backend.acquire() else {
        return Ok(no_backing);
    };
    let acquired = Acquired {
        backend,
        backing: Some(backing),
    };
    let status = acquired.backend.probe(
        acquired
            .backing
            .as_ref()
            .expect("acquired backing is present"),
    );
    drop(acquired);
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        calls: Vec<&'static str>,
        mounted: bool,
        acquired: bool,
        status: u32,
        panic_probe: bool,
    }

    impl Fake {
        fn new(mounted: bool, acquired: bool, status: u32) -> Self {
            Self {
                calls: Vec::new(),
                mounted,
                acquired,
                status,
                panic_probe: false,
            }
        }
    }

    impl Backend for Fake {
        type Backing = u64;

        fn mount(&mut self) -> bool {
            self.calls.push("mount");
            self.mounted
        }
        fn acquire(&mut self) -> Option<u64> {
            self.calls.push("acquire");
            self.acquired.then_some(0x1_0000_0007)
        }
        fn probe(&mut self, backing: &u64) -> u32 {
            assert_eq!(*backing, 0x1_0000_0007);
            self.calls.push("probe");
            assert!(!self.panic_probe, "synthetic probe panic");
            self.status
        }
        fn release(&mut self, backing: u64) {
            assert_eq!(backing, 0x1_0000_0007);
            self.calls.push("release");
        }
    }

    #[test]
    fn mount_refusal_does_not_acquire_probe_or_release() {
        let mut backend = Fake::new(false, true, 0);
        assert_eq!(prepare(&mut backend, 73), Err(MountRefused));
        assert_eq!(backend.calls, ["mount"]);
    }

    #[test]
    fn missing_backing_returns_supplied_status_without_release() {
        let mut backend = Fake::new(true, false, 0);
        assert_eq!(prepare(&mut backend, 73), Ok(73));
        assert_eq!(backend.calls, ["mount", "acquire"]);
    }

    #[test]
    fn every_probe_status_releases_once_before_return() {
        for status in [0, 1, 2, 3, u32::MAX] {
            let mut backend = Fake::new(true, true, status);
            assert_eq!(prepare(&mut backend, 73), Ok(status));
            assert_eq!(backend.calls, ["mount", "acquire", "probe", "release"]);
        }
    }

    #[test]
    fn safe_backend_unwind_still_releases_acquired_capability() {
        let mut backend = Fake::new(true, true, 0);
        backend.panic_probe = true;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = prepare(&mut backend, 73);
        }));
        assert!(result.is_err());
        assert_eq!(backend.calls, ["mount", "acquire", "probe", "release"]);
    }
}
