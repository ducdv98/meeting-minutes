use std::{
    collections::{HashMap, HashSet},
    fmt, fs,
    io::{self, Write},
    path::Path,
    sync::atomic::AtomicBool,
};

use serde::{Deserialize, Serialize, Serializer};
use serde_json::json;

use crate::{
    ai::{
        context_tokens, generate_json, AiError, GenerateRequest, Message, OutputSchema,
        ProviderKind, Role, TextProvider,
    },
    citation::{check_citations, estimate_tokens, evidence_line, evidence_text, Citation},
    participant::Participants,
    transcript::{Language, Segment, Speaker, Transcript},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerState {
    Pending,
    Answered,
    InsufficientEvidence,
    OffTopic,
    Incomplete,
    Unverified,
    Failed,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskEntry {
    pub id: String,
    pub question: String,
    pub asked_at: String,
    pub revision: u32,
    #[serde(default)]
    pub provider: Option<ProviderKind>,
    #[serde(default)]
    pub model: Option<String>,
    pub state: AnswerState,
    pub answer: String,
    pub citations: Vec<Citation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invalid_citations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskHistory {
    pub schema_version: u32,
    pub asks: Vec<AskEntry>,
}
impl Default for AskHistory {
    fn default() -> Self {
        Self {
            schema_version: 1,
            asks: Vec::new(),
        }
    }
}
pub fn read_asks(meeting_dir: &Path) -> io::Result<AskHistory> {
    match fs::read(meeting_dir.join("asks.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(AskHistory::default()),
        Err(e) => Err(e),
    }
}
pub fn write_asks(meeting_dir: &Path, history: &AskHistory) -> io::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(history)?;
    bytes.push(b'\n');
    let tmp = meeting_dir.join("asks.json.tmp");
    let mut file = fs::File::create(&tmp)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(tmp, meeting_dir.join("asks.json"))
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[test]
    fn ask_uses_the_meeting_language_even_for_an_english_question() {
        assert!(system(crate::transcript::Language::Es, "Ana").contains("Answer in Spanish"));
    }

    #[test]
    fn reads_asks_without_provider_or_model() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("asks.json"),
            r#"{"schema_version":1,"asks":[{"id":"a0001","question":"What?","asked_at":"now","revision":1,"state":"answered","answer":"Yes","citations":[]}]}"#,
        )
        .unwrap();
        let history = read_asks(dir.path()).unwrap();
        assert_eq!(history.asks[0].provider, None);
        assert_eq!(history.asks[0].model, None);
    }

    #[test]
    fn records_selected_provider_and_model_for_an_answer() {
        let dir = tempfile::tempdir().unwrap();
        let mut history = AskHistory::default();
        history
            .begin("What changed?", 2, ProviderKind::Openai, "gpt-4o", "now")
            .unwrap();
        write_asks(dir.path(), &history).unwrap();
        let answer = &read_asks(dir.path()).unwrap().asks[0];
        assert_eq!(answer.provider, Some(ProviderKind::Openai));
        assert_eq!(answer.model.as_deref(), Some("gpt-4o"));
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum AskError {
    EmptyQuestion,
    QuestionTooLong,
    Busy,
    NothingToRetry,
}
impl fmt::Display for AskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyQuestion => "Type a question about this Meeting",
            Self::QuestionTooLong => "Keep the question under 2000 characters",
            Self::Busy => "An Ask is already running for this Meeting",
            Self::NothingToRetry => "Only the latest failed question can be retried",
        })
    }
}
impl std::error::Error for AskError {}
impl Serialize for AskError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AskOutcome {
    pub state: AnswerState,
    pub answer: String,
    pub citations: Vec<Citation>,
    pub invalid_citations: Vec<String>,
    pub error: Option<String>,
}
impl AskHistory {
    pub fn begin(
        &mut self,
        question: &str,
        revision: u32,
        provider: ProviderKind,
        model: &str,
        at: &str,
    ) -> Result<String, AskError> {
        if self
            .asks
            .iter()
            .any(|entry| entry.state == AnswerState::Pending)
        {
            return Err(AskError::Busy);
        }
        let question = question.trim();
        if question.is_empty() {
            return Err(AskError::EmptyQuestion);
        }
        if question.chars().count() > 2000 {
            return Err(AskError::QuestionTooLong);
        }
        let next = self
            .asks
            .iter()
            .filter_map(|entry| entry.id.strip_prefix('a')?.parse::<u32>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        let id = format!("a{next:04}");
        self.asks.push(AskEntry {
            id: id.clone(),
            question: question.into(),
            asked_at: at.into(),
            revision,
            provider: Some(provider),
            model: Some(model.into()),
            state: AnswerState::Pending,
            answer: String::new(),
            citations: Vec::new(),
            invalid_citations: Vec::new(),
            error: None,
        });
        Ok(id)
    }
    pub fn retryable(&self) -> Option<&AskEntry> {
        self.asks.last().filter(|entry| {
            entry.state == AnswerState::Failed
                || (entry.state == AnswerState::Incomplete && entry.error.is_some())
        })
    }
    pub fn begin_retry(
        &mut self,
        revision: u32,
        provider: ProviderKind,
        model: &str,
        at: &str,
    ) -> Result<String, AskError> {
        if self
            .asks
            .iter()
            .any(|entry| entry.state == AnswerState::Pending)
        {
            return Err(AskError::Busy);
        }
        let entry = self
            .asks
            .last_mut()
            .filter(|entry| {
                entry.state == AnswerState::Failed
                    || (entry.state == AnswerState::Incomplete && entry.error.is_some())
            })
            .ok_or(AskError::NothingToRetry)?;
        entry.asked_at = at.into();
        entry.revision = revision;
        entry.provider = Some(provider);
        entry.model = Some(model.into());
        entry.state = AnswerState::Pending;
        entry.answer.clear();
        entry.citations.clear();
        entry.invalid_citations.clear();
        entry.error = None;
        Ok(entry.id.clone())
    }
    pub fn finish(&mut self, id: &str, outcome: AskOutcome) {
        if let Some(entry) = self.asks.iter_mut().find(|entry| entry.id == id) {
            entry.state = outcome.state;
            entry.answer = outcome.answer;
            entry.citations = outcome.citations;
            entry.invalid_citations = outcome.invalid_citations;
            entry.error = outcome.error;
        }
    }
    pub fn interrupted(&mut self) -> bool {
        let mut changed = false;
        for entry in &mut self.asks {
            if entry.state == AnswerState::Pending {
                entry.state = if entry.answer.is_empty() {
                    AnswerState::Failed
                } else {
                    AnswerState::Incomplete
                };
                entry.error = Some("The app closed before the Answer finished".into());
                changed = true;
            }
        }
        changed
    }
}

pub struct AskOptions {
    pub input_tokens: usize,
    pub max_tokens: u32,
    pub history_turns: usize,
}
impl AskOptions {
    pub fn for_provider(kind: ProviderKind) -> Self {
        Self {
            input_tokens: context_tokens(kind).saturating_sub(2048 + 512).max(1024),
            max_tokens: 2048,
            history_turns: 4,
        }
    }
}
pub struct AskRequest {
    pub request: GenerateRequest,
    pub allowed: HashSet<String>,
}
fn system(language: Language, user_name: &str) -> String {
    format!("Answer questions about one meeting only from the transcript segments given as [segment-id] Participant: text. {user_name} is the user. Put supporting segment ids in citations exactly as written; never write times or ids in the answer text. If the segments do not contain the answer, set insufficient_evidence true, citations [], and say briefly that the transcript does not cover it. If the input is not a question or request about this meeting (a greeting, general knowledge, coding, etc.), set off_topic true, citations [], and politely suggest asking something about the meeting. Answer in {} regardless of the question's language. Return only the JSON object.", language.name())
}
fn schema() -> OutputSchema {
    OutputSchema {
        name: "meeting_answer".into(),
        schema: json!({
            "type":"object", "properties":{
                "answer":{"type":"string"}, "citations":{"type":"array","items":{"type":"string"}},
                "insufficient_evidence":{"type":"boolean"}, "off_topic":{"type":"boolean"}
            }, "required":["answer","citations","insufficient_evidence","off_topic"], "additionalProperties":false
        }),
    }
}
pub fn build_ask(
    transcript: &Transcript,
    participants: &Participants,
    history: &AskHistory,
    question: &str,
    options: &AskOptions,
) -> AskRequest {
    let system = system(transcript.language, &participants.label_name(Speaker::Me));
    let eligible: Vec<_> = history
        .asks
        .iter()
        .filter(|entry| {
            entry.revision == transcript.revision
                && matches!(
                    entry.state,
                    AnswerState::Answered | AnswerState::InsufficientEvidence
                )
        })
        .rev()
        .take(options.history_turns)
        .collect();
    let mut pairs: Vec<_> = eligible
        .into_iter()
        .rev()
        .map(|entry| {
            [
                Message {
                    role: Role::User,
                    content: entry.question.clone(),
                },
                Message {
                    role: Role::Assistant,
                    content: entry.answer.clone(),
                },
            ]
        })
        .collect();
    let history_cost = |pairs: &[[Message; 2]]| {
        pairs
            .iter()
            .flatten()
            .map(|message| estimate_tokens(&message.content))
            .sum::<usize>()
    };
    while !pairs.is_empty() && history_cost(&pairs) > options.input_tokens / 4 {
        pairs.remove(0);
    }
    let budget = options.input_tokens.saturating_sub(
        estimate_tokens(&system)
            + history_cost(&pairs)
            + estimate_tokens(question)
            + estimate_tokens("Transcript segments:\n\n\nQuestion: "),
    );
    let all = evidence_text(&transcript.segments, participants, transcript.revision);
    let segments: Vec<&Segment> = if estimate_tokens(&all) <= budget {
        transcript.segments.iter().collect()
    } else {
        retrieve(transcript, participants, question, budget)
    };
    let allowed = segments.iter().map(|segment| segment.id.clone()).collect();
    let mut messages: Vec<_> = pairs.into_iter().flatten().collect();
    messages.push(Message {
        role: Role::User,
        content: format!(
            "Transcript segments:\n{}\n\nQuestion: {question}",
            evidence_text(segments, participants, transcript.revision)
        ),
    });
    AskRequest {
        request: GenerateRequest {
            system,
            messages,
            schema: Some(schema()),
            max_tokens: options.max_tokens,
            inline_audio: None,
        },
        allowed,
    }
}
fn words(input: &str) -> HashSet<String> {
    let folded = crate::search_folding::fold(input);
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|term| term.chars().count() >= 2)
        .map(str::to_owned)
        .collect()
}
pub(crate) fn terms(question: &str) -> HashSet<String> {
    let stops: HashSet<_> = "the and what was did who when is are of to in a an about we they it this that và là của có không gì ai khi nào được cho với này đó những các một".split_whitespace().flat_map(words).collect();
    words(question).difference(&stops).cloned().collect()
}
pub fn retrieve<'a>(
    transcript: &'a Transcript,
    participants: &Participants,
    question: &str,
    budget_tokens: usize,
) -> Vec<&'a Segment> {
    let segments = &transcript.segments;
    if segments.is_empty() {
        return Vec::new();
    }
    let line_chars: Vec<_> = segments
        .iter()
        .map(|segment| {
            evidence_line(segment, participants, transcript.revision)
                .chars()
                .count()
        })
        .collect();
    let query = terms(question);
    let segment_words: Vec<_> = segments
        .iter()
        .map(|segment| {
            words(&format!(
                "{} {}",
                participants.name(segment, transcript.revision),
                segment.text
            ))
        })
        .collect();
    let mut df = HashMap::new();
    for words in &segment_words {
        for term in words.intersection(&query) {
            *df.entry(term.clone()).or_insert(0usize) += 1;
        }
    }
    let mut ranks: Vec<_> = segment_words
        .iter()
        .enumerate()
        .map(|(i, words)| {
            let score: f64 = words
                .intersection(&query)
                .map(|term| (1.0 + segments.len() as f64 / *df.get(term).unwrap() as f64).ln())
                .sum();
            (i, score)
        })
        .filter(|(_, score)| *score > 0.0)
        .collect();
    ranks.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut selected = HashSet::new();
    if ranks.is_empty() {
        for stride in 1..=segments.len() {
            let candidate: HashSet<_> = (0..segments.len()).step_by(stride).collect();
            let chars: usize = candidate
                .iter()
                .map(|index| line_chars[*index])
                .sum::<usize>()
                + candidate.len().saturating_sub(1);
            if chars.div_ceil(3) <= budget_tokens {
                selected = candidate;
                break;
            }
        }
    } else {
        let mut selected_chars = 0;
        for (index, _) in ranks {
            for next in [
                Some(index),
                index.checked_sub(1),
                (index + 1 < segments.len()).then_some(index + 1),
            ]
            .into_iter()
            .flatten()
            {
                if !selected.contains(&next) {
                    let trial_chars =
                        selected_chars + line_chars[next] + usize::from(!selected.is_empty());
                    if trial_chars.div_ceil(3) <= budget_tokens {
                        selected.insert(next);
                        selected_chars = trial_chars;
                    }
                }
            }
        }
    }
    let mut result: Vec<_> = selected.into_iter().map(|index| &segments[index]).collect();
    result.sort_by_key(|segment| segment.start_ms);
    result
}

#[derive(Deserialize)]
struct RawAnswer {
    answer: String,
    citations: Vec<String>,
    insufficient_evidence: bool,
    off_topic: bool,
}
pub const INSUFFICIENT_EVIDENCE_TEXT: &str = "The transcript of this Meeting does not cover this.";
pub fn run_ask(
    provider: &dyn TextProvider,
    ask: &AskRequest,
    transcript: &Transcript,
    on_text: &mut dyn FnMut(&str),
    cancel: &AtomicBool,
) -> AskOutcome {
    let mut received = String::new();
    let mut shown = String::new();
    let result: Result<RawAnswer, AiError> = generate_json(
        provider,
        &ask.request,
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
            let (state, citations, invalid_citations) = if raw.off_topic {
                (AnswerState::OffTopic, Vec::new(), Vec::new())
            } else if raw.insufficient_evidence {
                (AnswerState::InsufficientEvidence, Vec::new(), Vec::new())
            } else {
                let checked = check_citations(transcript, Some(&ask.allowed), &raw.citations);
                let state = if checked.invalid.is_empty() && !checked.valid.is_empty() {
                    AnswerState::Answered
                } else {
                    AnswerState::Unverified
                };
                (state, checked.valid, checked.invalid)
            };
            AskOutcome {
                state,
                answer: if state == AnswerState::InsufficientEvidence {
                    INSUFFICIENT_EVIDENCE_TEXT.to_owned()
                } else {
                    raw.answer
                },
                citations,
                invalid_citations,
                error: None,
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
            AskOutcome {
                state,
                answer,
                citations: Vec::new(),
                invalid_citations: Vec::new(),
                error,
            }
        }
    }
}
/// Extracts the growing top-level answer string from a JSON prefix.
pub fn partial_answer(output_prefix: &str) -> String {
    let input = output_prefix
        .trim_start()
        .strip_prefix("```json")
        .or_else(|| output_prefix.trim_start().strip_prefix("```"))
        .unwrap_or(output_prefix)
        .trim_start();
    let bytes = input.as_bytes();
    let mut i = 0;
    let mut depth: usize = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            b'"' => {
                let start = i;
                i += 1;
                let mut escape = false;
                while i < bytes.len() {
                    if !escape && bytes[i] == b'"' {
                        break;
                    }
                    escape = !escape && bytes[i] == b'\\';
                    i += 1;
                }
                if i == bytes.len() {
                    break;
                }
                let key = &input[start + 1..i];
                i += 1;
                if depth != 1 || key != "answer" {
                    continue;
                }
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i >= bytes.len() || bytes[i] != b':' {
                    continue;
                }
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i >= bytes.len() || bytes[i] != b'"' {
                    return String::new();
                }
                return decode_prefix(&input[i + 1..]);
            }
            _ => i += 1,
        }
    }
    String::new()
}
fn decode_prefix(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '"' {
            break;
        }
        if c != '\\' {
            result.push(c);
            continue;
        }
        let Some(escaped) = chars.next() else {
            break;
        };
        match escaped {
            '"' => result.push('"'),
            '\\' => result.push('\\'),
            '/' => result.push('/'),
            'n' => result.push('\n'),
            't' => result.push('\t'),
            'r' => result.push('\r'),
            'b' => result.push('\u{0008}'),
            'f' => result.push('\u{000c}'),
            'u' => {
                let digits: String = chars.by_ref().take(4).collect();
                if digits.len() != 4 {
                    break;
                }
                let Ok(first) = u16::from_str_radix(&digits, 16) else {
                    break;
                };
                if (0xD800..=0xDBFF).contains(&first) {
                    if chars.next() != Some('\\') || chars.next() != Some('u') {
                        break;
                    }
                    let low: String = chars.by_ref().take(4).collect();
                    let Ok(second) = u16::from_str_radix(&low, 16) else {
                        break;
                    };
                    if !(0xDC00..=0xDFFF).contains(&second) {
                        break;
                    }
                    let point =
                        0x10000 + (((first as u32 - 0xD800) << 10) | (second as u32 - 0xDC00));
                    if let Some(ch) = char::from_u32(point) {
                        result.push(ch);
                    }
                } else if let Some(ch) = char::from_u32(first as u32) {
                    result.push(ch);
                }
            }
            _ => break,
        }
    }
    result
}
