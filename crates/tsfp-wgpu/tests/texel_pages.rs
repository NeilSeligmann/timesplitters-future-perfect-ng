// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024k: a texture is decoded again only when a texel write touched a page of its own data (the global texel epoch used to
//! redecode every bound texture after any write, MEASURED as most of the renderer thread's time on the engage route).
//! FABRICATED stream: one 4x4 white texture, an unrelated texel write, then a write into the texture's own page.
use tsfp_wgpu::context::{GpuContext, Preference};
use tsfp_wgpu::replay::Replayer;
use tsfp_wgpu::stream::Stream;

mod support;
use support::hex;

fn decodes(replayer: &Replayer) -> usize {
    replayer.report.texture_formats.values().sum()
}

#[test]
fn only_a_write_into_the_textures_own_pages_redecodes_it() {
    let gpu = match GpuContext::new(&Preference::Software) {
        Ok(gpu) => gpu,
        Err(error) => {
            println!("SKIPPED (no software adapter): {error}");
            return;
        }
    };
    let format_word = (6 << 8) | (1 << 20) | (1 << 24) | 0x29;
    let setup = format!(
        "mem 42000100 {}\nmem 80001100 {}\ncall SetTexture 0 42000100\n",
        hex(&[0x0004_0001, 0x8000_1100, 0, format_word, 0]),
        hex(&[0xFFFF_FFFF; 4])
    );
    let mut replayer = Replayer::new(gpu, 640, 480);
    replayer.run(&Stream::from_text(&setup).expect("parses"));
    assert_eq!(decodes(&replayer), 1, "the first bind decodes the texture");
    let again = |text: &str, replayer: &mut Replayer| {
        replayer.run(&Stream::from_text(text).expect("parses"));
    };
    again("call SetTexture 0 42000100\n", &mut replayer);
    assert_eq!(decodes(&replayer), 1, "nothing was written: cached");
    again(
        &format!(
            "mem 80009000 {}\ncall SetTexture 0 42000100\n",
            hex(&[0x1234_5678; 8])
        ),
        &mut replayer,
    );
    assert_eq!(
        decodes(&replayer),
        1,
        "a texel write to another page keeps the decode"
    );
    again(
        &format!(
            "mem 80001100 {}\ncall SetTexture 0 42000100\n",
            hex(&[0xFF00_FF00; 4])
        ),
        &mut replayer,
    );
    assert_eq!(
        decodes(&replayer),
        2,
        "a write into the texture's own page decodes it again"
    );
}
