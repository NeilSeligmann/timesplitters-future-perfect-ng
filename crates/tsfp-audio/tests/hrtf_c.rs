//! Tier 1 actual-C differential; synthetic kernel services, no disc or host.
use std::{fs, process::Command};
use tsfp_audio::hrtf::{self, CriticalState, HrtfKernel};

struct Kernel {
    case: u32,
    events: Vec<(u32, u32)>,
}
impl HrtfKernel for Kernel {
    fn irql(&self) -> Option<u8> {
        (self.case & 1 == 0).then_some(((self.case >> 1) & 3) as u8)
    }
    fn mapped(&self, address: u32, bytes: u32) -> bool {
        if address == hrtf::TABLE {
            assert_eq!(bytes, 44);
            self.case & 8 == 0
        } else {
            assert_eq!((address, bytes), (hrtf::CRITICAL_SECTION, 28));
            self.case & 16 == 0
        }
    }
    fn enter(&mut self, address: u32) -> u32 {
        assert_eq!(address, hrtf::CRITICAL_SECTION);
        self.events.push((1, address));
        0xdead
    }
    fn critical_state(&self, address: u32) -> Option<CriticalState> {
        assert_eq!(address, hrtf::CRITICAL_SECTION);
        (self.case & 32 == 0).then_some(CriticalState {
            owner: if self.case & 64 == 0 { 7 } else { 8 },
            recursion: if self.case & 128 == 0 { 2 } else { 0 },
        })
    }
    fn owner_token(&self) -> u32 {
        7
    }
    fn write_word(&mut self, address: u32, value: u32) -> bool {
        self.events.push((address, value));
        self.case & 256 == 0
    }
    fn leave(&mut self, address: u32) -> u32 {
        assert_eq!(address, hrtf::CRITICAL_SECTION);
        self.events.push((2, address));
        0xabcd
    }
}

#[test]
fn actual_c_hrtf_differential() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join(format!("tmp/hrtf-oracle-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    // Copy the complete actual translation unit. Stub only its external headers/services.
    fs::copy(
        root.join("crates/tsfp-audio-conformance/oracle/dsound_hrtf.c"),
        dir.join("dsound_hrtf.c"),
    )
    .unwrap();
    fs::copy(
        root.join("src/audio/dsound_hrtf.h"),
        dir.join("dsound_hrtf.h"),
    )
    .unwrap();
    for name in ["dsound_hle.h", "kernel_call.h", "kernel_critsec.h"] {
        fs::write(dir.join(name), "").unwrap();
    }
    fs::write(dir.join("oracle.c"), r#"
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <setjmp.h>
static unsigned c;
static jmp_buf stop;
typedef struct { uint32_t owner, recursion; } kernel_critsec_info;
static void logger(const char *fmt, ...) { (void)fmt; }
static void (*dsound_hle_log(void))(const char *, ...) { return logger; }
static void *kernel_guest_at(uint32_t a, uint32_t n) { (void)n; return (c & (a == 0x412B5C ? 8 : 16)) ? NULL : &c; }
static const void *kernel_guest_at_ro(uint32_t a, uint32_t n) { return kernel_guest_at(a, n); }
static uint32_t kernel_critsec_enter_guest(uint32_t a) { printf("1 %u\n", a); return 0xdead; }
static uint32_t kernel_critsec_leave_guest(uint32_t a) { printf("2 %u\n", a); return 0xabcd; }
static bool kernel_critsec_state(uint32_t a, kernel_critsec_info *s) { (void)a; s->owner = c & 64 ? 8 : 7; s->recursion = c & 128 ? 0 : 2; return !(c & 32); }
static uint32_t kernel_critsec_owner_token(void) { return 7; }
static bool kernel_guest_write_u32(uint32_t a, uint32_t v) { printf("%u %u\n", a, v); return !(c & 256); }
static int dsound_hle_register(uint32_t a, uint32_t (*h)(void *)) { (void)a; (void)h; return 1; }
#include "dsound_hrtf.c"
static bool irql(uint8_t *out) { *out = (c >> 1) & 3; return !(c & 1); }
static void fatal(uint32_t a, const char *reason) { (void)a; (void)reason; longjmp(stop, 1); }
int main(void) {
    dsound_hrtf_set_irql_provider(irql); dsound_hrtf_set_fatal(fatal);
    for (c = 0; c < 512; ++c) {
        printf("CASE %u\n", c);
        if (setjmp(stop)) puts("ERR");
        else { uint32_t answer = dsound_use_light_hrtf(); printf("OK %u\n", answer); }
    }
}
"#).unwrap();
    let compiler = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    let build = Command::new(compiler)
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
    for case in 0..512 {
        let mut kernel = Kernel {
            case,
            events: vec![],
        };
        let result = hrtf::use_light(&mut kernel);
        if std::env::var_os("TSFP_AUDIO_HRTF_MUTANT").is_some() && case == 0 {
            kernel.events[1].1 ^= 1;
        }
        expected += &format!("CASE {case}\n");
        for (a, b) in kernel.events {
            expected += &format!("{a} {b}\n");
        }
        expected += &match result {
            Ok(v) => format!("OK {v}\n"),
            Err(_) => "ERR\n".into(),
        };
    }
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    fs::remove_dir_all(dir).unwrap();
}
