//! Create preparation before any handle allocation. T2225 original preset.
use crate::acquisition::Backing;
use crate::disposition::{self, Plan, Target};
pub const COLLISION: u32 = 0xc0000035;
pub const ACCESS_DENIED: u32 = 0xc0000022;
#[derive(Clone, Copy, Debug)]
pub struct Outcome {
    pub resolved: bool,
    pub status: u32,
    pub information: u32,
}
#[derive(Clone, Copy, Debug)]
pub enum Diagnostic {
    Collision,
    OverwriteRefused,
}
pub trait CreateHost {
    fn release_resolution(&mut self);
    fn diagnostic(&mut self, diagnostic: Diagnostic);
    fn truncate(&mut self, backing: Backing) -> Result<(), u32>;
    fn locate(&mut self) -> Option<Backing>;
    fn create(&mut self, backing: Backing) -> Result<(), u32>;
    fn resolve_created(&mut self) -> Result<(), u32>;
}
/// Returns Err for an early failure, Ok for the common resolution/acquire tail.
pub fn prepare(
    host: &mut impl CreateHost,
    disposition: u32,
    target: &Target,
    resolve_status: u32,
) -> Result<Outcome, (u32, u32)> {
    let mut out = Outcome {
        resolved: target.exists,
        status: resolve_status,
        information: 0,
    };
    match disposition::plan(disposition, target) {
        Plan::Collision => {
            host.release_resolution();
            host.diagnostic(Diagnostic::Collision);
            return Err((COLLISION, disposition::INFO_EXISTS));
        }
        Plan::RefuseOverwrite => {
            host.release_resolution();
            host.diagnostic(Diagnostic::OverwriteRefused);
            return Err((ACCESS_DENIED, 0));
        }
        Plan::TruncateFatx { information } => {
            // C FATX resolutions carry no host descriptors; preserve its failure contract.
            host.truncate(Backing::Fatx).map_err(|status| (status, 0))?;
            out.information = information;
        }
        Plan::TruncateHost { information } => {
            if let Err(status) = host.truncate(Backing::HostDirectory) {
                host.release_resolution();
                return Err((status, 0));
            }
            out.information = information;
        }
        Plan::Open => out.information = disposition::INFO_OPENED,
        Plan::Refuse => (),
        Plan::TryCreate => {
            out.information = disposition::INFO_OPENED;
            if host.locate() == Some(Backing::Fatx) {
                match host.create(Backing::Fatx) {
                    Err(status) => out.status = status,
                    Ok(()) => match host.resolve_created() {
                        Err(status) => out.status = status,
                        Ok(()) => {
                            out.resolved = true;
                            out.status = 0;
                            out.information = disposition::INFO_CREATED;
                        }
                    },
                }
            }
            // Deliberately locate again, even after successful FATX creation.
            if host.locate() == Some(Backing::HostDirectory) {
                match host.create(Backing::HostDirectory) {
                    Err(status) => out.status = status,
                    Ok(()) => {
                        out.resolved = true;
                        out.status = 0;
                        out.information = disposition::INFO_CREATED;
                    }
                }
            }
        }
    }
    Ok(out)
}
