use meeting_minutes_core::transcript::Segment;

#[derive(Debug, PartialEq)]
pub struct SegmentStats {
    pub median_s: f64,
    pub p10_s: f64,
    pub p90_s: f64,
    pub short_share: f64,
    pub long_share: f64,
}

fn words(text: &str) -> Vec<String> {
    let normalised = text
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>();
    normalised.split_whitespace().map(str::to_owned).collect()
}

pub fn word_errors(reference: &str, hypothesis: &str) -> (usize, usize) {
    let reference = words(reference);
    let hypothesis = words(hypothesis);
    let mut previous = (0..=hypothesis.len()).collect::<Vec<_>>();
    let mut current = vec![0; hypothesis.len() + 1];
    for (index, word) in reference.iter().enumerate() {
        current[0] = index + 1;
        for (column, candidate) in hypothesis.iter().enumerate() {
            current[column + 1] = (previous[column + 1] + 1)
                .min(current[column] + 1)
                .min(previous[column] + usize::from(word != candidate));
        }
        std::mem::swap(&mut current, &mut previous);
    }
    (previous[hypothesis.len()], reference.len())
}

pub fn wer(reference: &str, hypothesis: &str) -> f64 {
    let (errors, words) = word_errors(reference, hypothesis);
    if words == 0 {
        return f64::from(errors > 0);
    }
    errors as f64 / words as f64
}

pub fn segment_stats(segments: &[Segment]) -> Option<SegmentStats> {
    if segments.is_empty() {
        return None;
    }
    let mut durations = segments
        .iter()
        .map(|segment| segment.end_ms.saturating_sub(segment.start_ms) as f64 / 1000.0)
        .collect::<Vec<_>>();
    durations.sort_by(f64::total_cmp);
    let percentile = |p: f64| {
        let position = p * (durations.len() - 1) as f64;
        let lower = position.floor() as usize;
        let upper = position.ceil() as usize;
        durations[lower] + (durations[upper] - durations[lower]) * position.fract()
    };
    Some(SegmentStats {
        median_s: percentile(0.5),
        p10_s: percentile(0.1),
        p90_s: percentile(0.9),
        short_share: durations.iter().filter(|&&duration| duration < 1.0).count() as f64
            / durations.len() as f64,
        long_share: durations
            .iter()
            .filter(|&&duration| duration > 20.0)
            .count() as f64
            / durations.len() as f64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use meeting_minutes_core::transcript::Speaker;

    #[test]
    fn wer_ignores_case_punctuation_and_spacing() {
        assert_eq!(wer("Hello,   WORLD!", "hello world"), 0.0);
    }

    #[test]
    fn wer_counts_substitutions_insertions_and_deletions() {
        assert_eq!(wer("one two three", "one four three extra"), 2.0 / 3.0);
        assert_eq!(wer("one two three", "one three"), 1.0 / 3.0);
    }

    #[test]
    fn word_errors_can_be_summed_across_clips() {
        assert_eq!(word_errors("one two", "one three"), (1, 2));
        assert_eq!(word_errors("four five", "four five extra"), (1, 2));
    }

    #[test]
    fn wer_preserves_vietnamese_diacritics() {
        assert_eq!(wer("Tôi ở Hà Nội.", "tôi ở hà nội"), 0.0);
        assert_eq!(wer("Tôi ở Hà Nội.", "toi o ha noi"), 1.0);
    }

    #[test]
    fn segment_duration_statistics_include_tails() {
        let durations = [500, 1_000, 3_000, 10_000, 21_000];
        let segments = durations
            .into_iter()
            .map(|end_ms| Segment {
                id: String::new(),
                start_ms: 0,
                end_ms,
                text: String::new(),
                speaker: Speaker::Unknown,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            segment_stats(&segments),
            Some(SegmentStats {
                median_s: 3.0,
                p10_s: 0.7,
                p90_s: 16.6,
                short_share: 0.2,
                long_share: 0.2,
            })
        );
        assert_eq!(segment_stats(&[]), None);
    }
}
