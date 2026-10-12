use chrono::{DateTime, Local};
use meeting_minutes_core::Library;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if !(4..=5).contains(&args.len()) {
        return Err(
            "Usage: create_meeting <library-root> <index-path> <title> [<rfc3339 started_at>]"
                .into(),
        );
    }
    let started_at = match args.get(4) {
        Some(value) => DateTime::parse_from_rfc3339(value)?,
        None => Local::now().fixed_offset(),
    };
    let mut library = Library::open(&args[1], &args[2])?;
    let meeting = library.create_meeting(&args[3], started_at)?;
    println!("{}", meeting.id);
    Ok(())
}
