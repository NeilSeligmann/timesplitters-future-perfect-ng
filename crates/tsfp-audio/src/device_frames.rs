//! C-free device frame admission. Decode guest ABI words into typed calls before
//! invoking the existing Rust-owned lifecycle. Caller restrictions describe the
//! recovered host scope, not the original DirectSound API. Tier 1: actual C handlers.
use crate::adapter::{GuestMemory, Refusal};
use crate::lifecycle::{CREATE_ENTRY, Device, GuestWorld, RELEASE_ENTRY};

pub const STARTUP_CALLER: u32 = 0x0002_79D5;
pub const MOVIE_CREATE_CALLER: u32 = 0x0044_570B;
pub const MOVIE_RELEASE_CALLER: u32 = 0x0044_5726;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceEntry {
    Create,
    Release,
}
impl DeviceEntry {
    pub fn from_address(address: u32) -> Option<Self> {
        match address {
            CREATE_ENTRY => Some(Self::Create),
            RELEASE_ENTRY => Some(Self::Release),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateOrigin {
    Startup,
    Movie,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceCall {
    Create {
        guid: u32,
        output: u32,
        outer: u32,
        origin: CreateOrigin,
    },
    ReleaseMovie {
        interface: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallFrame {
    pub stack_pointer: u32,
    /// Zero means no stack-bound check, matching kernel_call_frame.
    pub stack_limit: u32,
}

/// `stack` points at the return address, followed by stdcall arguments. `None`
/// represents the C null frame. Argument reads reject overflow and enforce the optional stack limit.
/// Movie enablement is checked for Create admission; Release checks it in Device.
pub fn decode(
    memory: &impl GuestMemory,
    entry: DeviceEntry,
    frame: Option<CallFrame>,
    movie_calls: bool,
) -> Result<DeviceCall, Refusal> {
    let frame = frame.ok_or(Refusal("unreadable device frame"))?;
    let stack = frame.stack_pointer;
    let caller = memory
        .read_u32(stack)
        .ok_or(Refusal("unreadable device frame"))?;
    let argument = |index: u32| {
        let slot = u64::from(stack) + 4 + 4 * u64::from(index);
        if stack == 0
            || slot + 4 > 1u64 << 32
            || (frame.stack_limit != 0 && slot + 4 > u64::from(frame.stack_limit))
        {
            return Err(Refusal("device argument outside stack boundary"));
        }
        memory
            .read_u32(slot as u32)
            .ok_or(Refusal("unreadable device argument"))
    };
    match entry {
        DeviceEntry::Create => {
            let origin = if caller == STARTUP_CALLER {
                CreateOrigin::Startup
            } else if caller == MOVIE_CREATE_CALLER && movie_calls {
                CreateOrigin::Movie
            } else {
                return Err(Refusal("unrecovered device Create caller"));
            };
            Ok(DeviceCall::Create {
                guid: argument(0)?,
                output: argument(1)?,
                outer: argument(2)?,
                origin,
            })
        }
        DeviceEntry::Release => {
            if caller != MOVIE_RELEASE_CALLER {
                return Err(Refusal("unrecovered device Release caller"));
            }
            Ok(DeviceCall::ReleaseMovie {
                interface: argument(0)?,
            })
        }
    }
}

impl DeviceCall {
    /// Lifecycle owns device references and guest heaps; the dispatcher supplies
    /// codec readiness. Fatal refusals must stop the guest, never become S_OK.
    pub fn apply(
        self,
        device: &mut Device,
        world: &mut impl GuestWorld,
        codec_ready: bool,
    ) -> Result<u32, Refusal> {
        match self {
            Self::Create {
                guid,
                output,
                outer,
                origin,
            } => device.create(
                world,
                guid,
                output,
                outer,
                origin == CreateOrigin::Movie,
                codec_ready,
            ),
            Self::ReleaseMovie { interface } => device.release_movie(world, interface),
        }
    }
}
