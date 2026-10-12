use serde::Serialize;

#[derive(Serialize)]
pub struct MeetingSummary {
    pub id: String,
    pub title: String,
    pub started_at: String,
}

pub struct Library;

impl Library {
    pub fn list_meetings(&self) -> Vec<MeetingSummary> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Library;

    #[test]
    fn new_library_has_no_meetings() {
        assert!(Library.list_meetings().is_empty());
    }
}
