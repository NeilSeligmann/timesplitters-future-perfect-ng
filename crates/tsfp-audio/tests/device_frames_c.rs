//! Actual-C handler differential on FABRICATED stack/service answers.
use std::{cell::RefCell, fs, process::Command};
use tsfp_audio::{adapter::GuestMemory, device_frames::*};

struct Stack {
    case: u32,
    base: u32,
    reads: RefCell<Vec<u32>>,
}
impl GuestMemory for Stack {
    fn read_u32(&self, address: u32) -> Option<u32> {
        self.reads.borrow_mut().push(address);
        let i = address.wrapping_sub(self.base) / 4;
        if self.case & (1 << (4 + i)) != 0 {
            return None;
        }
        Some(if i == 0 {
            [STARTUP_CALLER, MOVIE_CREATE_CALLER, MOVIE_RELEASE_CALLER, 0][(self.case & 3) as usize]
        } else {
            100 + i
        })
    }
    fn range_readable(&self, _: u32, _: u32) -> bool {
        panic!("word reads only")
    }
    fn write_u32(&mut self, _: u32, _: u32) -> bool {
        panic!("decode must not write")
    }
    fn read_bytes(&self, _: u32, _: usize) -> Option<Vec<u8>> {
        panic!("word reads only")
    }
}

#[test]
fn actual_c_device_frame_differential() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join(format!("tmp/device-frames-oracle-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source =
        fs::read_to_string(root.join("crates/tsfp-audio-conformance/oracle/dsound_device.c"))
            .unwrap();
    // Extract unchanged actual handlers, isolate lifecycle calls as typed service observations.
    let handlers = source
        .split("static uint32_t create_handler(void *context)")
        .nth(1)
        .unwrap()
        .split("size_t dsound_device_register(void)")
        .next()
        .unwrap();
    let definitions = source
        .lines()
        .filter(|line| {
            [
                "CREATE_CALLER",
                "MOVIE_CREATE_CALLER",
                "MOVIE_RELEASE_CALLER",
                "RELEASE_ENTRY",
            ]
            .iter()
            .any(|name| line.starts_with(&format!("#define {name} ")))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let prefix = r#"
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <setjmp.h>
static unsigned c;
static uint32_t base;
static jmp_buf stop;
typedef uint32_t kernel_guest_ptr;
#define GUEST_ADDRESS_LIMIT UINT64_C(0x100000000)
typedef struct { uint32_t stack_ptr, stack_limit; } kernel_call_frame;
static void refuse(const char *s) __attribute__((noreturn));
static void refuse(const char *s) { (void)s; longjmp(stop, 1); }
static void refuse_at(uint32_t a, const char *s) __attribute__((noreturn));
static void refuse_at(uint32_t a, const char *s) { (void)a; refuse(s); }
static bool kernel_guest_read_u32(uint32_t a, uint32_t *out) {
    printf("READ %u\n", a);
    unsigned i = (uint32_t)(a - base) / 4;
    if (c & (1u << (4 + i))) return false;
    const uint32_t callers[] = {CREATE_CALLER, MOVIE_CREATE_CALLER, MOVIE_RELEASE_CALLER, 0};
    *out = i == 0 ? callers[c & 3] : 100 + i; return true;
}
static bool dsound_device_movie_calls_enabled(void) { return (c & 4) != 0; }
static uint32_t create_device(uint32_t g, uint32_t o, uint32_t a, bool movie) {
    printf("CREATE %u %u %u %u\n", g, o, a, movie); return 123;
}
static uint32_t dsound_device_release_movie(uint32_t a) { printf("RELEASE %u\n", a); return 123; }
"#;
    let main = r#"
int main(void) {
    for (unsigned shape = 0; shape < 8; ++shape) {
    base = shape < 4 ? 0x1000 : (shape < 6 ? 0xfffffff8u : 0);
    kernel_call_frame frame = {base, shape % 4 == 0 ? 0 : base + (shape % 4) * 4};
    for (unsigned entry = 0; entry < 2; ++entry) for (c = 0; c < 256; ++c) {
        printf("CASE %u %u %u\n", shape, entry, c);
        if (setjmp(stop)) puts("ERR");
        else { uint32_t v = (entry ? release_handler : create_handler)(c & 8 ? NULL : &frame); printf("OK %u\n", v); }
    }
    }
}
"#;
    let kernel_source = fs::read_to_string(root.join("src/xbox/kernel_call.c")).unwrap();
    let argument_helper = kernel_source
        .split("bool kernel_frame_arg(")
        .nth(1)
        .unwrap()
        .split("bool kernel_frame_build(")
        .next()
        .unwrap();
    fs::write(dir.join("oracle.c"), format!("{definitions}\n{prefix}\nbool kernel_frame_arg({argument_helper}\nstatic uint32_t create_handler(void *context){handlers}\n{main}")).unwrap();
    let build = Command::new(std::env::var("CC").unwrap_or_else(|_| "cc".into()))
        .args(["-std=c11", "-O2"])
        .arg(dir.join("oracle.c"))
        .arg("-o")
        .arg(dir.join("oracle"))
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new(dir.join("oracle")).output().unwrap();
    assert!(output.status.success());
    let mut expected = String::new();
    for shape in 0..8 {
        let base = if shape < 4 {
            0x1000
        } else if shape < 6 {
            0xffff_fff8u32
        } else {
            0
        };
        let frame = CallFrame {
            stack_pointer: base,
            stack_limit: if shape % 4 == 0 {
                0
            } else {
                base.wrapping_add((shape % 4) * 4)
            },
        };
        for (index, entry) in [DeviceEntry::Create, DeviceEntry::Release]
            .into_iter()
            .enumerate()
        {
            for case in 0..256 {
                expected += &format!("CASE {shape} {index} {case}\n");
                let memory = Stack {
                    case,
                    base,
                    reads: RefCell::new(vec![]),
                };
                let result = decode(
                    &memory,
                    entry,
                    (case & 8 == 0).then_some(frame),
                    case & 4 != 0,
                );
                for address in memory.reads.into_inner() {
                    expected += &format!("READ {address}\n");
                }
                match result {
                    Ok(DeviceCall::Create {
                        guid,
                        output,
                        outer,
                        origin,
                    }) => {
                        let mut movie = origin == CreateOrigin::Movie;
                        if std::env::var_os("TSFP_AUDIO_DEVICE_FRAME_MUTANT").is_some() {
                            movie = !movie;
                        }
                        expected += &format!(
                            "CREATE {guid} {output} {outer} {}\nOK 123\n",
                            u8::from(movie)
                        );
                    }
                    Ok(DeviceCall::ReleaseMovie { interface }) => {
                        expected += &format!("RELEASE {interface}\nOK 123\n")
                    }
                    Err(_) => expected += "ERR\n",
                }
            }
        }
    }
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    fs::remove_dir_all(dir).unwrap();
}

mod common;
#[test]
fn decoded_calls_use_rust_owned_device_lifecycle() {
    use tsfp_audio::lifecycle::{CREATE_ENTRY, Device, RELEASE_ENTRY};
    let mut world = common::World::new();
    let mut device = Device::new();
    let stack = 0x100000;
    let output = stack + 0x100;
    for (index, word) in [STARTUP_CALLER, 0, output, 0].into_iter().enumerate() {
        assert!(world.write_u32(stack + index as u32 * 4, word));
    }
    let frame = Some(CallFrame {
        stack_pointer: stack,
        stack_limit: stack + 16,
    });
    let create = DeviceEntry::from_address(CREATE_ENTRY).unwrap();
    assert_eq!(
        decode(&world, create, frame, false)
            .unwrap()
            .apply(&mut device, &mut world, true)
            .unwrap(),
        0
    );
    let interface = world.read_u32(output).unwrap();
    device.movie_calls = true;
    world.write_u32(stack, MOVIE_CREATE_CALLER);
    assert_eq!(
        decode(&world, create, frame, true)
            .unwrap()
            .apply(&mut device, &mut world, false)
            .unwrap(),
        0
    );
    assert_eq!(
        (device.reference_count(), device.movie_references()),
        (7, 1)
    );
    world.write_u32(stack, MOVIE_RELEASE_CALLER);
    world.write_u32(stack + 4, interface);
    let release = DeviceEntry::from_address(RELEASE_ENTRY).unwrap();
    assert_eq!(
        decode(&world, release, frame, true)
            .unwrap()
            .apply(&mut device, &mut world, true)
            .unwrap(),
        6
    );
    assert_eq!(device.movie_references(), 0);
    assert!(
        decode(&world, release, frame, true)
            .unwrap()
            .apply(&mut device, &mut world, true)
            .is_err()
    );
    assert!(DeviceEntry::from_address(0).is_none());
}
