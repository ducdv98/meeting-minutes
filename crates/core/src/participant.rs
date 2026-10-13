use std::collections::BTreeMap;

use crate::transcript::{Segment, Speaker};

pub type MeetingParticipants = BTreeMap<String, String>;

pub fn label_key(label: Speaker) -> &'static str {
    match label {
        Speaker::Me => "me",
        Speaker::Remote => "remote",
        Speaker::Unknown => "unknown",
    }
}

pub fn default_name(label: Speaker, settings_name: Option<&str>) -> &str {
    match label {
        Speaker::Me => settings_name.map(str::trim).filter(|name| !name.is_empty()).unwrap_or("Me"),
        Speaker::Remote => "Remote",
        Speaker::Unknown => "Unknown",
    }
}

pub fn participant_name(participants: &MeetingParticipants, settings_name: Option<&str>, is_voice_note: bool, segment: &Segment) -> String {
    let label = if is_voice_note { Speaker::Me } else { segment.speaker };
    participants.get(label_key(label)).cloned().unwrap_or_else(|| default_name(label, settings_name).to_owned())
}
