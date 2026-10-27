use std::sync::{atomic::AtomicBool, Mutex};

use meeting_minutes_core::{
    ai::{AiError, GenerateRequest, ProviderKind, Role, TextProvider},
    query_expansion::{build_expansion, expand_question, parse_expansion, question_terms},
    transcript::Language,
};
use serde_json::json;

struct FakeProvider {
    response: Result<String, AiError>,
    requests: Mutex<Vec<GenerateRequest>>,
}

impl FakeProvider {
    fn reply(response: &str) -> Self {
        Self {
            response: Ok(response.into()),
            requests: Mutex::new(Vec::new()),
        }
    }
}

impl TextProvider for FakeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Ollama
    }

    fn model(&self) -> &str {
        "fake"
    }

    fn generate(
        &self,
        request: &GenerateRequest,
        _: &mut dyn FnMut(&str),
        _: &AtomicBool,
    ) -> Result<String, AiError> {
        self.requests.lock().unwrap().push(request.clone());
        match &self.response {
            Ok(response) => Ok(response.clone()),
            Err(_) => Err(AiError::Unreachable("timed out".into())),
        }
    }
}

#[test]
fn question_terms_match_ask_stop_words_in_english_and_vietnamese() {
    assert_eq!(
        question_terms("What did we agree about the launch budget?"),
        ["agree", "budget", "launch"]
    );
    assert_eq!(
        question_terms("Chúng ta đã nói gì về ngân sách và lịch họp?"),
        ["chung", "da", "hop", "lich", "ngan", "noi", "sach", "ta", "ve"]
    );
}

#[test]
fn request_contains_only_the_question_and_meeting_language_names() {
    let request = build_expansion(
        "What was agreed about the budget?",
        &[Language::En, Language::Vi],
    );
    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.messages[0].role, Role::User);
    assert_eq!(
        request.messages[0].content,
        "Question: What was agreed about the budget?\nMeeting languages: English, Vietnamese"
    );
    assert!(request.max_tokens <= 256);
    assert_eq!(
        request.schema.as_ref().unwrap().schema["required"],
        json!(["terms"])
    );
}

#[test]
fn good_json_adds_expanded_terms_to_question_terms() {
    let provider =
        FakeProvider::reply(r#"{"terms":["  pricing  ","cost estimate","ngân sách","BUDGET"]}"#);
    let terms = expand_question(
        &provider,
        "What about budget?",
        &[Language::En, Language::Vi],
        &AtomicBool::new(false),
    );
    assert_eq!(terms, ["pricing", "cost estimate", "ngân sách", "BUDGET"]);
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
}

#[test]
fn bad_or_empty_output_and_provider_error_use_question_terms() {
    let fallback = question_terms("What about budget?");
    for provider in [
        FakeProvider::reply("not json"),
        FakeProvider::reply(r#"{"terms":[]}"#),
        FakeProvider::reply(r#"{"terms":[" ","x"]}"#),
        FakeProvider {
            response: Err(AiError::Unreachable("timed out".into())),
            requests: Mutex::new(Vec::new()),
        },
    ] {
        assert_eq!(
            expand_question(
                &provider,
                "What about budget?",
                &[Language::En],
                &AtomicBool::new(false)
            ),
            fallback
        );
    }
}

#[test]
fn parser_trims_deduplicates_case_insensitively_and_caps_terms() {
    let raw = json!({"terms": [" Budget ", "budget", "X", " ", 3, "COST", "cost"]});
    assert_eq!(
        parse_expansion(&raw),
        Some(vec!["Budget".into(), "COST".into()])
    );
    assert_eq!(parse_expansion(&json!({"terms": ["x", " "]})), None);
    assert_eq!(parse_expansion(&json!({"terms": "budget"})), None);
    let many = json!({"terms": (0..50).map(|i| format!("term{i}")).collect::<Vec<_>>()});
    assert_eq!(parse_expansion(&many).unwrap().len(), 24);
}
