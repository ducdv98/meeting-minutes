use crate::recording::Marker;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub timeline_start_ms: u64,
    pub markers: Vec<Marker>,
}

/// Places a Recording after the Meeting's earlier Recordings.
pub fn place_recording(earlier: &[u64], duration_ms: u64, markers: &[Marker]) -> Placement {
    let timeline_start_ms = earlier.iter().sum();
    let mut placed = Vec::with_capacity(markers.len() + usize::from(!earlier.is_empty()));
    if !earlier.is_empty() {
        placed.push(Marker::Gap {
            at_ms: timeline_start_ms,
        });
    }
    placed.extend(markers.iter().map(|marker| match marker {
        Marker::Pause { at_ms, paused_ms } => Marker::Pause {
            at_ms: timeline_start_ms + (*at_ms).min(duration_ms),
            paused_ms: *paused_ms,
        },
        Marker::Gap { at_ms } => Marker::Gap {
            at_ms: timeline_start_ms + (*at_ms).min(duration_ms),
        },
    }));
    Placement {
        timeline_start_ms,
        markers: placed,
    }
}
