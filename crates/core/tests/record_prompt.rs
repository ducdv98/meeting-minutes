use meeting_minutes_core::{
    microphone_usage::MicrophoneApp,
    record_prompt::{PromptDecision, PromptSettings, RecordPromptDecider},
};

fn app(id: &str) -> MicrophoneApp {
    MicrophoneApp {
        id: id.into(),
        display_name: id.into(),
    }
}

fn check(decider: &mut RecordPromptDecider, apps: &[MicrophoneApp]) -> PromptDecision {
    decider.observe(apps, false, PromptSettings::default())
}

#[test]
fn a_new_app_prompts_once_and_prompts_again_after_it_stops() {
    let mut decider = RecordPromptDecider::default();
    assert_eq!(check(&mut decider, &[]), PromptDecision::None);
    assert_eq!(
        check(&mut decider, &[app("Teams")]),
        PromptDecision::Show(app("Teams"))
    );
    assert_eq!(check(&mut decider, &[app("Teams")]), PromptDecision::None);
    decider.dismiss();
    assert_eq!(check(&mut decider, &[app("Teams")]), PromptDecision::None);
    assert_eq!(check(&mut decider, &[]), PromptDecision::None);
    assert_eq!(
        check(&mut decider, &[app("Teams")]),
        PromptDecision::Show(app("Teams"))
    );
}

#[test]
fn a_second_app_prompts_only_when_it_starts() {
    let mut decider = RecordPromptDecider::default();
    check(&mut decider, &[app("Teams")]);
    assert_eq!(
        check(&mut decider, &[app("Teams"), app("Zoom")]),
        PromptDecision::Show(app("Zoom"))
    );
    decider.dismiss();
    assert_eq!(
        check(&mut decider, &[app("Teams"), app("Zoom")]),
        PromptDecision::None
    );
    assert_eq!(check(&mut decider, &[app("Zoom")]), PromptDecision::None);
}

#[test]
fn an_app_suppressed_by_an_open_prompt_prompts_after_that_prompt_closes() {
    let mut decider = RecordPromptDecider::default();
    check(&mut decider, &[]);
    assert_eq!(
        check(&mut decider, &[app("Teams")]),
        PromptDecision::Show(app("Teams"))
    );
    assert_eq!(
        check(&mut decider, &[app("Teams"), app("Zoom")]),
        PromptDecision::None
    );
    decider.dismiss();
    assert_eq!(
        check(&mut decider, &[app("Teams"), app("Zoom")]),
        PromptDecision::Show(app("Zoom"))
    );
    decider.dismiss();
    assert_eq!(
        check(&mut decider, &[app("Teams"), app("Zoom")]),
        PromptDecision::None
    );
}

#[test]
fn apps_present_on_the_first_check_are_not_new() {
    let mut decider = RecordPromptDecider::default();
    assert_eq!(check(&mut decider, &[app("Teams")]), PromptDecision::None);
    assert_eq!(check(&mut decider, &[app("Teams")]), PromptDecision::None);
    assert_eq!(
        check(&mut decider, &[app("Teams"), app("Zoom")]),
        PromptDecision::Show(app("Zoom"))
    );
}

#[test]
fn recording_suppresses_a_new_app_for_its_entire_microphone_session() {
    let mut decider = RecordPromptDecider::default();
    check(&mut decider, &[]);
    assert_eq!(
        decider.observe(&[app("Teams")], true, PromptSettings::default()),
        PromptDecision::None
    );
    assert_eq!(check(&mut decider, &[app("Teams")]), PromptDecision::None);
    assert_eq!(check(&mut decider, &[]), PromptDecision::None);
    assert_eq!(
        check(&mut decider, &[app("Teams")]),
        PromptDecision::Show(app("Teams"))
    );
}

#[test]
fn an_open_prompt_closes_when_its_app_stops_or_recording_starts() {
    let mut decider = RecordPromptDecider::default();
    check(&mut decider, &[]);
    check(&mut decider, &[app("Teams")]);
    assert_eq!(check(&mut decider, &[]), PromptDecision::Close);
    check(&mut decider, &[app("Teams")]);
    assert_eq!(
        decider.observe(&[app("Teams")], true, PromptSettings::default()),
        PromptDecision::Close
    );
    assert_eq!(check(&mut decider, &[app("Teams")]), PromptDecision::None);
}

#[test]
fn silenced_apps_and_the_off_switch_do_not_prompt() {
    let mut decider = RecordPromptDecider::default();
    check(&mut decider, &[]);
    let silenced = vec!["Teams".to_owned()];
    assert_eq!(
        decider.observe(
            &[app("Teams")],
            false,
            PromptSettings {
                enabled: true,
                silenced_app_ids: &silenced
            }
        ),
        PromptDecision::None
    );
    assert_eq!(
        decider.observe(
            &[app("Teams"), app("Zoom")],
            false,
            PromptSettings {
                enabled: false,
                silenced_app_ids: &[]
            }
        ),
        PromptDecision::None
    );
    assert_eq!(
        check(&mut decider, &[app("Teams"), app("Zoom")]),
        PromptDecision::None
    );
}

#[test]
fn removing_a_silenced_app_prompts_on_its_next_microphone_session() {
    let mut decider = RecordPromptDecider::default();
    check(&mut decider, &[]);
    let silenced = vec!["Teams".to_owned()];
    assert_eq!(
        decider.observe(
            &[app("Teams")],
            false,
            PromptSettings {
                enabled: true,
                silenced_app_ids: &silenced
            },
        ),
        PromptDecision::None
    );
    assert_eq!(check(&mut decider, &[app("Teams")]), PromptDecision::None);
    assert_eq!(check(&mut decider, &[]), PromptDecision::None);
    assert_eq!(
        check(&mut decider, &[app("Teams")]),
        PromptDecision::Show(app("Teams"))
    );
}
