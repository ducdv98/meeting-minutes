use std::{
    collections::HashSet,
    fs,
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    ai::{
        context_tokens, generate_json, AiError, GenerateRequest, Message, OutputSchema,
        ProviderKind, Role, TextProvider,
    },
    citation::{check_citations, estimate_tokens, evidence_line, evidence_text, Citation},
    transcript::{Language, Segment, Transcript},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryItem {
    pub text: String,
    pub citations: Vec<Citation>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionItem {
    pub text: String,
    pub owner: Option<String>,
    pub due: Option<String>,
    pub citations: Vec<Citation>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub schema_version: u32,
    pub revision: u32,
    pub created_at: String,
    pub provider: ProviderKind,
    pub model: String,
    pub overview: Vec<SummaryItem>,
    pub key_points: Vec<SummaryItem>,
    pub decisions: Vec<SummaryItem>,
    pub action_items: Vec<ActionItem>,
    pub dropped_items: u32,
}

pub fn read_summary(meeting_dir: &Path) -> io::Result<Option<Summary>> {
    match fs::read(meeting_dir.join("summary.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_file_name(format!(
        "{}.tmp",
        path.file_name().unwrap().to_string_lossy()
    ));
    let mut file = fs::File::create(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(tmp, path)
}

pub fn write_summary(meeting_dir: &Path, title: &str, summary: &Summary) -> io::Result<()> {
    let mut json = serde_json::to_vec_pretty(summary)?;
    json.push(b'\n');
    write_atomic(&meeting_dir.join("summary.json"), &json)?;
    write_atomic(
        &meeting_dir.join("summary.md"),
        render_summary_markdown(title, summary).as_bytes(),
    )
}

fn time(ms: u64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}
fn cite(citations: &[Citation]) -> String {
    if citations.is_empty() {
        String::new()
    } else {
        format!(
            " [{}]",
            citations
                .iter()
                .map(|c| time(c.start_ms))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
fn section(out: &mut String, title: &str, items: &[SummaryItem]) {
    out.push_str(&format!("\n## {title}\n"));
    if items.is_empty() {
        out.push_str("_None_\n");
    }
    for item in items {
        out.push_str(&format!("- {}{}\n", item.text, cite(&item.citations)));
    }
}
pub fn render_summary_markdown(title: &str, summary: &Summary) -> String {
    let mut out = format!(
        "# {title} — Summary\n\nFrom transcript revision {} · {} {}\n",
        summary.revision,
        summary.provider.label(),
        summary.model
    );
    section(&mut out, "Overview", &summary.overview);
    section(&mut out, "Key points", &summary.key_points);
    section(&mut out, "Decisions", &summary.decisions);
    out.push_str("\n## Action items\n");
    if summary.action_items.is_empty() {
        out.push_str("_None_\n");
    }
    for item in &summary.action_items {
        out.push_str(&format!("- {}", item.text));
        if let Some(owner) = &item.owner {
            out.push_str(&format!(" — owner: {owner}"));
        }
        if let Some(due) = &item.due {
            out.push_str(&format!(
                "{}due: {due}",
                if item.owner.is_some() {
                    " · "
                } else {
                    " — "
                }
            ));
        }
        out.push_str(&format!("{}\n", cite(&item.citations)));
    }
    out
}

#[derive(Clone, Serialize, Deserialize)]
struct RawItem {
    text: String,
    citations: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct RawAction {
    task: String,
    owner: String,
    due: String,
    citations: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct RawSummary {
    overview: Vec<RawItem>,
    key_points: Vec<RawItem>,
    decisions: Vec<RawItem>,
    action_items: Vec<RawAction>,
}
impl RawSummary {
    fn append(&mut self, other: Self) {
        self.overview.extend(other.overview);
        self.key_points.extend(other.key_points);
        self.decisions.extend(other.decisions);
        self.action_items.extend(other.action_items);
    }
    fn ids(&self) -> HashSet<String> {
        self.overview
            .iter()
            .chain(&self.key_points)
            .chain(&self.decisions)
            .flat_map(|item| &item.citations)
            .chain(self.action_items.iter().flat_map(|item| &item.citations))
            .cloned()
            .collect()
    }
}
fn schema() -> OutputSchema {
    let string = json!({"type":"string"});
    let citations = json!({"type":"array","items":{"type":"string"}});
    let item = json!({"type":"object","properties":{"text":string,"citations":citations},"required":["text","citations"],"additionalProperties":false});
    let action = json!({"type":"object","properties":{"task":string,"owner":string,"due":string,"citations":citations},"required":["task","owner","due","citations"],"additionalProperties":false});
    let mut properties = serde_json::Map::new();
    for name in ["overview", "key_points", "decisions"] {
        properties.insert(name.into(), json!({"type":"array","items":item}));
    }
    properties.insert(
        "action_items".into(),
        json!({"type":"array","items":action}),
    );
    OutputSchema {
        name: "meeting_summary".into(),
        schema: Value::Object(serde_json::Map::from_iter([
            ("type".into(), json!("object")),
            ("properties".into(), Value::Object(properties)),
            (
                "required".into(),
                json!(["overview", "key_points", "decisions", "action_items"]),
            ),
            ("additionalProperties".into(), json!(false)),
        ])),
    }
}
fn system(language: Language, reduce: bool) -> String {
    let language = match language {
        Language::En => "English",
        Language::Vi => "Vietnamese",
    };
    let mut s = format!("Summarise one meeting from its transcript in {language}. Transcript lines are [segment-id] Speaker: text; Speaker is Me, Remote or Unknown. overview: 1–3 short sentences about the meeting; key_points: main points discussed; decisions: only decisions actually made; action_items: tasks committed to or requested, owner who is responsible as stated else empty string, due deadline exactly as stated else empty string. Every item must list supporting segment ids in citations exactly as written (e.g. s00012). Never invent ids. Never write times or ids inside text. Leave a section empty when unsupported. Return only the JSON object.");
    if reduce {
        s.push_str(" You are given partial summaries of consecutive parts of one meeting, in order, as JSON. Merge them into one summary of the whole meeting: combine duplicates, keep each merged item's citations (union of the ones it came from), and use only citation ids that appear in the input.");
    }
    s
}
fn map_request(language: Language, segments: &[Segment], max_tokens: u32) -> GenerateRequest {
    GenerateRequest {
        system: system(language, false),
        messages: vec![Message {
            role: Role::User,
            content: format!("Transcript:\n{}", evidence_text(segments)),
        }],
        schema: Some(schema()),
        max_tokens,
    }
}
fn reduce_request(language: Language, parts: &[RawSummary], max_tokens: u32) -> GenerateRequest {
    GenerateRequest {
        system: system(language, true),
        messages: vec![Message {
            role: Role::User,
            content: json!({"parts":parts}).to_string(),
        }],
        schema: Some(schema()),
        max_tokens,
    }
}
pub struct SummaryOptions {
    pub input_tokens: usize,
    pub max_tokens: u32,
}
impl SummaryOptions {
    pub fn for_provider(kind: ProviderKind) -> Self {
        Self {
            input_tokens: context_tokens(kind).saturating_sub(4096 + 512).max(1024),
            max_tokens: 4096,
        }
    }
}
pub fn summary_requests(transcript: &Transcript, options: &SummaryOptions) -> Vec<GenerateRequest> {
    let mut requests = Vec::new();
    let system_tokens = estimate_tokens(&system(transcript.language, false));
    let prefix_chars = "Transcript:\n".chars().count();
    let mut chunk_start = 0;
    let mut evidence_chars = 0;
    for (index, segment) in transcript.segments.iter().enumerate() {
        let line_chars = evidence_line(segment).chars().count();
        let trial_chars = evidence_chars + line_chars + usize::from(index > chunk_start);
        if index > chunk_start
            && system_tokens + (prefix_chars + trial_chars).div_ceil(3) > options.input_tokens
        {
            requests.push(map_request(
                transcript.language,
                &transcript.segments[chunk_start..index],
                options.max_tokens,
            ));
            chunk_start = index;
            evidence_chars = line_chars;
        } else {
            evidence_chars = trial_chars;
        }
    }
    if chunk_start < transcript.segments.len() {
        requests.push(map_request(
            transcript.language,
            &transcript.segments[chunk_start..],
            options.max_tokens,
        ));
    }
    requests
}
fn validate(
    raw: RawSummary,
    transcript: &Transcript,
    allowed: Option<&HashSet<String>>,
    dropped: &mut u32,
) -> RawSummary {
    fn items(
        input: Vec<RawItem>,
        transcript: &Transcript,
        allowed: Option<&HashSet<String>>,
        dropped: &mut u32,
    ) -> Vec<RawItem> {
        input
            .into_iter()
            .filter_map(|mut item| {
                item.text = item.text.trim().to_owned();
                item.citations = check_citations(transcript, allowed, &item.citations)
                    .valid
                    .into_iter()
                    .map(|c| c.segment_id)
                    .collect();
                if item.text.is_empty() || item.citations.is_empty() {
                    *dropped += 1;
                    None
                } else {
                    Some(item)
                }
            })
            .collect()
    }
    let overview = items(raw.overview, transcript, allowed, dropped);
    let key_points = items(raw.key_points, transcript, allowed, dropped);
    let decisions = items(raw.decisions, transcript, allowed, dropped);
    let action_items = raw
        .action_items
        .into_iter()
        .filter_map(|mut item| {
            item.task = item.task.trim().to_owned();
            item.citations = check_citations(transcript, allowed, &item.citations)
                .valid
                .into_iter()
                .map(|c| c.segment_id)
                .collect();
            if item.task.is_empty() || item.citations.is_empty() {
                *dropped += 1;
                None
            } else {
                Some(item)
            }
        })
        .collect();
    RawSummary {
        overview,
        key_points,
        decisions,
        action_items,
    }
}
fn finish(
    raw: RawSummary,
    transcript: &Transcript,
    provider: &dyn TextProvider,
    dropped: u32,
) -> Summary {
    let cite_ids = |ids: &[String]| check_citations(transcript, None, ids).valid;
    let items = |raw: Vec<RawItem>| {
        raw.into_iter()
            .map(|item| SummaryItem {
                text: item.text,
                citations: cite_ids(&item.citations),
            })
            .collect()
    };
    Summary {
        schema_version: 1,
        revision: transcript.revision,
        created_at: Local::now().to_rfc3339(),
        provider: provider.kind(),
        model: provider.model().into(),
        overview: items(raw.overview),
        key_points: items(raw.key_points),
        decisions: items(raw.decisions),
        action_items: raw
            .action_items
            .into_iter()
            .map(|item| ActionItem {
                text: item.task,
                owner: (!item.owner.trim().is_empty()).then(|| item.owner.trim().to_owned()),
                due: (!item.due.trim().is_empty()).then(|| item.due.trim().to_owned()),
                citations: cite_ids(&item.citations),
            })
            .collect(),
        dropped_items: dropped,
    }
}
pub fn summarize(
    provider: &dyn TextProvider,
    transcript: &Transcript,
    options: &SummaryOptions,
    on_step: &mut dyn FnMut(usize, usize),
    cancel: &AtomicBool,
) -> Result<Summary, AiError> {
    if transcript.segments.is_empty() {
        return Err(AiError::InvalidOutput(
            "The Transcript has no Segments".into(),
        ));
    }
    let requests = summary_requests(transcript, options);
    let mut total = requests.len();
    let mut done = 0;
    let mut dropped = 0;
    let mut parts = Vec::new();
    for request in &requests {
        if cancel.load(Ordering::SeqCst) {
            return Err(AiError::Cancelled);
        }
        let raw: RawSummary = generate_json(provider, request, &mut |_| {}, cancel)?;
        let allowed = request.messages[0]
            .content
            .lines()
            .filter_map(|line| {
                line.strip_prefix('[')?
                    .split_once(']')
                    .map(|(id, _)| id.to_owned())
            })
            .collect();
        parts.push(validate(raw, transcript, Some(&allowed), &mut dropped));
        done += 1;
        on_step(done, total);
    }
    while parts.len() > 1 {
        let mut groups: Vec<Vec<RawSummary>> = Vec::new();
        let mut group = Vec::new();
        for part in parts {
            if !group.is_empty() {
                let mut trial = group.clone();
                trial.push(part.clone());
                let req = reduce_request(transcript.language, &trial, options.max_tokens);
                if estimate_tokens(&req.system) + estimate_tokens(&req.messages[0].content)
                    > options.input_tokens
                {
                    groups.push(group);
                    group = Vec::new();
                }
            }
            group.push(part);
        }
        if !group.is_empty() {
            groups.push(group);
        }
        if groups.len() == groups.iter().map(Vec::len).sum::<usize>() {
            let mut combined = groups.into_iter().flatten();
            let mut first = combined.next().unwrap();
            for part in combined {
                first.append(part);
            }
            parts = vec![first];
            break;
        }
        total += groups.len();
        parts = Vec::new();
        for group in groups {
            if cancel.load(Ordering::SeqCst) {
                return Err(AiError::Cancelled);
            }
            let allowed = group.iter().flat_map(RawSummary::ids).collect();
            let req = reduce_request(transcript.language, &group, options.max_tokens);
            let raw: RawSummary = generate_json(provider, &req, &mut |_| {}, cancel)?;
            parts.push(validate(raw, transcript, Some(&allowed), &mut dropped));
            done += 1;
            on_step(done, total);
        }
    }
    let final_raw = validate(parts.pop().unwrap(), transcript, None, &mut dropped);
    Ok(finish(final_raw, transcript, provider, dropped))
}
