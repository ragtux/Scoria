use super::*;

#[test]
fn only_a_mermaid_info_string_is_a_diagram() {
    assert!(is_mermaid(Some("mermaid")));
    assert!(!is_mermaid(Some("mmd")));
    assert!(!is_mermaid(None));
}

#[test]
fn errors_carry_peles_messages() {
    let t = Theme::light();
    // Observed in Obsidian 1.15.1, byte for byte.
    let e = render("notADiagram\n    A --> B", 700.0, &t).unwrap_err();
    assert_eq!(e.message, "No diagram type detected for the given text.");
    let e = render("", 700.0, &t).unwrap_err();
    assert_eq!(e.message, "No diagram type detected for the given text.");
    let e = render("flowchart TD\n    A --> B -->\n    ((", 700.0, &t).unwrap_err();
    assert_eq!(
        e.message,
        "Parse error on line 3:\n    ((\n----^\nExpecting 'NODE_STRING', got 'PS'"
    );
}

#[test]
fn a_result_is_cached_and_a_theme_repaints_without_moving_anything() {
    let src = "flowchart LR\n    A --> B";
    let a = render(src, 700.0, &Theme::light()).unwrap();
    let b = render(src, 700.4, &Theme::light()).unwrap();
    assert!(
        Arc::ptr_eq(&a, &b),
        "a fraction of a pixel lays out the same"
    );
    let dark = render(src, 700.0, &Theme::dark()).unwrap();
    assert_eq!((a.width, a.height), (dark.width, dark.height));
    assert_ne!(a.shapes, dark.shapes);
}

#[test]
fn every_text_run_is_anchored_and_coloured() {
    let d = render(
        "sequenceDiagram\n    Alice->>Bob: Hi\n    Note over Alice: x",
        700.0,
        &Theme::dark(),
    )
    .unwrap();
    let runs: Vec<_> = d
        .shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Text(t) => Some(t),
            _ => None,
        })
        .collect();
    assert!(runs.iter().any(|t| t.spans[0].text == "Alice"));
    assert!(runs.iter().all(|t| t.color.a > 0 && !t.spans.is_empty()));
}

#[test]
fn a_turned_point_turns_about_its_centre() {
    let mut l = List::default();
    l.turns.push((90.0, Point::new(10.0, 10.0)));
    let (x, y) = l.turn(20.0, 10.0);
    assert!(
        (x - 10.0).abs() < 1e-9 && (y - 20.0).abs() < 1e-9,
        "{x},{y}"
    );
}
