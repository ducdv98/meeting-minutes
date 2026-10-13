use chrono::{DateTime, FixedOffset};

pub fn folder_name(started_at: DateTime<FixedOffset>) -> String {
    started_at.format("%Y-%m-%d %H%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::folder_name;
    use chrono::DateTime;

    #[test]
    fn names_meeting_with_its_own_wall_clock_time() {
        let started_at = DateTime::parse_from_rfc3339("2026-10-12T09:30:00+07:00").unwrap();
        assert_eq!(folder_name(started_at), "2026-10-12 0930");
    }

    #[test]
    fn seconds_do_not_affect_folder_name() {
        let at = DateTime::parse_from_rfc3339("2026-10-12T09:30:59Z").unwrap();
        assert_eq!(folder_name(at), "2026-10-12 0930");
    }
}
