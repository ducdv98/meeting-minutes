use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Microphone,
    SystemAudio,
}

#[derive(Clone, Debug, Serialize)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits: u16,
    pub sample_type: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PacketFlags {
    pub silent: bool,
    pub discontinuity: bool,
    pub timestamp_error: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Start {
        minutes: f64,
        out: String,
        role: &'static str,
        start_qpc_100ns: u64,
    },
    DeviceFormat {
        source: Source,
        role: &'static str,
        device_id: String,
        device_name: String,
        mix_format: AudioFormat,
        output_format: AudioFormat,
    },
    Packet {
        source: Source,
        qpc_100ns: u64,
        arrival_qpc_100ns: u64,
        placed_qpc_100ns: u64,
        timestamp_source: &'static str,
        device_position: u64,
        frames: u32,
        flags: PacketFlags,
        gap_frames_inserted: u64,
        overlap_frames_dropped: u64,
    },
    IdlePad {
        source: Source,
        frames: u64,
    },
    DeviceSwitch {
        source: Source,
        from_device: Option<String>,
        to_device: Option<String>,
        reason: String,
    },
    Error {
        source: Option<Source>,
        message: String,
    },
    Stop {
        microphone_frames: u64,
        system_audio_frames: u64,
        elapsed_qpc_100ns: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn all_events_have_tagged_json_shape() {
        let format = AudioFormat {
            sample_rate: 48_000,
            channels: 2,
            bits: 32,
            sample_type: "float".into(),
        };
        let events = [
            Event::Start {
                minutes: 0.5,
                out: "take".into(),
                role: "communications",
                start_qpc_100ns: 123,
            },
            Event::DeviceFormat {
                source: Source::Microphone,
                role: "console",
                device_id: "id".into(),
                device_name: "Mic".into(),
                mix_format: format.clone(),
                output_format: format,
            },
            Event::Packet {
                source: Source::SystemAudio,
                qpc_100ns: 123,
                arrival_qpc_100ns: 125,
                placed_qpc_100ns: 123,
                timestamp_source: "packet",
                device_position: 42,
                frames: 480,
                flags: PacketFlags {
                    silent: false,
                    discontinuity: true,
                    timestamp_error: false,
                },
                gap_frames_inserted: 0,
                overlap_frames_dropped: 0,
            },
            Event::IdlePad {
                source: Source::SystemAudio,
                frames: 480,
            },
            Event::DeviceSwitch {
                source: Source::Microphone,
                from_device: Some("old".into()),
                to_device: Some("new".into()),
                reason: "default_changed".into(),
            },
            Event::Error {
                source: Some(Source::Microphone),
                message: "test".into(),
            },
            Event::Stop {
                microphone_frames: 100,
                system_audio_frames: 100,
                elapsed_qpc_100ns: 20_833,
            },
        ];
        let names = [
            "start",
            "device_format",
            "packet",
            "idle_pad",
            "device_switch",
            "error",
            "stop",
        ];
        for (event, name) in events.iter().zip(names) {
            let json: Value = serde_json::to_value(event).unwrap();
            assert_eq!(json["event"], name);
        }
        let start = serde_json::to_value(&events[0]).unwrap();
        assert_eq!(start["minutes"], 0.5);
        assert_eq!(start["role"], "communications");
        assert_eq!(start["start_qpc_100ns"], 123);
        let device = serde_json::to_value(&events[1]).unwrap();
        assert_eq!(device["source"], "microphone");
        assert_eq!(device["role"], "console");
        assert_eq!(device["mix_format"]["sample_rate"], 48_000);
        assert_eq!(device["output_format"]["sample_type"], "float");
        let packet = serde_json::to_value(&events[2]).unwrap();
        assert_eq!(packet["source"], "system_audio");
        assert_eq!(packet["flags"]["discontinuity"], true);
        assert_eq!(packet["timestamp_source"], "packet");
        assert_eq!(packet["qpc_100ns"], 123);
        assert_eq!(packet["arrival_qpc_100ns"], 125);
        assert_eq!(packet["placed_qpc_100ns"], 123);
        assert_eq!(packet["device_position"], 42);
        let idle = serde_json::to_value(&events[3]).unwrap();
        assert_eq!(idle["frames"], 480);
        let switch = serde_json::to_value(&events[4]).unwrap();
        assert_eq!(switch["to_device"], "new");
        assert_eq!(switch["reason"], "default_changed");
        let error = serde_json::to_value(&events[5]).unwrap();
        assert_eq!(error["message"], "test");
        let stop = serde_json::to_value(&events[6]).unwrap();
        assert_eq!(stop["microphone_frames"], 100);
        assert_eq!(stop["elapsed_qpc_100ns"], 20_833);
    }
}
