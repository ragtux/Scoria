//! Every HTML fixture the 2026-11-15 probe pasted into Obsidian 1.15.0,
//! converted here and compared with the text Obsidian's editor held after
//! the paste (`obsidian-html-paste.json`, written from
//! `tools/obsidian-probe/results/2026-11-15-paste-html-and-drops.json`).

use serde_json::Value;

#[test]
fn every_fixture_converts_to_obsidians_markdown() {
    let doc: Value =
        serde_json::from_str(include_str!("obsidian-html-paste.json")).expect("fixture json");
    let cases = doc["cases"].as_array().expect("cases");
    assert!(cases.len() >= 50, "{} cases", cases.len());
    let mut failures = Vec::new();
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let html = case["html"].as_str().unwrap();
        let want = case["markdown"].as_str().unwrap();
        let got = scoria_paste::to_markdown(html);
        if got != want {
            failures.push(format!("{id}:\n  want {want:?}\n  got  {got:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}
