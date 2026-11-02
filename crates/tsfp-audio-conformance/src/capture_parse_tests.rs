use super::*;

#[test]
fn parses_all_scalar_contract_fields_without_narrowing_or_reordering() {
    use Command::*;
    let cases = [
        (
            "CUT 17 4294967297 0",
            CutStream {
                key: 17,
                ticks: 4294967297,
            },
        ),
        (
            "VOL 17 -2400 0",
            SetStreamVolume {
                key: 17,
                millibels: -2400,
            },
        ),
        ("RVOL 17 0", ResetStreamVolume { key: 17 }),
        (
            "RUN 17 4294967297 1 0",
            SetStreamRunning {
                key: 17,
                ticks: 4294967297,
                running: true,
            },
        ),
        (
            "FRQ 17 4294967297 22050 0",
            SetStreamFrequency {
                key: 17,
                ticks: 4294967297,
                source_hz: 22050,
            },
        ),
        (
            "FS 17 4294967298 4294967297 44100 22050 0",
            FormatStream {
                key: 17,
                serial: 4294967298,
                ticks: 4294967297,
                old_hz: 44100,
                new_hz: 22050,
            },
        ),
        (
            "FQ 17 4294967298 4294967297 48000 44100 22050 0",
            FrequencyStream {
                key: 17,
                serial: 4294967298,
                ticks: 4294967297,
                base_hz: 48000,
                old_hz: 44100,
                new_hz: 22050,
            },
        ),
        ("HE 1 0", EnableHeadroom { enabled: true }),
        (
            "HB 4294967297 0",
            BindHeadroom {
                identity: 4294967297,
            },
        ),
        (
            "HS 4294967297 4294967298 3 7 0",
            SetHeadroom {
                identity: 4294967297,
                ticks: 4294967298,
                bin: 3,
                headroom: 7,
            },
        ),
        (
            "PAB 17 4294967298 4294967297 1 0",
            PauseBuffer {
                key: 17,
                serial: 4294967298,
                ticks: 4294967297,
                paused: true,
            },
        ),
        (
            "STB 17 4294967298 4294967297 0",
            StopBuffer {
                key: 17,
                serial: 4294967298,
                ticks: 4294967297,
            },
        ),
        (
            "FRB 17 4294967298 4294967297 22050 0",
            FrequencyBuffer {
                key: 17,
                serial: 4294967298,
                ticks: 4294967297,
                frequency: 22050,
            },
        ),
        (
            "VLB 17 4294967298 4294967297 -3200 0",
            VolumeBuffer {
                key: 17,
                serial: 4294967298,
                ticks: 4294967297,
                volume: -3200,
            },
        ),
        (
            "LPB 17 4294967298 4294967297 3 9 0",
            LoopBuffer {
                key: 17,
                serial: 4294967298,
                ticks: 4294967297,
                loop_start: 3,
                loop_end: 9,
            },
        ),
    ];
    for (text, expected) in cases {
        // Check the independent live-recorder encoder against the pinned C
        // grammar, including refusal bits and values wider than u32.
        let mut recorder = tsfp_audio_observe::Recorder::new(Vec::new(), 48000, 48000).unwrap();
        recorder.command(&expected, false, None).unwrap();
        let encoded = String::from_utf8(recorder.finish().unwrap()).unwrap();
        assert_eq!(encoded.lines().nth(2).unwrap(), text);
        let Record::Apply(command, accepted) = parse(text).unwrap() else {
            panic!("not a command")
        };
        assert_eq!(command, expected);
        assert!(!accepted);
        let accepted_text = format!("{}1", &text[..text.len() - 1]);
        assert!(matches!(
            parse(&accepted_text).unwrap(),
            Record::Apply(_, true)
        ));
    }
    assert!(matches!(
        parse("RST").unwrap(),
        Record::Apply(ResetBuffers, true)
    ));
}

#[test]
fn recorder_preserves_payloads_routing_and_reset_in_the_c_grammar() {
    let route = Routing {
        volume: u32::MAX,
        headroom: 2,
        aggregate: 3,
        count: 8,
        bins: std::array::from_fn(|i| i as u8 + 1),
        bin_volume: std::array::from_fn(|i| u32::MAX - i as u32),
    };
    for command in [
        Command::SubmitAdpcm {
            key: 17,
            ticks: u64::MAX,
            source_hz: 22050,
            mono: true,
            data: vec![0, 128, 255],
        },
        Command::SubmitAdpcm {
            key: 17,
            ticks: 31,
            source_hz: 48000,
            mono: false,
            data: vec![],
        },
        Command::SubmitPcm16Stereo {
            key: 17,
            ticks: 31,
            source_hz: 44100,
            pcm: vec![i16::MIN, i16::MAX, -1, 0],
        },
        Command::RouteStream {
            key: 17,
            serial: u64::MAX,
            ticks: 31,
            routing: route,
        },
        Command::ResetBuffers,
    ] {
        let mut recorder = tsfp_audio_observe::Recorder::new(Vec::new(), 48000, 48000).unwrap();
        recorder.command(&command, true, None).unwrap();
        let encoded = String::from_utf8(recorder.finish().unwrap()).unwrap();
        let Record::Apply(decoded, accepted) = parse(encoded.lines().nth(2).unwrap()).unwrap()
        else {
            panic!()
        };
        assert!(accepted);
        assert_eq!(decoded, command);
    }
}

#[test]
fn payload_and_routing_fields_preserve_signed_samples_and_all_bins() {
    let Record::Apply(Command::SubmitPcm16Stereo { pcm, .. }, true) =
        parse("SP 17 99 22050 1 0080ff7f").unwrap()
    else {
        panic!()
    };
    assert_eq!(pcm, [-32768, 32767]);
    let Record::Apply(
        Command::SubmitAdpcm {
            key,
            ticks,
            source_hz,
            mono,
            data,
        },
        false,
    ) = parse("SA 17 99 22050 1 0 01AB").unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (key, ticks, source_hz, mono, data),
        (17, 99, 22050, true, vec![1, 171])
    );
    let bins: [u8; 8] = std::array::from_fn(|i| i as u8 + 2);
    let bin_volume: [u32; 32] = std::array::from_fn(|i| u32::MAX - i as u32);
    let suffix = bins
        .iter()
        .map(|n| n.to_string())
        .chain(bin_volume.iter().map(|n| n.to_string()))
        .collect::<Vec<_>>()
        .join(" ");
    let Record::Apply(
        Command::RouteStream {
            key,
            serial,
            ticks,
            routing,
        },
        true,
    ) = parse(&format!("RT 17 23 31 43 47 53 8 1 {suffix}")).unwrap()
    else {
        panic!()
    };
    assert_eq!((key, serial, ticks), (17, 23, 31));
    assert_eq!(
        routing,
        Routing {
            volume: 43,
            headroom: 47,
            aggregate: 53,
            count: 8,
            bins,
            bin_volume
        }
    );
}

#[test]
fn buffer_payload_uses_independent_c_and_rust_decoders() {
    let payload = format!("34120000{}", "11".repeat(32));
    let Record::Apply(
        Command::PlayBuffer {
            key,
            serial,
            ticks,
            frequency,
            pcm,
            paused,
            looping,
            loop_start,
            loop_end,
            volume,
        },
        true,
    ) = parse(&format!("PB 17 23 31 22050 1 0 3 9 -2400 1 {payload}")).unwrap()
    else {
        panic!()
    };
    assert_eq!((key, serial, ticks, frequency), (17, 23, 31, 22050));
    assert_eq!(
        (paused, looping, loop_start, loop_end, volume),
        (true, false, 3, 9, -2400)
    );
    assert_eq!(pcm.len(), 64);
    assert_eq!(pcm[0], 0x1234);
    let invalid = payload.replacen("34120000", "3412ff00", 1);
    assert!(matches!(
        parse(&format!("PB 17 23 31 22050 1 0 3 9 -2400 0 {invalid}")).unwrap(),
        Record::RefusedBuffer
    ));
    assert!(parse(&format!("PB 17 23 31 22050 1 0 3 9 -2400 1 {invalid}")).is_err());
}

#[test]
fn strict_fields_reject_truncation_overflow_unknown_and_unsafe_allocations() {
    for line in [
        "",
        "FUTURE 1",
        "START 0 10",
        "START 48000 0",
        "START 384001 1",
        "START 48000 1 9",
        "HE 2 1",
        "HE 1 9",
        "VOL 4294967296 -1 1",
        "VOL 1 -2147483649 1",
        "SP 1 0 48000 1 000",
        "SP 1 0 48000 1 0000",
        "SA 1 0 48000 1 1 zz",
        "R 1 1048577 0 0000000000000000",
        "R 1 2 3 0000000000000000",
        "R 1 1 1 xyz",
        "CCFG",
        "CW -1",
    ] {
        assert!(parse(line).is_err(), "accepted malformed record");
    }
}

#[test]
fn completion_record_field_counts_are_pinned() {
    for (op, fields) in [
        ("CCFG", 1),
        ("CW", 1),
        ("CPN", 3),
        ("CBS", 3),
        ("CSD", 4),
        ("CBG", 4),
        ("CSS", 5),
        ("CBA", 5),
        ("CBF", 5),
        ("CSM", 6),
        ("CBL", 6),
        ("CSF", 7),
        ("CSP", 10),
        ("CBP", 10),
    ] {
        assert!(matches!(
            parse(&format!("{op}{}", " 0".repeat(fields))).unwrap(),
            Record::Completion
        ));
    }
}
