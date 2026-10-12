use meeting_minutes_core::{capture::Source, recording::Marker, timeline::place_recording};

#[test]
fn device_switch_is_offset_clamped_and_keeps_its_details() {
    let marker = Marker::DeviceSwitch {
        at_ms: 500,
        source: Source::Microphone,
        device: "Logi USB Headset".into(),
    };
    let placement = place_recording(&[1_000], 200, &[marker]);
    assert_eq!(
        placement.markers,
        vec![
            Marker::Gap { at_ms: 1_000 },
            Marker::DeviceSwitch {
                at_ms: 1_200,
                source: Source::Microphone,
                device: "Logi USB Headset".into(),
            },
        ]
    );
    assert_eq!(
        serde_json::to_value(&placement.markers[1]).unwrap(),
        serde_json::json!({
            "kind": "device_switch", "at_ms": 1_200,
            "source": "microphone", "device": "Logi USB Headset"
        })
    );
}

#[test]
fn first_recording_keeps_relative_pauses_without_a_gap() {
    let pause = Marker::Pause {
        at_ms: 300,
        paused_ms: 200,
    };
    let placement = place_recording(&[], 1_000, std::slice::from_ref(&pause));
    assert_eq!(placement.timeline_start_ms, 0);
    assert_eq!(placement.markers, vec![pause]);
}

#[test]
fn later_recordings_start_at_the_sum_with_gap_before_offset_pauses() {
    let placement = place_recording(
        &[1_000, 2_000],
        500,
        &[Marker::Pause {
            at_ms: 100,
            paused_ms: 50,
        }],
    );
    assert_eq!(placement.timeline_start_ms, 3_000);
    assert_eq!(
        placement.markers,
        vec![
            Marker::Gap { at_ms: 3_000 },
            Marker::Pause {
                at_ms: 3_100,
                paused_ms: 50
            },
        ]
    );
}

#[test]
fn recovered_pause_is_clamped_to_the_recording_end() {
    let placement = place_recording(
        &[1_000],
        200,
        &[Marker::Pause {
            at_ms: 500,
            paused_ms: 20,
        }],
    );
    assert_eq!(
        placement.markers,
        vec![
            Marker::Gap { at_ms: 1_000 },
            Marker::Pause {
                at_ms: 1_200,
                paused_ms: 20
            },
        ]
    );
}

#[test]
fn three_recordings_keep_each_pause_on_the_meeting_timeline() {
    let first = place_recording(
        &[],
        500,
        &[Marker::Pause {
            at_ms: 100,
            paused_ms: 20,
        }],
    );
    let second = place_recording(
        &[500],
        700,
        &[Marker::Pause {
            at_ms: 300,
            paused_ms: 40,
        }],
    );
    let third = place_recording(
        &[500, 700],
        200,
        &[Marker::Pause {
            at_ms: 50,
            paused_ms: 10,
        }],
    );
    let markers = [first.markers, second.markers, third.markers].concat();
    assert_eq!(
        markers,
        vec![
            Marker::Pause {
                at_ms: 100,
                paused_ms: 20
            },
            Marker::Gap { at_ms: 500 },
            Marker::Pause {
                at_ms: 800,
                paused_ms: 40
            },
            Marker::Gap { at_ms: 1_200 },
            Marker::Pause {
                at_ms: 1_250,
                paused_ms: 10
            },
        ]
    );
}
