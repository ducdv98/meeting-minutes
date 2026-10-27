use std::{
    collections::{HashMap, HashSet},
    fmt, fs,
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use serde::{Deserialize, Serialize, Serializer};
use serde_json::json;

use crate::{
    ai::{
        generate_json, AiError, GenerateRequest, Message, OutputSchema, ProviderKind, Role,
        TextProvider,
    },
    ask::{partial_answer, AnswerState, AskOptions},
    citation::estimate_tokens,
    library_retrieval::{evidence_header, evidence_segment_line, MeetingEvidence},
    query_expansion::expand_question,
    transcript::Language,
    Library, LibraryError,
};

pub const INSUFFICIENT_EVIDENCE_TEXT: &str = "Your Library doesn't cover this.";
pub const INSUFFICIENT_EVIDENCE_RANGE_TEXT: &str =
    "Your Library doesn't cover this in the selected range.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DateRange {
    #[serde(rename = "last_7_days")]
    Last7Days,
    #[serde(rename = "last_30_days")]
    Last30Days,
    #[serde(rename = "last_90_days")]
    Last90Days,
    #[serde(rename = "all")]
    All,
}
impl DateRange {
    pub fn since_ms(self, now_ms: i64) -> Option<i64> {
        let days: i64 = match self {
            Self::Last7Days => 7,
            Self::Last30Days => 30,
            Self::Last90Days => 90,
            Self::All => return None,
        };
        Some(now_ms.saturating_sub(days * 86_400_000))
    }
    fn insufficient_text(self) -> &'static str {
        if self == Self::All {
            INSUFFICIENT_EVIDENCE_TEXT
        } else {
            INSUFFICIENT_EVIDENCE_RANGE_TEXT
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryAskPlan {
    pub question: String,
    pub range: DateRange,
    pub evidence: Vec<MeetingEvidence>,
}

#[derive(Debug)]
pub enum LibraryAskError {
    EmptyQuestion,
    QuestionTooLong,
    Busy,
    Cancelled,
    Library(LibraryError),
}
impl fmt::Display for LibraryAskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyQuestion => f.write_str("Type a question about your Library"),
            Self::QuestionTooLong => f.write_str("Keep the question under 2000 characters"),
            Self::Busy => f.write_str("A Library Ask is already running"),
            Self::Cancelled => f.write_str("Cancelled"),
            Self::Library(error) => write!(f, "{error}"),
        }
    }
}
impl std::error::Error for LibraryAskError {}
impl From<LibraryError> for LibraryAskError {
    fn from(error: LibraryError) -> Self {
        Self::Library(error)
    }
}
impl Serialize for LibraryAskError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
fn validate_question(question: &str) -> Result<&str, LibraryAskError> {
    let question = question.trim();
    if question.is_empty() {
        return Err(LibraryAskError::EmptyQuestion);
    }
    if question.chars().count() > 2000 {
        return Err(LibraryAskError::QuestionTooLong);
    }
    Ok(question)
}

fn system() -> String {
    "Answer questions about the Library only from the Meeting Transcript Segments provided. Each Meeting has a Title and date. Segment references such as [M1-s00001] are unique across the Library; put supporting references in citations exactly as written. Never write times or references in the answer text. If the evidence does not contain the answer, set insufficient_evidence true and citations []. If the input is not a question or request about the Library (a greeting, general knowledge, coding, etc.), set off_topic true, citations [], and politely suggest asking about the Library. Answer in the language of the question, regardless of the Meeting language. Return only the JSON object.".into()
}
fn schema() -> OutputSchema {
    OutputSchema {
        name: "library_answer".into(),
        schema: json!({
            "type": "object", "properties": {
                "answer": {"type":"string"},
                "citations": {"type":"array", "items": {"type":"string"}},
                "insufficient_evidence": {"type":"boolean"},
                "off_topic": {"type":"boolean"}
            },
            "required": ["answer", "citations", "insufficient_evidence", "off_topic"],
            "additionalProperties": false
        }),
    }
}

/// The search terms for a question. Calls the Language model, so callers
/// should not hold the Library while it runs.
pub fn library_ask_terms(
    provider: &dyn TextProvider,
    question: &str,
    languages: &[Language],
    cancel: &AtomicBool,
) -> Result<Vec<String>, LibraryAskError> {
    let question = validate_question(question)?;
    if cancel.load(Ordering::SeqCst) {
        return Err(LibraryAskError::Cancelled);
    }
    let terms = expand_question(provider, question, languages, cancel);
    if cancel.load(Ordering::SeqCst) {
        return Err(LibraryAskError::Cancelled);
    }
    Ok(terms)
}

pub fn gather_library_evidence(
    library: &Library,
    kind: ProviderKind,
    question: &str,
    terms: &[String],
    range: DateRange,
    now_ms: i64,
) -> Result<LibraryAskPlan, LibraryAskError> {
    let question = validate_question(question)?;
    let options = AskOptions::for_provider(kind);
    let budget = options.input_tokens.saturating_sub(
        estimate_tokens(&system())
            + estimate_tokens(question)
            + estimate_tokens("Meeting evidence:\n\nQuestion: ")
            + 512,
    );
    let evidence = library.library_evidence(terms, range.since_ms(now_ms), budget)?;
    Ok(LibraryAskPlan {
        question: question.into(),
        range,
        evidence,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryCitation {
    pub meeting_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub started_at: String,
    pub segment_id: String,
    pub revision: u32,
    pub start_ms: u64,
    pub end_ms: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryAskOutcome {
    pub state: AnswerState,
    pub answer: String,
    pub citations: Vec<LibraryCitation>,
    pub invalid_citations: Vec<String>,
    pub error: Option<String>,
    pub meetings_read: Vec<String>,
}
#[derive(Deserialize)]
struct RawAnswer {
    answer: String,
    citations: Vec<String>,
    insufficient_evidence: bool,
    off_topic: bool,
}
fn normalize_reference(reference: &str) -> &str {
    let reference = reference.trim();
    reference
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(reference)
        .trim()
}

pub fn run_library_ask(
    provider: &dyn TextProvider,
    plan: &LibraryAskPlan,
    excluded_meetings: &HashSet<String>,
    on_text: &mut dyn FnMut(&str),
    cancel: &AtomicBool,
) -> LibraryAskOutcome {
    let evidence: Vec<_> = plan
        .evidence
        .iter()
        .filter(|meeting| {
            !excluded_meetings.contains(&meeting.meeting_id) && !meeting.segments.is_empty()
        })
        .collect();
    let meetings_read = evidence
        .iter()
        .map(|meeting| meeting.meeting_id.clone())
        .collect();
    let empty = || LibraryAskOutcome {
        state: AnswerState::InsufficientEvidence,
        answer: plan.range.insufficient_text().into(),
        citations: Vec::new(),
        invalid_citations: Vec::new(),
        error: None,
        meetings_read: Vec::new(),
    };
    if evidence.is_empty() {
        return empty();
    }

    let mut references = HashMap::new();
    let mut sections = Vec::new();
    for (index, meeting) in evidence.iter().enumerate() {
        let prefix = format!("M{}", index + 1);
        let mut lines = vec![evidence_header(&prefix, meeting)];
        for segment in &meeting.segments {
            let reference = format!("{prefix}-{}", segment.segment_id);
            lines.push(evidence_segment_line(&prefix, segment));
            references.insert(
                reference,
                LibraryCitation {
                    meeting_id: meeting.meeting_id.clone(),
                    title: meeting.title.clone(),
                    started_at: meeting.started_at.clone(),
                    segment_id: segment.segment_id.clone(),
                    revision: segment.revision,
                    start_ms: segment.start_ms,
                    end_ms: segment.end_ms,
                },
            );
        }
        sections.push(lines.join("\n"));
    }
    let request = GenerateRequest {
        system: system(),
        messages: vec![Message {
            role: Role::User,
            content: format!(
                "Meeting evidence:\n{}\n\nQuestion: {}",
                sections.join("\n\n"),
                plan.question
            ),
        }],
        schema: Some(schema()),
        max_tokens: AskOptions::for_provider(provider.kind()).max_tokens,
        inline_audio: None,
    };
    let mut received = String::new();
    let mut shown = String::new();
    let result: Result<RawAnswer, AiError> = generate_json(
        provider,
        &request,
        &mut |delta| {
            received.push_str(delta);
            let answer = partial_answer(&received);
            if answer.len() > shown.len() {
                shown = answer;
                on_text(&shown);
            }
        },
        cancel,
    );
    match result {
        Ok(raw) => {
            if raw.answer.len() > shown.len() {
                on_text(&raw.answer);
            }
            let mut citations = Vec::new();
            let mut invalid_citations = Vec::new();
            let state = if raw.off_topic {
                AnswerState::OffTopic
            } else if raw.insufficient_evidence {
                AnswerState::InsufficientEvidence
            } else {
                let mut seen = HashSet::new();
                for reference in &raw.citations {
                    let normalized = normalize_reference(reference);
                    if normalized.is_empty() || !seen.insert(normalized.to_owned()) {
                        continue;
                    }
                    if let Some(citation) = references.get(normalized) {
                        citations.push(citation.clone());
                    } else {
                        invalid_citations.push(normalized.to_owned());
                    }
                }
                if invalid_citations.is_empty() && !citations.is_empty() {
                    AnswerState::Answered
                } else {
                    AnswerState::Unverified
                }
            };
            LibraryAskOutcome {
                state,
                answer: if state == AnswerState::InsufficientEvidence {
                    plan.range.insufficient_text().into()
                } else {
                    raw.answer
                },
                citations,
                invalid_citations,
                error: None,
                meetings_read,
            }
        }
        Err(error) => {
            let answer = partial_answer(&received);
            let (state, error) = if matches!(error, AiError::Cancelled) {
                (AnswerState::Incomplete, None)
            } else if answer.is_empty() {
                (AnswerState::Failed, Some(error.to_string()))
            } else {
                (AnswerState::Incomplete, Some(error.to_string()))
            };
            LibraryAskOutcome {
                state,
                answer,
                citations: Vec::new(),
                invalid_citations: Vec::new(),
                error,
                meetings_read,
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryAskEntry {
    pub id: String,
    pub question: String,
    pub range: DateRange,
    pub asked_at: String,
    pub provider: ProviderKind,
    pub model: String,
    pub state: AnswerState,
    pub answer: String,
    pub citations: Vec<LibraryCitation>,
    pub invalid_citations: Vec<String>,
    pub error: Option<String>,
    pub meetings_read: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryAskHistory {
    pub schema_version: u32,
    pub asks: Vec<LibraryAskEntry>,
}
impl Default for LibraryAskHistory {
    fn default() -> Self {
        Self {
            schema_version: 1,
            asks: Vec::new(),
        }
    }
}
impl LibraryAskHistory {
    pub fn begin(
        &mut self,
        question: &str,
        range: DateRange,
        provider: ProviderKind,
        model: &str,
        at: &str,
    ) -> Result<String, LibraryAskError> {
        if self
            .asks
            .iter()
            .any(|entry| entry.state == AnswerState::Pending)
        {
            return Err(LibraryAskError::Busy);
        }
        let question = validate_question(question)?;
        let next = self
            .asks
            .iter()
            .filter_map(|entry| entry.id.strip_prefix("la")?.parse::<u32>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        let id = format!("la{next:04}");
        self.asks.push(LibraryAskEntry {
            id: id.clone(),
            question: question.into(),
            range,
            asked_at: at.into(),
            provider,
            model: model.into(),
            state: AnswerState::Pending,
            answer: String::new(),
            citations: Vec::new(),
            invalid_citations: Vec::new(),
            error: None,
            meetings_read: Vec::new(),
        });
        Ok(id)
    }
    pub fn finish(&mut self, id: &str, outcome: LibraryAskOutcome) {
        if let Some(entry) = self.asks.iter_mut().find(|entry| entry.id == id) {
            entry.state = outcome.state;
            entry.answer = outcome.answer;
            entry.citations = outcome.citations;
            entry.invalid_citations = outcome.invalid_citations;
            entry.error = outcome.error;
            entry.meetings_read = outcome.meetings_read;
        }
    }
    pub fn interrupted(&mut self) -> bool {
        let mut changed = false;
        for entry in &mut self.asks {
            if entry.state == AnswerState::Pending {
                entry.state = AnswerState::Incomplete;
                entry.error = Some("The app closed before the Answer finished".into());
                changed = true;
            }
        }
        changed
    }
}
pub fn read_library_asks(library_root: &Path) -> io::Result<LibraryAskHistory> {
    match fs::read(library_root.join("library-asks.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(LibraryAskHistory::default()),
        Err(error) => Err(error),
    }
}
pub fn write_library_asks(library_root: &Path, history: &LibraryAskHistory) -> io::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(history)?;
    bytes.push(b'\n');
    let tmp = library_root.join("library-asks.json.tmp");
    let mut file = fs::File::create(&tmp)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(tmp, library_root.join("library-asks.json"))
}
