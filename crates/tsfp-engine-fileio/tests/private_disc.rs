//! Owner-disc checks. Skipped unless TSFP_DISC_ISO names the image; prints counts only.
use tsfp_engine_fileio::pak::PakIndex;
use tsfp_engine_fileio::vfs::{Access, Disposition, Vfs};
use tsfp_engine_fileio::xiso::Xiso;
use tsfp_engine_fileio::{map_document::MapDocument, storage_flow::SampleMaps};

#[test]
fn every_small_pak_streams_like_the_format_crate() {
    let Some(iso) = std::env::var_os("TSFP_DISC_ISO") else {
        eprintln!("SKIP: TSFP_DISC_ISO unset");
        return;
    };
    let disc = Xiso::open(std::path::Path::new(&iso)).unwrap();
    let paks: Vec<_> = disc
        .walk()
        .unwrap()
        .into_iter()
        .filter(|(p, e)| p.to_ascii_lowercase().ends_with(".pak") && e.size <= 8 << 20)
        .collect();
    assert!(
        paks.len() > 100,
        "expected many small paks, got {}",
        paks.len()
    );
    let mut vfs = Vfs::new();
    vfs.mount_disc("\\Device\\CdRom0", disc);
    vfs.add_symlink("D:", "\\Device\\CdRom0");
    let (mut entries, mut mismatched) = (0usize, 0usize);
    for (path, _) in &paks {
        let h = vfs
            .open(&format!("D:\\{path}"), Access::READ, Disposition::Open)
            .unwrap();
        let mut whole = vec![0u8; vfs.size(h).unwrap() as usize];
        assert_eq!(vfs.read_at(h, 0, &mut whole).unwrap(), whole.len());
        let reference = tsfp_format_pak::Pak::parse(&whole).unwrap();
        let idx = PakIndex::open(&mut vfs, h).unwrap();
        assert_eq!(idx.entries, reference.entries, "{path}");
        for i in 0..idx.entries.len() {
            if idx.stored(&mut vfs, i).unwrap() != reference.stored(i).unwrap() {
                mismatched += 1;
            }
        }
        entries += idx.entries.len();
        vfs.close(h).unwrap();
    }
    eprintln!(
        "paks={} entries={entries} mismatched={mismatched}",
        paks.len()
    );
    assert_eq!(mismatched, 0);
}

#[test]
#[ignore = "requires TSFP_DISC_ISO naming the private owner's Xbox ISO"]
fn private_disc_sample_resources_decode_to_owned_map_documents() {
    let iso = std::env::var_os("TSFP_DISC_ISO").expect("private disc path");
    let mut samples = tsfp_engine_fileio::sample_maps::DiscSampleMaps::open(
        std::path::Path::new(&iso),
        |text_id| Some(format!("Private sample label {text_id}")),
    )
    .unwrap();
    let catalogue = samples.catalogue().unwrap();
    assert_eq!(catalogue.len(), 8);
    assert_eq!(
        catalogue.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        (0..8).collect::<Vec<_>>()
    );

    for sample in 0..8 {
        let payload = samples.load_sample(sample).unwrap();
        let document = MapDocument::parse(&payload).unwrap();
        assert_eq!(
            MapDocument::parse(&document.serialize().unwrap()).unwrap(),
            document
        );
    }

    eprintln!(
        "owned disc sample maps parsed and roundtripped: {}",
        catalogue.len()
    );
    assert!(samples.load_sample(8).is_err());
}
