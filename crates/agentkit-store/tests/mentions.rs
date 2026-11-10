#![allow(clippy::unwrap_used)]
use agentkit_protocol::generated as g;
use agentkit_store::core::mentions::*;
fn skills() -> Vec<Skill> {
    vec![Skill {
        slug: "review-code".into(),
        name: "Review".into(),
        description: "Review changes".into(),
        plugin: "my workspace".into(),
    }]
}
#[test]
fn upstream_grammar_dedupe_and_utf16_badges() {
    let text = "😀 [skill:my workspace:review-code] [file:src/你好.rs] [folder:src] [skill:review-code] [skill:missing]";
    let m = metadata(text, &skills());
    assert_eq!(m.skill_slugs, Some(vec!["review-code".into()]));
    let b = m.badges.unwrap();
    assert_eq!(b.len(), 4);
    assert_eq!(b[0].start, 3.);
    assert_eq!(b[1].label, "你好.rs");
    assert_eq!(b[3].raw_text, "[skill:my workspace:review-code]");
    assert_eq!(b[3].end - b[3].start, "[skill:review-code]".len() as f64);
    assert!(
        metadata(
            "[skill:bad\nworkspace:review-code] [skill:review.code]",
            &skills()
        )
        .badges
        .is_none()
    );
}
#[test]
fn caret_queries_and_safe_candidates() {
    assert_eq!(query("hello @src/", 11).unwrap().text, "src/");
    assert!(query("a@b.com", 7).is_none());
    assert_eq!(query("你好@", 7).unwrap().kind, Kind::File);
    assert_eq!(query("line\n /rev", 10).unwrap().kind, Kind::Skill);
    assert!(query("text /rev", 9).is_none());
    assert!(query("[file:@src]", 11).is_none());
    assert!(query("😀", 1).is_none());
    assert!(file_candidate("a]b", false).is_none());
    assert_eq!(
        file_candidate("src/a.rs", false).unwrap().raw,
        "[file:src/a.rs]"
    );
    assert_eq!(
        skill_candidates(&skills(), "changes")[0].raw,
        "[skill:my workspace:review-code]"
    );
}
#[test]
fn utf16_spans_reject_partial_surrogates_and_fractional_offsets() {
    assert_eq!(byte_range("😀x", 0., 2.), Some(0..4));
    assert_eq!(byte_range("😀x", 1., 2.), None);
    assert_eq!(byte_range("x", 0.5, 1.), None);
    assert_eq!(byte_range("x", 0., f64::NAN), None);
    assert_eq!(byte_range("x", 2., 3.), None);
    let _ = g::SendMessageOptions::default();
}
#[test]
fn parse_matches_upstream_sources_and_invalid_skills() {
    let text = "[skill:review-code] [skill:unknown] [file:a] [file:a] [folder:src] [skill:.agents:review-code] [skill:unknown] [file:[file:nested]]";
    assert_eq!(
        parse(text, &skills()),
        Parsed {
            skills: vec!["review-code".into()],
            invalid_skills: vec!["unknown".into()],
            files: vec!["a".into(), "[file:nested".into()],
            folders: vec!["src".into()]
        }
    );
    assert_eq!(metadata("[file:a/]", &[]).badges.unwrap()[0].label, "a/");
    let opts = metadata("[file:a] [folder:a] [skill:review-code]", &skills());
    assert_eq!(message_badges(&opts).unwrap().len(), 3);
    assert!(message_badges(&g::SendMessageOptions::default()).is_none());
}

#[test]
fn pinned_upstream_parser_and_badge_oracle() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/upstream-mentions.json")).unwrap();
    let loaded: Vec<g::LoadedSkill> = serde_json::from_value(fixture["skills"].clone()).unwrap();
    let skills = loaded
        .iter()
        .map(|s| Skill::from_loaded(s, fixture["workspace"].as_str().unwrap()))
        .collect::<Vec<_>>();
    for case in fixture["cases"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        let parsed = parse(text, &skills);
        assert_eq!(
            serde_json::json!({"skills":parsed.skills,"invalidSkills":parsed.invalid_skills,"files":parsed.files,"folders":parsed.folders}),
            case["parsed"],
            "{text}"
        );
        let expected: Vec<g::SendMessageOptionsBadgesItem> =
            serde_json::from_value(case["badges"].clone()).unwrap();
        assert_eq!(
            serde_json::to_value(metadata(text, &skills).badges.unwrap_or_default()).unwrap(),
            serde_json::to_value(expected).unwrap(),
            "{text}"
        );
    }
}

#[test]
fn stale_project_cache_cannot_supply_labels_or_slugs_to_new_directory() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/upstream-mentions.json")).unwrap();
    let loaded: Vec<g::LoadedSkill> = serde_json::from_value(fixture["skills"].clone()).unwrap();
    let a = catalog(&loaded, "workspace", Some("/project/a"), Some("/project/a"));
    assert_eq!(
        metadata("[skill:.agents:project-review]", &a).skill_slugs,
        Some(vec!["project-review".into()])
    );
    let b = catalog(&loaded, "workspace", Some("/project/a"), Some("/project/b"));
    let options = metadata("[skill:.agents:project-review]", &b);
    assert!(options.badges.is_none());
    assert!(options.skill_slugs.is_none());
    assert_eq!(b.len(), 2);
    assert!(b.iter().any(|s| s.slug == "review-code"));
    assert!(b.iter().any(|s| s.slug == "global-review"));
    assert_eq!(catalog(&loaded, "workspace", None, None).len(), 2);
}
