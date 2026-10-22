//! The document format end to end: the checked-in schema, the sample
//! fixture, strictness and the semantic rules.

use std::fs;
use std::path::PathBuf;

use kinora_project::schema::{project_schema, schema_file_name};
use kinora_project::{LoadError, from_json, to_json};
use serde_json::{Value, json};

const SAMPLE: &str = include_str!("fixtures/v0/sample.kinora/project.json");

fn sample() -> Value {
    serde_json::from_str(SAMPLE).expect("the sample is JSON")
}

fn schema_errors(instance: &Value) -> Vec<String> {
    let validator = jsonschema::validator_for(&project_schema()).expect("the schema compiles");
    validator
        .iter_errors(instance)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect()
}

/// Changes one thing in a document.
type Mutation = Box<dyn Fn(&mut Value)>;

/// Loads a JSON value through the real parser.
fn load(value: &Value) -> Result<kinora_project::Project, LoadError> {
    from_json(&value.to_string())
}

#[test]
fn schema_is_up_to_date() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(schema_file_name());
    let checked_in: Option<Value> = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok());
    assert!(
        checked_in.as_ref() == Some(&project_schema()),
        "{} is stale. Run `cargo run -p kinora-ipc --example generate` and commit the result.",
        path.display()
    );
}

#[test]
fn sample_matches_the_schema() {
    let errors = schema_errors(&sample());
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn sample_loads_and_is_canonical() {
    let project = from_json(SAMPLE).expect("the sample is valid");
    assert_eq!(project.timelines.len(), 2);
    assert_eq!(project.media.len(), 4);
    // Writing it back yields the same JSON value: the sample holds every
    // field, canonical rationals and the float spellings serde_json emits.
    let written = to_json(&project).expect("the sample can be written");
    let written: Value = serde_json::from_str(&written).expect("output is JSON");
    assert_eq!(written, sample());
}

/// Each case breaks one structural rule; the schema and the parser must both
/// reject it, so neither is more lenient than the other.
#[test]
fn schema_and_parser_agree_on_structure() {
    let cases: Vec<(&str, Mutation)> = vec![
        ("unknown root field", Box::new(|d| d["extra"] = json!(1))),
        (
            "unknown nested field",
            Box::new(|d| d["settings"]["audio"]["bitDepth"] = json!(24)),
        ),
        (
            "missing field",
            Box::new(|d| {
                if let Some(media) = d["media"][0].as_object_mut() {
                    media.remove("hash");
                }
            }),
        ),
        ("newer version", Box::new(|d| d["schemaVersion"] = json!(1))),
        (
            "zero denominator",
            Box::new(|d| {
                d["timelines"][0]["tracks"][0]["clips"][0]["start"] = json!({ "num": 0, "den": 0 })
            }),
        ),
        (
            "unsafe numerator",
            Box::new(|d| {
                d["timelines"][0]["tracks"][0]["clips"][0]["start"] =
                    json!({ "num": 9_007_199_254_740_992_i64, "den": 1 })
            }),
        ),
        (
            "float time",
            Box::new(|d| d["timelines"][0]["tracks"][0]["clips"][0]["start"] = json!(1.5)),
        ),
        (
            "unknown enum value",
            Box::new(|d| d["settings"]["colorSpace"] = json!("rec601")),
        ),
        (
            "unknown tag",
            Box::new(|d| {
                d["timelines"][0]["tracks"][0]["clips"][0]["content"]["kind"] = json!("hologram")
            }),
        ),
        (
            "field of another variant",
            Box::new(|d| {
                d["timelines"][0]["tracks"][0]["clips"][2]["content"]["text"] = json!("x")
            }),
        ),
        ("bad uuid", Box::new(|d| d["id"] = json!("not-a-uuid"))),
        (
            "non-positive frame rate",
            Box::new(|d| d["timelines"][1]["format"]["frameRate"] = json!({ "num": 0, "den": 1 })),
        ),
    ];
    for (name, mutate) in cases {
        let mut document = sample();
        mutate(&mut document);
        assert!(load(&document).is_err(), "parser accepted: {name}");
        // `format: uuid` is an annotation in draft 2020-12, so only the parser
        // checks UUID syntax.
        if name != "bad uuid" {
            assert!(
                !schema_errors(&document).is_empty(),
                "schema accepted: {name}"
            );
        }
    }
}

/// Each case is structurally valid but breaks a semantic rule.
#[test]
fn semantic_rules_are_enforced() {
    let clips = "/timelines/0/tracks/0/clips";
    let cases: Vec<(&str, String, Mutation)> = vec![
        (
            "overlap",
            format!("{clips}/1/start"),
            Box::new(|d| {
                d["timelines"][0]["tracks"][0]["clips"][1]["start"] = json!({ "num": 4, "den": 1 });
            }),
        ),
        (
            "dangling media",
            format!("{clips}/0/content/media"),
            Box::new(|d| {
                d["timelines"][0]["tracks"][0]["clips"][0]["content"]["media"] =
                    json!("0192f1c4-0000-7000-8000-00000000ffff");
            }),
        ),
        (
            "duplicate id",
            format!("{clips}/1/id"),
            Box::new(|d| {
                d["timelines"][0]["tracks"][0]["clips"][1]["id"] =
                    d["timelines"][0]["tracks"][0]["clips"][0]["id"].clone();
            }),
        ),
        (
            "source past media end",
            format!("{clips}/1/content"),
            Box::new(|d| {
                d["timelines"][0]["tracks"][0]["clips"][1]["content"]["sourceIn"] =
                    json!({ "num": 25, "den": 1 });
            }),
        ),
        (
            "keyframe after clip end",
            format!("{clips}/0/visual/transform/position/keyframes/1/at"),
            Box::new(|d| {
                d["timelines"][0]["tracks"][0]["clips"][0]["visual"]["transform"]["position"]["keyframes"]
                    [1]["at"] = json!({ "num": 6, "den": 1 });
            }),
        ),
        (
            "audio on a text track",
            "/timelines/0/tracks/3/clips/0/audio".into(),
            Box::new(|d| {
                d["timelines"][0]["tracks"][3]["clips"][0]["audio"] =
                    d["timelines"][0]["tracks"][2]["clips"][1]["audio"].clone();
            }),
        ),
        (
            "text on a video track",
            "/timelines/0/tracks/0/clips/2/content".into(),
            Box::new(|d| {
                d["timelines"][0]["tracks"][0]["clips"][2]["content"] =
                    d["timelines"][0]["tracks"][3]["clips"][0]["content"].clone();
            }),
        ),
        (
            "transition across a gap",
            "/timelines/0/tracks/0/transitions/0".into(),
            Box::new(|d| {
                d["timelines"][0]["tracks"][0]["transitions"][0]["to"] =
                    d["timelines"][0]["tracks"][0]["clips"][2]["id"].clone();
            }),
        ),
        (
            "mixed parameter types",
            format!("{clips}/0/visual/effects/0/params/radius/keyframes/1/value"),
            Box::new(|d| {
                d["timelines"][0]["tracks"][0]["clips"][0]["visual"]["effects"][0]["params"]["radius"]
                    ["keyframes"][1]["value"] = json!({ "bool": true });
            }),
        ),
        (
            "odd canvas",
            "/timelines/1/format/width".into(),
            Box::new(|d| {
                d["timelines"][1]["format"]["width"] = json!(1081);
            }),
        ),
        (
            "opacity above one",
            format!("{clips}/0/visual/opacity/value"),
            Box::new(|d| {
                d["timelines"][0]["tracks"][0]["clips"][0]["visual"]["opacity"]["value"] =
                    json!(1.5);
            }),
        ),
        (
            "absolute relative path",
            "/media/0/location/relativePath".into(),
            Box::new(|d| {
                d["media"][0]["location"]["relativePath"] = json!("C:/Footage/beach.mp4");
            }),
        ),
    ];
    for (name, pointer, mutate) in cases {
        let mut document = sample();
        mutate(&mut document);
        match load(&document) {
            Err(LoadError::Invalid { issues }) => assert!(
                issues.iter().any(|issue| issue.path == pointer),
                "{name}: expected an issue at {pointer}, got {issues:#?}"
            ),
            other => panic!("{name}: expected LoadError::Invalid, got {other:?}"),
        }
    }
}

#[test]
fn non_canonical_rationals_are_normalised() {
    let mut document = sample();
    document["timelines"][0]["tracks"][0]["clips"][0]["duration"] = json!({ "num": 10, "den": 2 });
    let project = load(&document).expect("valid");
    let written: Value = serde_json::from_str(&to_json(&project).expect("writes")).expect("JSON");
    assert_eq!(written, sample());
}
