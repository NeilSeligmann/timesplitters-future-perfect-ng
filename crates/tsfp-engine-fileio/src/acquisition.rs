//! Shared post-resolution Open/Create ownership transaction (T2225).
//! Storage and object services own their state; Rust orders the transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle(pub u32);
pub const INSUFFICIENT_RESOURCES: u32 = 0xc000009a;
pub const SHARING_VIOLATION: u32 = 0xc0000043;
pub trait AcquireHost {
    fn allocate(&mut self) -> Option<Handle>;
    fn claim(&mut self, handle: Handle) -> Result<(), u32>;
    fn release_resolution(&mut self);
    fn release_handle(&mut self, handle: Handle);
}
pub fn acquire(host: &mut impl AcquireHost) -> Result<Handle, u32> {
    let Some(handle) = host.allocate() else {
        host.release_resolution();
        return Err(INSUFFICIENT_RESOURCES);
    };
    if let Err(status) = host.claim(handle) {
        host.release_resolution();
        if status == SHARING_VIOLATION {
            host.release_handle(handle);
        }
        return Err(status);
    }
    Ok(handle)
}

/// A mounted real backing is authoritative: failures never fabricate content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backing {
    Fatx,
    HostDirectory,
    Disc,
    Empty,
}
#[derive(Clone, Copy, Debug)]
pub struct Located {
    pub backing: Backing,
    pub declared: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    Refuse,
    Fabricate,
}
pub const NAME_NOT_FOUND: u32 = 0xc0000034;
pub trait ResolveHost {
    fn locate(&mut self) -> Result<Located, u32>;
    fn real(&mut self, backing: Backing) -> Result<(), u32>;
    fn empty(&mut self, fabricated: bool);
    fn refused(&mut self);
}
pub fn resolve(
    host: &mut impl ResolveHost,
    allow_empty: bool,
    missing: Missing,
) -> Result<(), u32> {
    let found = host.locate()?;
    if found.backing != Backing::Empty {
        return host.real(found.backing);
    }
    if !allow_empty || (!found.declared && missing == Missing::Refuse) {
        host.refused();
        return Err(NAME_NOT_FOUND);
    }
    host.empty(!found.declared);
    Ok(())
}
