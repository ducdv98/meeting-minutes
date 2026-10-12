use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CaptureRole {
    Console,
    Communications,
}

impl CaptureRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Console => "console",
            Self::Communications => "communications",
        }
    }
}

#[derive(Debug, Clone, Parser)]
#[command(name = "capture-spike")]
pub struct Args {
    #[arg(long, value_parser = positive_minutes)]
    pub minutes: f64,
    #[arg(long)]
    pub out: PathBuf,
    #[arg(long, value_enum, default_value_t = CaptureRole::Communications)]
    pub role: CaptureRole,
}

fn positive_minutes(value: &str) -> Result<f64, String> {
    let minutes: f64 = value.parse().map_err(|_| "minutes must be a number")?;
    if minutes.is_finite() && minutes > 0.0 && minutes <= 180.0 {
        Ok(minutes)
    } else {
        Err("minutes must be greater than zero and at most 180 (WAV size limit)".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fractional_minutes_and_output() {
        let args =
            Args::try_parse_from(["capture-spike", "--minutes", "0.5", "--out", "take"]).unwrap();
        assert_eq!(args.minutes, 0.5);
        assert_eq!(args.out, PathBuf::from("take"));
    }

    #[test]
    fn rejects_invalid_minutes() {
        for value in ["0", "-1", "NaN", "inf", "word", "180.1", "181"] {
            assert!(
                Args::try_parse_from(["capture-spike", "--minutes", value, "--out", "take"])
                    .is_err()
            );
        }
    }

    #[test]
    fn accepts_maximum_duration() {
        let args =
            Args::try_parse_from(["capture-spike", "--minutes", "180", "--out", "take"]).unwrap();
        assert_eq!(args.minutes, 180.0);
    }

    #[test]
    fn duration_limit_error_explains_wav_limit() {
        let error = Args::try_parse_from(["capture-spike", "--minutes", "181", "--out", "take"])
            .unwrap_err();
        assert!(error.to_string().contains("at most 180 (WAV size limit)"));
    }

    #[test]
    fn defaults_to_communications_and_accepts_console_role() {
        let default =
            Args::try_parse_from(["capture-spike", "--minutes", "1", "--out", "take"]).unwrap();
        assert_eq!(default.role, CaptureRole::Communications);

        let console = Args::try_parse_from([
            "capture-spike",
            "--minutes",
            "1",
            "--out",
            "take",
            "--role",
            "console",
        ])
        .unwrap();
        assert_eq!(console.role, CaptureRole::Console);

        let communications = Args::try_parse_from([
            "capture-spike",
            "--minutes",
            "1",
            "--out",
            "take",
            "--role",
            "communications",
        ])
        .unwrap();
        assert_eq!(communications.role, CaptureRole::Communications);
    }

    #[test]
    fn rejects_unknown_role() {
        assert!(Args::try_parse_from([
            "capture-spike",
            "--minutes",
            "1",
            "--out",
            "take",
            "--role",
            "multimedia",
        ])
        .is_err());
    }
}
