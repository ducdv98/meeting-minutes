use meeting_minutes_core::{
    capture::{Clock, Packet, Source, SourceClock, HNS_PER_SECOND, SAMPLE_RATE},
    recording::{Marker, RecordingClock},
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

const START: u64 = 10_000_000;
const BUFFER_FRAMES: u64 = 480;
const BUFFER_TICKS: u64 = 100_000;

#[derive(Clone)]
struct FakeClock(Arc<AtomicU64>);

impl FakeClock {
    fn new() -> Self {
        Self(Arc::new(AtomicU64::new(START)))
    }

    fn set(&self, at_100ns: u64) {
        self.0.store(at_100ns, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_100ns(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

struct FakeSources {
    microphone: SourceClock,
    system_audio: SourceClock,
}

impl FakeSources {
    fn new() -> Self {
        Self {
            microphone: SourceClock::new(START),
            system_audio: SourceClock::new(START),
        }
    }

    fn get_mut(&mut self, source: Source) -> &mut SourceClock {
        match source {
            Source::Microphone => &mut self.microphone,
            Source::SystemAudio => &mut self.system_audio,
        }
    }

    fn send(
        &mut self,
        source: Source,
        clock: &FakeClock,
        host_100ns: u64,
        frames: u64,
        value: f32,
        discontinuity: bool,
    ) -> Vec<f32> {
        self.get_mut(source).packet(
            &Packet {
                samples: vec![value; frames as usize * 2],
                host_100ns: Some(host_100ns),
                silent: false,
                discontinuity,
            },
            clock.now_100ns(),
        )
    }
}

#[test]
fn two_sources_pad_gaps_drop_overlaps_and_exclude_pause_from_timeline() {
    let clock = FakeClock::new();
    let mut sources = FakeSources::new();
    let mut timeline = RecordingClock::new(clock.now_100ns());
    let frames = 480;
    let ten_ms = 100_000;

    for (source, value) in [(Source::Microphone, 0.25), (Source::SystemAudio, 0.5)] {
        assert_eq!(
            sources.send(source, &clock, START, frames, value, false),
            vec![value; 960]
        );
    }

    clock.set(START + 3 * ten_ms);
    for (source, value) in [(Source::Microphone, 0.25), (Source::SystemAudio, 0.5)] {
        let placed = sources.send(source, &clock, clock.now_100ns(), frames, value, true);
        assert_eq!(&placed[..960 * 2], &vec![0.0; 960 * 2]);
        assert_eq!(&placed[960 * 2..], &vec![value; 960]);
    }

    clock.set(START + 35 * 10_000);
    for (source, value) in [(Source::Microphone, 0.25), (Source::SystemAudio, 0.5)] {
        assert_eq!(
            sources.send(source, &clock, clock.now_100ns(), frames, value, false),
            vec![value; 480]
        );
    }

    clock.set(START + 5 * ten_ms);
    timeline.pause(clock.now_100ns());
    for source in [Source::Microphone, Source::SystemAudio] {
        assert_eq!(
            sources.get_mut(source).pause(clock.now_100ns()),
            vec![0.0; 480]
        );
    }
    clock.set(START + 105 * ten_ms);
    timeline.resume(clock.now_100ns());
    for source in [Source::Microphone, Source::SystemAudio] {
        sources.get_mut(source).resume(clock.now_100ns());
        assert!(sources
            .send(source, &clock, START + ten_ms, frames, 1.0, false)
            .is_empty());
        assert_eq!(
            sources.send(source, &clock, clock.now_100ns(), frames, 1.0, false),
            vec![1.0; 960]
        );
        assert_eq!(sources.get_mut(source).frames_written(), 2_880);
    }
    clock.set(START + 106 * ten_ms);
    assert_eq!(timeline.elapsed_ms(clock.now_100ns()), 60);
    assert_eq!(
        timeline.markers(),
        &[Marker::Pause {
            at_ms: 50,
            paused_ms: 1000
        }]
    );
}

fn timeline_frames_at(timeline: &RecordingClock, at_100ns: u64) -> u64 {
    timeline.elapsed_ms(at_100ns) * SAMPLE_RATE as u64 / 1000
}

#[test]
fn source_drifting_plus_100_ppm_stays_within_one_buffer_of_timeline() {
    let clock = FakeClock::new();
    let mut sources = FakeSources::new();
    let timeline = RecordingClock::new(clock.now_100ns());
    // Ten minutes at +100 ppm drifts 60 ms, several buffers' worth, so the bound is exercised.
    let buffers = 600 * SAMPLE_RATE as u64 / BUFFER_FRAMES;
    for index in 0..buffers {
        let at = START + index * BUFFER_TICKS;
        clock.set(at);
        let exact = sources.send(Source::Microphone, &clock, at, BUFFER_FRAMES, 0.25, false);
        assert_eq!(exact.len(), BUFFER_FRAMES as usize * 2);
        assert_eq!(
            sources.microphone.frames_written(),
            timeline_frames_at(&timeline, at) + BUFFER_FRAMES
        );

        // A faster device delivers its buffers slightly more often, timed on the host clock.
        let drifting_at = START + (index * BUFFER_TICKS * 10_000) / 10_001;
        clock.set(drifting_at);
        let drifting = sources.send(
            Source::SystemAudio,
            &clock,
            drifting_at,
            BUFFER_FRAMES,
            0.5,
            false,
        );
        assert!(drifting.len() <= BUFFER_FRAMES as usize * 2);
        let expected = timeline_frames_at(&timeline, drifting_at) + BUFFER_FRAMES;
        assert!(
            sources.system_audio.frames_written().abs_diff(expected) <= BUFFER_FRAMES,
            "buffer {index}: source {}, timeline {expected}",
            sources.system_audio.frames_written()
        );
    }
    clock.set(START + buffers * BUFFER_TICKS);
    assert_eq!(clock.now_100ns() - START, 600 * HNS_PER_SECOND);
    assert_eq!(
        sources.microphone.frames_written(),
        timeline_frames_at(&timeline, clock.now_100ns())
    );
    // Without the overlap drop, the faster Source would be 60 ms (2,880 frames) ahead.
    let undropped = buffers * BUFFER_FRAMES;
    assert!(undropped - sources.system_audio.frames_written() >= 2 * BUFFER_FRAMES);
}

#[test]
fn an_unflagged_timestamp_gap_beyond_tolerance_is_padded_with_silence() {
    let clock = FakeClock::new();
    let mut sources = FakeSources::new();
    let source = Source::SystemAudio;
    assert_eq!(
        sources.send(source, &clock, START, BUFFER_FRAMES, 0.5, false),
        vec![0.5; 960]
    );
    // The next buffer arrives 30 ms after the last one ended, with no discontinuity flag.
    let at = START + 4 * BUFFER_TICKS;
    clock.set(at);
    let placed = sources.send(source, &clock, at, BUFFER_FRAMES, 0.5, false);
    assert_eq!(&placed[..3 * 960], &vec![0.0; 3 * 960]);
    assert_eq!(&placed[3 * 960..], &vec![0.5; 960]);
    assert_eq!(sources.system_audio.frames_written(), 5 * BUFFER_FRAMES);
}

#[cfg(not(windows))]
#[test]
fn unsupported_platform_capture_reports_recording_unavailable() {
    use meeting_minutes_core::capture::{platform_capture, InputOpener};

    let (opener, clock) = platform_capture().unwrap();
    let first_tick = clock.now_100ns();
    assert!(clock.now_100ns() >= first_tick);
    for source in [Source::Microphone, Source::SystemAudio] {
        assert_eq!(
            opener.open(source).err().as_deref(),
            Some("Recording isn't available on this platform yet")
        );
        assert_eq!(
            opener.default_device_id(source).err().as_deref(),
            Some("Recording isn't available on this platform yet")
        );
    }
}
