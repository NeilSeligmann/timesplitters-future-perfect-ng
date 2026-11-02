//! Spec tests for the file-information decision table (T2074). FABRICATED inputs, Tier 0/1.
use tsfp_engine_fileio::info::*;

fn file() -> FileFacts {
    FileFacts {
        offset: 0x1_0000_0002,
        size: 5000,
        ..Default::default()
    }
}

#[test]
fn position_is_the_offset_and_checks_length() {
    let reply = query(CLASS_POSITION, 8, &file());
    assert_eq!((reply.status, reply.information), (0, 8));
    assert_eq!(reply.payload, 0x1_0000_0002u64.to_le_bytes());
    assert_eq!(
        query(CLASS_POSITION, 7, &file()).status,
        STATUS_INFO_LENGTH_MISMATCH
    );
}

#[test]
fn network_open_rounds_allocation_unless_host_dir() {
    let disc = query(CLASS_NETWORK_OPEN, 0x38, &file());
    assert_eq!(disc.information, 0x38);
    assert!(disc.fabricated_timestamps);
    assert_eq!(
        u64::from_le_bytes(disc.payload[0x20..0x28].try_into().unwrap()),
        6144
    );
    assert_eq!(
        u64::from_le_bytes(disc.payload[0x28..0x30].try_into().unwrap()),
        5000
    );
    assert_eq!(
        u32::from_le_bytes(disc.payload[0x30..0x34].try_into().unwrap()),
        0x20
    );
    let host = query(
        CLASS_NETWORK_OPEN,
        0x38,
        &FileFacts {
            host_dir: true,
            is_directory: true,
            ..file()
        },
    );
    assert_eq!(
        u64::from_le_bytes(host.payload[0x20..0x28].try_into().unwrap()),
        5000
    );
    assert_eq!(
        u32::from_le_bytes(host.payload[0x30..0x34].try_into().unwrap()),
        0x10
    );
    assert_eq!(
        query(CLASS_NETWORK_OPEN, 0x37, &file()).status,
        STATUS_INFO_LENGTH_MISMATCH
    );
}

#[test]
fn internal_only_on_a_disc_file_and_other_classes_are_refused() {
    assert!(query(CLASS_INTERNAL, 8, &file()).unknown_class);
    let reply = query(
        CLASS_INTERNAL,
        8,
        &FileFacts {
            disc_file: true,
            disc_sector: 77,
            ..file()
        },
    );
    assert_eq!(reply.payload, 77u64.to_le_bytes());
    for class in [4, 5, 10, 0x2F] {
        let r = query(class, 64, &file());
        assert_eq!(
            (r.status, r.unknown_class),
            (STATUS_INVALID_INFO_CLASS, true)
        );
    }
}

#[test]
fn set_plans() {
    assert_eq!(set_plan(CLASS_POSITION, 8, Some(9)), SetPlan::Seek(9));
    assert_eq!(
        set_plan(CLASS_ALLOCATION, 8, Some(4096)),
        SetPlan::Allocation(4096)
    );
    assert_eq!(
        set_plan(CLASS_END_OF_FILE, 16, Some(1)),
        SetPlan::EndOfFile(1)
    );
    assert_eq!(
        set_plan(CLASS_END_OF_FILE, 4, Some(1)),
        SetPlan::Refuse {
            status: STATUS_INFO_LENGTH_MISMATCH,
            unknown_class: false
        }
    );
    assert_eq!(
        set_plan(CLASS_END_OF_FILE, 8, None),
        SetPlan::Refuse {
            status: STATUS_INFO_LENGTH_MISMATCH,
            unknown_class: false
        }
    );
    assert_eq!(
        set_plan(10, 8, Some(1)),
        SetPlan::Refuse {
            status: STATUS_INVALID_INFO_CLASS,
            unknown_class: true
        }
    );
}

/// T3786 (I-T3786-1): XDeleteSaveGame clears a save directory's attributes (class 4, Length 40) before deleting it;
/// a refusal left every deleted profile's empty directory listed.
#[test]
fn basic_information_is_accepted_for_the_recursive_delete() {
    assert_eq!(set_plan(CLASS_BASIC, 40, Some(0)), SetPlan::Basic);
    assert_eq!(set_plan(CLASS_BASIC, BASIC_BYTES, Some(0)), SetPlan::Basic);
    assert_eq!(
        set_plan(CLASS_BASIC, BASIC_BYTES - 1, Some(0)),
        SetPlan::Refuse {
            status: STATUS_INFO_LENGTH_MISMATCH,
            unknown_class: false
        }
    );
    assert_eq!(
        set_plan(CLASS_BASIC, 40, None),
        SetPlan::Refuse {
            status: STATUS_INFO_LENGTH_MISMATCH,
            unknown_class: false
        }
    );
}
