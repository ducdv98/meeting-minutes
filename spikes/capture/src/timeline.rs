pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: usize = 2;
const HNS_PER_SECOND: u64 = 10_000_000;
const GAP_TOLERANCE_FRAMES: u64 = 480;
const IDLE_MARGIN_HNS: u64 = 2_000_000;

#[derive(Default, Debug, PartialEq)]
pub struct PacketResult {
    pub samples: Vec<f32>,
    pub gap_frames_inserted: u64,
    pub overlap_frames_dropped: u64,
    pub timestamp_source: &'static str,
    pub placed_qpc_100ns: u64,
}

pub struct Timeline {
    start_qpc: u64,
    frames_written: u64,
    previous_packet_qpc: Option<u64>,
}

impl Timeline {
    pub fn new(start_qpc: u64) -> Self {
        Self {
            start_qpc,
            frames_written: 0,
            previous_packet_qpc: None,
        }
    }

    pub fn frames_written(&self) -> u64 {
        self.frames_written
    }

    fn frame_at(&self, qpc: u64) -> u64 {
        ((qpc.saturating_sub(self.start_qpc) as u128 * SAMPLE_RATE as u128)
            / HNS_PER_SECOND as u128) as u64
    }

    pub fn packet(
        &mut self,
        qpc: Option<u64>,
        arrival_qpc: u64,
        data: &[f32],
        silent: bool,
        discontinuity: bool,
    ) -> PacketResult {
        assert_eq!(data.len() % CHANNELS, 0);
        let usable = qpc.filter(|&t| {
            t >= self.start_qpc
                && t.abs_diff(arrival_qpc) <= 2 * HNS_PER_SECOND
                && self.previous_packet_qpc.is_none_or(|previous| t > previous)
        });
        let (time, timestamp_source) = match usable {
            Some(t) => (t, "packet"),
            None => (arrival_qpc, "arrival"),
        };
        if let Some(t) = usable {
            self.previous_packet_qpc = Some(t);
        }
        let packet_start = self.frame_at(time);
        let packet_frames = (data.len() / CHANNELS) as u64;
        let mut result = PacketResult {
            timestamp_source,
            placed_qpc_100ns: time,
            ..PacketResult::default()
        };
        if packet_start > self.frames_written
            && (discontinuity || packet_start - self.frames_written > GAP_TOLERANCE_FRAMES)
        {
            result.gap_frames_inserted = packet_start - self.frames_written;
            result
                .samples
                .resize(result.gap_frames_inserted as usize * CHANNELS, 0.0);
            self.frames_written = packet_start;
        }
        let dropped = self
            .frames_written
            .saturating_sub(packet_start)
            .min(packet_frames);
        result.overlap_frames_dropped = dropped;
        let remaining = packet_frames - dropped;
        if silent {
            result
                .samples
                .resize(result.samples.len() + remaining as usize * CHANNELS, 0.0);
        } else {
            result
                .samples
                .extend_from_slice(&data[dropped as usize * CHANNELS..]);
        }
        self.frames_written += remaining;
        result
    }

    pub fn pad_to(&mut self, now_qpc: u64, final_pad: bool) -> Vec<f32> {
        let target = self.frame_at(if final_pad {
            now_qpc
        } else {
            now_qpc.saturating_sub(IDLE_MARGIN_HNS)
        });
        let frames = target.saturating_sub(self.frames_written);
        self.frames_written += frames;
        vec![0.0; frames as usize * CHANNELS]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: u64 = 1_000_000_000;
    fn packet(frames: usize) -> Vec<f32> {
        vec![1.0; frames * CHANNELS]
    }

    #[test]
    fn inserts_gap_and_silent_packet_zeros() {
        let mut t = Timeline::new(START);
        let first = t.packet(Some(START), START, &packet(480), false, false);
        assert_eq!(first.samples.len(), 960);
        let second = t.packet(
            Some(START + 300_000),
            START + 300_000,
            &packet(480),
            true,
            false,
        );
        assert_eq!(second.gap_frames_inserted, 960);
        assert_eq!(second.samples.len(), 1440 * CHANNELS);
        assert!(second.samples.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn idle_pad_advances_and_later_packet_trims_overlap() {
        let mut t = Timeline::new(START);
        assert_eq!(t.pad_to(START + 5_000_000, false).len(), 14_400 * CHANNELS);
        let result = t.packet(
            Some(START + 2_000_000),
            START + 3_100_000,
            &packet(9_600),
            false,
            false,
        );
        assert_eq!(result.overlap_frames_dropped, 4_800);
        assert_eq!(result.samples.len(), 4_800 * CHANNELS);
    }

    #[test]
    fn final_pad_reaches_stop_time() {
        let mut t = Timeline::new(START);
        t.pad_to(START + 5_000_000, false);
        assert_eq!(t.pad_to(START + 5_000_000, true).len(), 9_600 * CHANNELS);
        assert_eq!(t.frames_written(), 24_000);
    }

    #[test]
    fn bad_timestamp_uses_arrival() {
        let mut t = Timeline::new(START);
        let result = t.packet(None, START + 200_000, &packet(480), false, false);
        assert_eq!(result.timestamp_source, "arrival");
        assert_eq!(result.gap_frames_inserted, 960);
    }

    #[test]
    fn discontinuity_inserts_small_forward_gap() {
        let mut t = Timeline::new(START);
        t.packet(Some(START), START, &packet(480), false, false);
        let result = t.packet(
            Some(START + 150_000),
            START + 150_000,
            &packet(480),
            false,
            true,
        );
        assert_eq!(result.gap_frames_inserted, 240);
        assert_eq!(result.samples.len(), 720 * CHANNELS);
        assert_eq!(t.frames_written(), 1_200);

        let mut without_flag = Timeline::new(START);
        without_flag.packet(Some(START), START, &packet(480), false, false);
        let unflagged = without_flag.packet(
            Some(START + 150_000),
            START + 150_000,
            &packet(480),
            false,
            false,
        );
        assert_eq!(unflagged.gap_frames_inserted, 0);
    }

    #[test]
    fn repeated_packet_qpc_uses_arrival_for_placement() {
        let mut t = Timeline::new(START);
        t.packet(Some(START), START, &packet(480), false, false);
        let arrival = START + 300_000;
        let result = t.packet(Some(START), arrival, &packet(480), false, false);
        assert_eq!(result.timestamp_source, "arrival");
        assert_eq!(result.placed_qpc_100ns, arrival);
        assert_eq!(result.gap_frames_inserted, 960);
    }
}
