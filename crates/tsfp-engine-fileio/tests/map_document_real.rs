//! Opt-in private-file conformance. No disc or save bytes are tracked.
use tsfp_engine_fileio::{
    map_document::MapDocument, map_nodes::WireTransform, storage_flow::MapPayload,
};
#[test]
#[ignore = "requires TSFP_TEST_SIGNED_MAP pointing at a private signed map"]
fn private_signed_map_node_codec() {
    let path = std::env::var_os("TSFP_TEST_SIGNED_MAP").expect("private map path");
    let bytes = std::fs::read(path).unwrap();
    let payload = MapPayload::new(bytes[..bytes.len() - 20].to_vec()).unwrap();
    let document = MapDocument::parse(&payload).unwrap();
    let nodes = document.nodes();
    assert_eq!(
        nodes.groups().len(),
        document.settings().node_count as usize
    );
    assert!(!nodes.groups().is_empty());
    let reparsed = MapDocument::parse(&document.serialize().unwrap()).unwrap();
    assert_eq!(reparsed.nodes(), nodes);
    let first = nodes.groups()[0].transform();
    let transform = WireTransform {
        position: [
            first.position[0].wrapping_add(1),
            first.position[1],
            first.position[2],
        ],
        rotation: first.rotation,
    };
    let changed = document.with_node_transform(0, transform).unwrap();
    let changed_bytes = changed.serialize().unwrap();
    let original_bytes = document.serialize().unwrap();
    let used =
        u32::from_le_bytes(original_bytes.bytes()[0x258..0x25c].try_into().unwrap()) as usize;
    assert_eq!(
        original_bytes.bytes()[..used]
            .iter()
            .zip(&changed_bytes.bytes()[..used])
            .filter(|(a, b)| a != b)
            .count(),
        1
    );
    assert_eq!(
        MapDocument::parse(&changed_bytes).unwrap().nodes().groups()[0].transform(),
        transform
    );
    eprintln!(
        "groups={} entries={} prefix_min={} prefix_max={}",
        nodes.groups().len(),
        nodes
            .groups()
            .iter()
            .map(|group| group.entries().len())
            .sum::<usize>(),
        nodes
            .groups()
            .iter()
            .map(|group| group.prefix_bytes().len())
            .min()
            .unwrap(),
        nodes
            .groups()
            .iter()
            .map(|group| group.prefix_bytes().len())
            .max()
            .unwrap()
    );
}
