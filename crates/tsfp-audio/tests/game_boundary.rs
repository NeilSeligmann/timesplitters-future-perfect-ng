use tsfp_audio::contract::{Command, Routing};
use tsfp_audio::game_boundary::{
    AbiContract, AudioObject, BufferCall, CallingConvention, DIRECTSOUND_METHOD_ABI,
    GameAudioBoundary, GuestAddress, ReturnRegister, SOUND_SUBMIT_ABI, STREAM_PREDICATE_ABI,
    SoundRequest, SoundToken, SpatialCall, StreamCall, SubmitResult, VoiceStart,
};
use tsfp_audio::recording::{Recording, Silent};

fn object() -> AudioObject {
    AudioObject {
        address: GuestAddress(0x1234),
        serial: 9,
    }
}

#[test]
fn request_record_round_trips_exact_30_byte_layout_and_defaults() {
    let defaults = SoundRequest::defaults(0x1648);
    let bytes = defaults.to_bytes();
    assert_eq!(bytes.len(), 0x30);
    assert_eq!(&bytes[0..4], &0x1648u32.to_le_bytes());
    assert_eq!(&bytes[4..8], &[0; 4]);
    assert_eq!(&bytes[8..12], &1.0f32.to_bits().to_le_bytes());
    assert_eq!(&bytes[0x10..0x14], &0x14u32.to_le_bytes());
    assert_eq!(&bytes[0x14..0x1c], &[0, 0, 0x80, 0x3f, 0, 0, 0x80, 0x3f]);
    assert_eq!(&bytes[0x28..0x2c], &[0; 4]);
    assert_eq!(&bytes[0x2c..], &50.0f32.to_bits().to_le_bytes());
    assert_eq!(SoundRequest::from_bytes(bytes), defaults);

    let request = SoundRequest {
        owner: GuestAddress(0xdead_beef),
        gain_bits: 0x7fc0_1234,
        reserved_0c: 0x8765_4321,
        flags: 0x220,
        ramp_bits: 0x8000_0000,
        scalar_bits: 0x3f00_0000,
        position_bits: [1, 2, 3],
        positional: 0xa5,
        reserved_29_2b: [4, 5, 6],
        ..defaults
    };
    assert_eq!(SoundRequest::from_bytes(request.to_bytes()), request);
}

#[test]
fn abi_and_submit_result_keep_the_two_return_contracts_distinct() {
    let check = |abi: AbiContract,
                 convention,
                 stack_bytes,
                 this_in_ecx,
                 result_register,
                 callee_pops_stack| {
        assert_eq!(
            abi,
            AbiContract {
                convention,
                stack_bytes,
                this_in_ecx,
                result_register,
                callee_pops_stack
            }
        );
    };
    check(
        SOUND_SUBMIT_ABI,
        CallingConvention::Cdecl,
        Some(4),
        false,
        ReturnRegister::Eax,
        false,
    );
    check(
        DIRECTSOUND_METHOD_ABI,
        CallingConvention::Stdcall,
        None,
        false,
        ReturnRegister::Eax,
        true,
    );
    check(
        STREAM_PREDICATE_ABI,
        CallingConvention::Thiscall,
        Some(0),
        true,
        ReturnRegister::Al,
        false,
    );
    for (raw, expected) in [
        (-1, SubmitResult::Failed),
        (-2, SubmitResult::QueueFull),
        (-3, SubmitResult::Duplicate),
        (-4, SubmitResult::Unclassified(-4)),
        (0x4000_002a, SubmitResult::Started(SoundToken(0x4000_002a))),
    ] {
        assert_eq!(SubmitResult::from_abi(raw), expected);
        assert_eq!(expected.to_abi(), raw);
    }
}

#[test]
fn typed_voice_buffer_spatial_and_stream_calls_reach_current_host_commands() {
    let mut host = Recording::new(Silent);
    let object = object();
    assert!(host.start_voice(VoiceStart {
        object,
        ticks: 11,
        frequency: 22042,
        pcm: vec![10, -10],
        paused: false,
        looping: true,
        loop_start: 1,
        loop_end: 2,
        millibels: -1600,
    }));
    assert!(host.buffer_call(BufferCall::Pause {
        object,
        ticks: 12,
        paused: true
    }));
    assert!(host.buffer_call(BufferCall::Stop { object, ticks: 12 }));
    assert!(host.buffer_call(BufferCall::Frequency {
        object,
        ticks: 12,
        hertz: 32000,
    }));
    assert!(host.buffer_call(BufferCall::Volume {
        object,
        ticks: 12,
        millibels: -3204,
    }));
    assert!(host.buffer_call(BufferCall::Loop {
        object,
        ticks: 13,
        start: 4,
        end: 8
    }));
    let routing = Routing {
        volume: 10,
        headroom: 20,
        aggregate: 30,
        count: 1,
        bins: [2; 8],
        bin_volume: [3; 32],
    };
    assert!(host.spatial_call(SpatialCall::RouteStream {
        object,
        ticks: 14,
        routing
    }));
    assert!(host.spatial_call(SpatialCall::EnableHeadroom { enabled: true }));
    assert!(host.spatial_call(SpatialCall::BindHeadroom { identity: 0x55 }));
    assert!(host.spatial_call(SpatialCall::SetHeadroom {
        identity: 0x55,
        ticks: 14,
        bin: 2,
        headroom: 3,
    }));
    assert!(host.stream_call(StreamCall::SubmitAdpcm {
        stream: GuestAddress(0x5678),
        ticks: 15,
        source_hz: 44100,
        mono: true,
        data: vec![1, 2, 3],
    }));
    assert!(host.stream_call(StreamCall::SubmitPcm16Stereo {
        stream: GuestAddress(0x5678),
        ticks: 15,
        source_hz: 48000,
        pcm: vec![4, -4],
    }));
    assert!(host.stream_call(StreamCall::Cut {
        stream: GuestAddress(0x5678),
        ticks: 15,
    }));
    assert!(host.stream_call(StreamCall::Running {
        stream: GuestAddress(0x5678),
        ticks: 16,
        running: false,
    }));
    assert!(host.stream_call(StreamCall::Frequency {
        stream: GuestAddress(0x5678),
        ticks: 16,
        source_hz: 22050,
    }));
    assert!(host.stream_call(StreamCall::Volume {
        stream: GuestAddress(0x5678),
        millibels: -1200,
    }));

    assert_eq!(
        host.commands(),
        vec![
            Command::PlayBuffer {
                key: 0x1234,
                serial: 9,
                ticks: 11,
                frequency: 22042,
                pcm: vec![10, -10],
                paused: false,
                looping: true,
                loop_start: 1,
                loop_end: 2,
                volume: -1600,
            },
            Command::PauseBuffer {
                key: 0x1234,
                serial: 9,
                ticks: 12,
                paused: true
            },
            Command::StopBuffer {
                key: 0x1234,
                serial: 9,
                ticks: 12,
            },
            Command::FrequencyBuffer {
                key: 0x1234,
                serial: 9,
                ticks: 12,
                frequency: 32000,
            },
            Command::VolumeBuffer {
                key: 0x1234,
                serial: 9,
                ticks: 12,
                volume: -3204,
            },
            Command::LoopBuffer {
                key: 0x1234,
                serial: 9,
                ticks: 13,
                loop_start: 4,
                loop_end: 8
            },
            Command::RouteStream {
                key: 0x1234,
                serial: 9,
                ticks: 14,
                routing
            },
            Command::EnableHeadroom { enabled: true },
            Command::BindHeadroom { identity: 0x55 },
            Command::SetHeadroom {
                identity: 0x55,
                ticks: 14,
                bin: 2,
                headroom: 3,
            },
            Command::SubmitAdpcm {
                key: 0x5678,
                ticks: 15,
                source_hz: 44100,
                mono: true,
                data: vec![1, 2, 3]
            },
            Command::SubmitPcm16Stereo {
                key: 0x5678,
                ticks: 15,
                source_hz: 48000,
                pcm: vec![4, -4],
            },
            Command::CutStream {
                key: 0x5678,
                ticks: 15,
            },
            Command::SetStreamRunning {
                key: 0x5678,
                ticks: 16,
                running: false
            },
            Command::SetStreamFrequency {
                key: 0x5678,
                ticks: 16,
                source_hz: 22050,
            },
            Command::SetStreamVolume {
                key: 0x5678,
                millibels: -1200,
            },
        ]
    );
}
