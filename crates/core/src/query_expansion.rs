use std::{collections::HashSet, sync::atomic::AtomicBool};

use serde_json::{json, Value};

use crate::{
    ai::{generate_json, GenerateRequest, Message, OutputSchema, Role, TextProvider},
    ask,
    transcript::Language,
};

const MAX_TERMS: usize = 24;

pub fn question_terms(question: &str) -> Vec<String> {
    let mut terms: Vec<_> = ask::terms(question).into_iter().collect();
    terms.sort();
    terms
}

pub fn build_expansion(question: &str, languages: &[Language]) -> GenerateRequest {
    let languages = languages
        .iter()
        .map(|language| language.name())
        .collect::<Vec<_>>()
        .join(", ");
    GenerateRequest {
        system: "Rewrite the question as search keywords and synonyms likely spoken in a Meeting. Include short phrases of 1 to 3 words and translations into each listed Meeting language. Return only a JSON object with a terms array of strings. Do not answer the question.".into(),
        messages: vec![Message {
            role: Role::User,
            content: format!("Question: {question}\nMeeting languages: {languages}"),
        }],
        schema: Some(OutputSchema {
            name: "query_expansion".into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "terms": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["terms"],
                "additionalProperties": false
            }),
        }),
        max_tokens: 256,
        inline_audio: None,
    }
}

pub fn parse_expansion(raw: &Value) -> Option<Vec<String>> {
    let items = raw.get("terms")?.as_array()?;
    let mut seen = HashSet::new();
    let terms: Vec<_> = items
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|term| term.chars().count() >= 2)
        .filter(|term| seen.insert(term.to_lowercase()))
        .take(MAX_TERMS)
        .map(str::to_owned)
        .collect();
    (!terms.is_empty()).then_some(terms)
}

pub fn expand_question(
    provider: &dyn TextProvider,
    question: &str,
    languages: &[Language],
    cancel: &AtomicBool,
) -> Vec<String> {
    let fallback = question_terms(question);
    let request = build_expansion(question, languages);
    let expanded = generate_json::<Value>(provider, &request, &mut |_| {}, cancel)
        .ok()
        .and_then(|raw| parse_expansion(&raw));
    let Some(mut terms) = expanded else {
        return fallback;
    };
    let mut seen: HashSet<_> = terms.iter().map(|term| term.to_lowercase()).collect();
    terms.extend(
        fallback
            .into_iter()
            .filter(|term| seen.insert(term.to_lowercase())),
    );
    terms
}
