//! Borrowed C services for typed Rust create preparation.
use crate::disposition_ffi::Facts;
use std::ffi::c_void;
use tsfp_engine_fileio::{
    acquisition::Backing,
    create::{self, CreateHost, Diagnostic},
    disposition::Target,
};
#[repr(C)]
pub struct Callbacks {
    context: *mut c_void,
    release: unsafe extern "C" fn(*mut c_void),
    diagnostic: unsafe extern "C" fn(*mut c_void, u32),
    truncate: unsafe extern "C" fn(*mut c_void, u32) -> u32,
    locate: unsafe extern "C" fn(*mut c_void) -> u32,
    create: unsafe extern "C" fn(*mut c_void, u32) -> u32,
    resolve: unsafe extern "C" fn(*mut c_void) -> u32,
}
struct Host<'a>(&'a Callbacks);
fn result(status: u32) -> Result<(), u32> {
    if status == 0 { Ok(()) } else { Err(status) }
}
fn code(backing: Backing) -> u32 {
    match backing {
        Backing::Fatx => 1,
        Backing::HostDirectory => 2,
        _ => 0,
    }
}
impl CreateHost for Host<'_> {
    fn release_resolution(&mut self) {
        unsafe { (self.0.release)(self.0.context) };
    }
    fn diagnostic(&mut self, diagnostic: Diagnostic) {
        unsafe {
            (self.0.diagnostic)(
                self.0.context,
                match diagnostic {
                    Diagnostic::Collision => 0,
                    Diagnostic::OverwriteRefused => 1,
                },
            )
        };
    }
    fn truncate(&mut self, backing: Backing) -> Result<(), u32> {
        result(unsafe { (self.0.truncate)(self.0.context, code(backing)) })
    }
    fn locate(&mut self) -> Option<Backing> {
        match unsafe { (self.0.locate)(self.0.context) } {
            1 => Some(Backing::Fatx),
            2 => Some(Backing::HostDirectory),
            _ => None,
        }
    }
    fn create(&mut self, backing: Backing) -> Result<(), u32> {
        result(unsafe { (self.0.create)(self.0.context, code(backing)) })
    }
    fn resolve_created(&mut self) -> Result<(), u32> {
        result(unsafe { (self.0.resolve)(self.0.context) })
    }
}
#[repr(C)]
pub struct Outcome {
    resolved: bool,
    status: u32,
    information: u32,
}
pub unsafe extern "C" fn prepare(
    host: *const Callbacks,
    disposition: u32,
    facts: *const Facts,
    status: u32,
    out: *mut Outcome,
) -> bool {
    if host.is_null() || facts.is_null() || out.is_null() {
        return false;
    }
    crate::counters::bump(crate::counters::CREATE);
    let f = unsafe { &*facts };
    let target = Target {
        exists: f.exists,
        name_not_found: f.name_not_found,
        fatx: f.fatx,
        host_dir: f.host_dir,
        is_directory: f.is_directory,
        writable: f.writable,
        has_host_fd: f.has_host_fd,
        device: f.device,
    };
    let (resolved, status, information, early) =
        match create::prepare(&mut Host(unsafe { &*host }), disposition, &target, status) {
            Ok(o) => (o.resolved, o.status, o.information, false),
            Err((status, information)) => (false, status, information, true),
        };
    unsafe {
        *out = Outcome {
            resolved,
            status,
            information,
        }
    };
    early
}
