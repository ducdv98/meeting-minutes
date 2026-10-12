use chrono::{DateTime, FixedOffset};

pub fn folder_name(started_at: DateTime<FixedOffset>, title: &str) -> String {
    let cleaned: String = title
        .chars()
        .filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c))
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let limited: String = collapsed.chars().take(80).collect();
    let safe = limited.trim_end_matches(['.', ' ']);
    let safe = if safe.is_empty() {
        "Untitled meeting"
    } else {
        safe
    };
    format!("{} {safe}", started_at.format("%Y-%m-%d %H%M"))
}

#[cfg(test)]
mod tests {
    use super::folder_name;
    use chrono::DateTime;

    #[test]
    fn names_meeting_with_its_own_wall_clock_time() {
        let started_at = DateTime::parse_from_rfc3339("2026-10-12T09:30:00+07:00").unwrap();
        assert_eq!(
            folder_name(started_at, "Planning"),
            "2026-10-12 0930 Planning"
        );
    }

    #[test]
    fn sanitises_windows_names_and_uses_fallback_for_empty_titles() {
        let at = DateTime::parse_from_rfc3339("2026-10-12T09:30:00Z").unwrap();
        assert_eq!(
            folder_name(at, "  Plan<>:\"/\\|?*\n next...  "),
            "2026-10-12 0930 Plan next"
        );
        assert_eq!(
            folder_name(at, " \t<>?.  "),
            "2026-10-12 0930 Untitled meeting"
        );
    }

    #[test]
    fn limits_sanitised_title_to_eighty_characters() {
        let at = DateTime::parse_from_rfc3339("2026-10-12T09:30:00Z").unwrap();
        assert_eq!(
            folder_name(at, &"é".repeat(81)),
            format!("2026-10-12 0930 {}", "é".repeat(80))
        );
    }
}
