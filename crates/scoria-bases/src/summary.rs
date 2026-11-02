//! Column summaries: the options Obsidian offers a column and what each one
//! computes over the column's values.
//!
//! **Observed** (`tools/obsidian-probe/results/2026-11-01-bases-ui.json`,
//! `s08` `submenus`/`summaryMenus`, `s18` `footer`/`footerMenu`, `s24`
//! `formulaHeaderSub`): the Summarize page lists, **alphabetically without
//! regard to case**, the built-in summaries that apply to the column's values
//! together with the base's own `summaries` formulas, each with its value
//! computed over the column. A number column (and a formula that yields
//! numbers) offers *Average*, *Max*, *Median*, *Min*, *Range*, *Stddev* and
//! *Sum* with *Empty*, *Filled* and *Unique*; a date column *Earliest*,
//! *Latest* and *Range* with the same three; a checkbox column *Checked* and
//! *Unchecked* with them; text, lists and files the three alone. A number
//! shows rounded to two decimals (`Average 3.71`, `Stddev 1.03`); a date
//! summary shows the date-time (`2026-02-28T00:00:00`); a date *Range* is
//! the humanized duration (`3 months`). The icons: `binary` for every
//! number-valued summary, `clock` for *Earliest* and *Latest*,
//! `calendar-range` for a date *Range*, `square-function` for a custom one.
//!
//! The view writes `summaries: {<id>: <Name>}` under the column's id as
//! `order` spells it (`rating: Average`, `formula.score: Max`), and *Hide
//! summary* removes the key. A custom summary's statement runs with
//! `values` bound to the column's values, the way the base's own
//! `customMean: values.mean().round(2)` reads.
//!
//! **Unprobed**: whether *Stddev* is the population or the sample deviation
//! — the list method of the same name decides here — and what *Add summary*
//! creates.

use crate::context::SimpleContext;
use crate::eval::eval_str;
use crate::funcs::time::{between, humanize};
use crate::value::{FileData, Value};
use indexmap::IndexMap;
use std::sync::Arc;

/// What a column's values are, which decides its built-in options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueClass {
    Number,
    Date,
    Checkbox,
    Other,
}

/// The class of a column from its values: the first non-null value decides,
/// the way a property's kind is inferred from its first value.
pub fn class_of<'a>(values: impl IntoIterator<Item = &'a Value>) -> ValueClass {
    for value in values {
        match value {
            Value::Null => continue,
            Value::Number(_) => return ValueClass::Number,
            Value::Date(_) => return ValueClass::Date,
            Value::Bool(_) => return ValueClass::Checkbox,
            _ => return ValueClass::Other,
        }
    }
    ValueClass::Other
}

/// The built-in summaries, each with the classes it applies to.
const BUILTIN: [(&str, &[ValueClass]); 14] = [
    ("Average", &[ValueClass::Number]),
    ("Checked", &[ValueClass::Checkbox]),
    ("Earliest", &[ValueClass::Date]),
    (
        "Empty",
        &[
            ValueClass::Number,
            ValueClass::Date,
            ValueClass::Checkbox,
            ValueClass::Other,
        ],
    ),
    (
        "Filled",
        &[
            ValueClass::Number,
            ValueClass::Date,
            ValueClass::Checkbox,
            ValueClass::Other,
        ],
    ),
    ("Latest", &[ValueClass::Date]),
    ("Max", &[ValueClass::Number]),
    ("Median", &[ValueClass::Number]),
    ("Min", &[ValueClass::Number]),
    ("Range", &[ValueClass::Number, ValueClass::Date]),
    ("Stddev", &[ValueClass::Number]),
    ("Sum", &[ValueClass::Number]),
    ("Unchecked", &[ValueClass::Checkbox]),
    (
        "Unique",
        &[
            ValueClass::Number,
            ValueClass::Date,
            ValueClass::Checkbox,
            ValueClass::Other,
        ],
    ),
];

/// One row of the Summarize page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryOption {
    /// The name the view writes.
    pub name: String,
    /// A `summaries` formula of the base rather than a built-in.
    pub custom: bool,
}

/// The options a column of `class` offers, with the base's `custom`
/// summaries among them, in Obsidian's case-insensitive alphabetical order.
pub fn options(class: ValueClass, custom: &[String]) -> Vec<SummaryOption> {
    let mut out: Vec<SummaryOption> = BUILTIN
        .iter()
        .filter(|(_, classes)| classes.contains(&class))
        .map(|(name, _)| SummaryOption {
            name: name.to_string(),
            custom: false,
        })
        .collect();
    out.extend(custom.iter().map(|name| SummaryOption {
        name: name.clone(),
        custom: true,
    }));
    out.sort_by_key(|o| o.name.to_lowercase());
    out
}

/// The icon a summary row wears.
pub fn icon_of(name: &str, class: ValueClass, custom: bool) -> &'static str {
    if custom {
        return "square-function";
    }
    match (name, class) {
        ("Earliest", _) | ("Latest", _) => "clock",
        ("Range", ValueClass::Date) => "calendar-range",
        _ => "binary",
    }
}

/// Whether a value counts as empty for *Empty* and *Filled*.
fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Str(s) => s.is_empty(),
        Value::List(items) => items.is_empty(),
        Value::Tags(tags) => tags.is_empty(),
        _ => false,
    }
}

/// A number as a summary shows it: rounded to two decimals, printed the way
/// a cell prints one.
fn number_text(n: f64) -> String {
    let rounded = (n * 100.).round() / 100.;
    Value::Number(rounded).to_string()
}

/// What `name` computes over `values` — the text the summary cell draws.
/// `custom` is the statement of a `summaries` formula when `name` is one.
/// An empty string is a summary with nothing to say (a *Max* over no
/// numbers), which the cell draws as such.
pub fn compute(name: &str, custom: Option<&str>, values: &[Value]) -> String {
    if let Some(statement) = custom {
        return evaluate_custom(statement, values);
    }
    let filled: Vec<&Value> = values.iter().filter(|v| !is_empty(v)).collect();
    let numbers: Vec<f64> = values
        .iter()
        .filter_map(|v| match v {
            Value::Number(n) => Some(*n),
            _ => None,
        })
        .collect();
    let dates: Vec<&crate::value::DateValue> = values
        .iter()
        .filter_map(|v| match v {
            Value::Date(d) => Some(d),
            _ => None,
        })
        .collect();
    match name {
        "Empty" => (values.len() - filled.len()).to_string(),
        "Filled" => filled.len().to_string(),
        "Unique" => {
            let mut seen: Vec<String> = Vec::new();
            for v in &filled {
                let text = v.to_string();
                if !seen.contains(&text) {
                    seen.push(text);
                }
            }
            seen.len().to_string()
        }
        "Checked" => values
            .iter()
            .filter(|v| matches!(v, Value::Bool(true)))
            .count()
            .to_string(),
        "Unchecked" => values
            .iter()
            .filter(|v| matches!(v, Value::Bool(false)))
            .count()
            .to_string(),
        "Earliest" => dates
            .iter()
            .min_by_key(|d| d.at)
            .map(|d| d.at.format("%Y-%m-%dT%H:%M:%S").to_string())
            .unwrap_or_default(),
        "Latest" => dates
            .iter()
            .max_by_key(|d| d.at)
            .map(|d| d.at.format("%Y-%m-%dT%H:%M:%S").to_string())
            .unwrap_or_default(),
        "Range" if !dates.is_empty() && numbers.is_empty() => {
            let earliest = dates.iter().min_by_key(|d| d.at).unwrap();
            let latest = dates.iter().max_by_key(|d| d.at).unwrap();
            humanize(&between(earliest, latest))
        }
        _ if numbers.is_empty() => String::new(),
        "Average" => number_text(numbers.iter().sum::<f64>() / numbers.len() as f64),
        "Sum" => number_text(numbers.iter().sum()),
        "Max" => number_text(numbers.iter().cloned().fold(f64::MIN, f64::max)),
        "Min" => number_text(numbers.iter().cloned().fold(f64::MAX, f64::min)),
        "Range" => number_text(
            numbers.iter().cloned().fold(f64::MIN, f64::max)
                - numbers.iter().cloned().fold(f64::MAX, f64::min),
        ),
        "Median" => {
            let mut sorted = numbers.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let n = sorted.len();
            let median = if n % 2 == 1 {
                sorted[n / 2]
            } else {
                (sorted[n / 2 - 1] + sorted[n / 2]) / 2.
            };
            number_text(median)
        }
        "Stddev" => evaluate_custom("values.stddev()", values),
        _ => String::new(),
    }
}

/// Runs a summary statement with `values` bound to the column's values.
fn evaluate_custom(statement: &str, values: &[Value]) -> String {
    let mut properties = IndexMap::new();
    properties.insert("values".to_string(), Value::List(values.to_vec()));
    let ctx = SimpleContext {
        file: Some(Arc::new(FileData {
            properties,
            ..FileData::default()
        })),
        ..SimpleContext::default()
    };
    match eval_str(statement, &ctx) {
        Ok(Value::Number(n)) => number_text(n),
        Ok(Value::Null) => String::new(),
        Ok(v) => v.to_string(),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::DateValue;
    use chrono::NaiveDate;

    fn n(v: f64) -> Value {
        Value::Number(v)
    }

    fn date(y: i32, m: u32, d: u32) -> Value {
        Value::Date(DateValue {
            at: NaiveDate::from_ymd_opt(y, m, d)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            has_time: false,
        })
    }

    #[test]
    fn the_options_are_alphabetical_per_class_with_the_customs_among_them() {
        let custom = vec!["customMean".to_string()];
        let names = |class| -> Vec<String> {
            options(class, &custom)
                .into_iter()
                .map(|o| o.name)
                .collect()
        };
        // `s18` `footerMenu` on Rating.
        assert_eq!(
            names(ValueClass::Number),
            [
                "Average",
                "customMean",
                "Empty",
                "Filled",
                "Max",
                "Median",
                "Min",
                "Range",
                "Stddev",
                "Sum",
                "Unique"
            ]
        );
        // `s18` `summaryMenus` on `file name`, on `due` and on `done`.
        assert_eq!(
            names(ValueClass::Other),
            ["customMean", "Empty", "Filled", "Unique"]
        );
        assert_eq!(
            names(ValueClass::Date),
            [
                "customMean",
                "Earliest",
                "Empty",
                "Filled",
                "Latest",
                "Range",
                "Unique"
            ]
        );
        assert_eq!(
            names(ValueClass::Checkbox),
            [
                "Checked",
                "customMean",
                "Empty",
                "Filled",
                "Unchecked",
                "Unique"
            ]
        );
        assert!(options(ValueClass::Number, &custom)
            .iter()
            .any(|o| o.custom && o.name == "customMean"));
    }

    #[test]
    fn the_numbers_round_to_two_decimals_like_the_footer() {
        // `s18` `footer`: Average 3.71 over Sum 26, Filled 7, Max 5, Min 2,
        // Median 4, Range 3, Unique 4.
        let values = vec![n(2.), n(3.), n(4.), n(4.), n(4.), n(4.), n(5.)];
        assert_eq!(compute("Average", None, &values), "3.71");
        assert_eq!(compute("Sum", None, &values), "26");
        assert_eq!(compute("Max", None, &values), "5");
        assert_eq!(compute("Min", None, &values), "2");
        assert_eq!(compute("Median", None, &values), "4");
        assert_eq!(compute("Range", None, &values), "3");
        assert_eq!(compute("Unique", None, &values), "4");
        assert_eq!(compute("Filled", None, &values), "7");
        assert_eq!(compute("Empty", None, &values), "0");
        assert_eq!(compute("Average", None, &[n(3.), n(4.), n(7.)]), "4.67");
        assert_eq!(compute("Average", None, &[Value::Null]), "");
        // A custom statement runs over `values`.
        assert_eq!(
            compute("customMean", Some("values.mean().round(2)"), &values),
            "3.71"
        );
    }

    #[test]
    fn dates_summarize_to_date_times_and_a_humanized_range() {
        // `s18` `summaryMenus` on `due`: Earliest 2026-02-28T00:00:00,
        // Latest 2026-05-20T00:00:00, Range "3 months".
        let values = vec![date(2026, 2, 28), Value::Null, date(2026, 5, 20)];
        assert_eq!(class_of(&values), ValueClass::Date);
        assert_eq!(compute("Earliest", None, &values), "2026-02-28T00:00:00");
        assert_eq!(compute("Latest", None, &values), "2026-05-20T00:00:00");
        assert_eq!(compute("Range", None, &values), "3 months");
        assert_eq!(compute("Empty", None, &values), "1");
        assert_eq!(compute("Filled", None, &values), "2");
        assert_eq!(compute("Unique", None, &values), "2");
    }

    #[test]
    fn checkboxes_count_and_the_class_follows_the_first_value() {
        let values = vec![Value::Bool(false), Value::Bool(false), Value::Null];
        assert_eq!(class_of(&values), ValueClass::Checkbox);
        assert_eq!(compute("Checked", None, &values), "0");
        assert_eq!(compute("Unchecked", None, &values), "2");
        assert_eq!(
            class_of(&[Value::Null, Value::Str("x".into())]),
            ValueClass::Other
        );
        assert_eq!(class_of(&[]), ValueClass::Other);
        assert_eq!(icon_of("Earliest", ValueClass::Date, false), "clock");
        assert_eq!(icon_of("Range", ValueClass::Date, false), "calendar-range");
        assert_eq!(icon_of("Range", ValueClass::Number, false), "binary");
        assert_eq!(
            icon_of("customMean", ValueClass::Number, true),
            "square-function"
        );
    }
}
