//! Obsidian's **structured filter rows**: a statement read as
//! *property · operator · value*, and the statement such a row writes back.
//!
//! The filter panel shows each statement of a `filters` group as a row of
//! three controls when the statement has one of a few recognised shapes, and
//! as a raw expression (the "Advanced filter" mode, `lucide-code-xml`)
//! otherwise. This module is the gpui-free half of that: [`parse_rule`] reads
//! a statement into a [`Rule`] or says it has no row shape, and
//! [`Rule::statement`] spells a row back out. Nothing here evaluates
//! anything, and nothing here touches `scoria-formats`: a `.base` still holds
//! its statements verbatim, and a row only rewrites the one statement whose
//! controls were edited.
//!
//! # Observed, Obsidian 1.14.5 (installer build 1.13.7)
//!
//! Read off the filter panel of `tools/obsidian-probe/bases-ui.mjs`'s fixture
//! (`results/2026-11-01-bases-ui.json`, step `s05-filters`, key `operators`):
//! the operator combobox of one row per property type, in the order the list
//! shows them, and the row each fixture statement rendered as.
//!
//! | Property type (icon) | Operators, in combobox order |
//! | --- | --- |
//! | file (`lucide-file`) | links to, in folder, has tag, has property, does not link to, is not in folder, does not have tag, does not have property |
//! | number (`lucide-binary`) | =, ≠, <, ≤, >, ≥, is empty, is not empty |
//! | text (`lucide-text`) | is, is not, starts with, ends with, is empty, contains, contains any of, contains all of, does not start with, does not end with, is not empty, does not contain, does not contain any of, does not contain all of |
//! | checkbox (`lucide-check-square`) | is, is not |
//!
//! | Statement | Row |
//! | --- | --- |
//! | `file.hasTag("rock")` | file · has tag · pills `rock` |
//! | `rating >= 1` | Rating · ≥ · `1` |
//! | `status != "archived"` | status · is not · `archived` |
//! | `done != "nope"` | done · is not · a checkbox |
//! | `file.hasLink(...)` | file · links to · text |
//!
//! Two things those pairs decide. A comparison is a row only when its left
//! side is the property and its right side a literal, which is why
//! `rating * 2 > 3` and `type + " " + file.name` are advanced filters. And a
//! checkbox property's `==` / `!=` is a row **whatever the right side is**:
//! `done != "nope"` kept its comparison and its string and showed the
//! checkbox control, so [`parse_rule`] keeps the literal as the row's value
//! and only a `true` / `false` literal becomes a [`RuleValue::Bool`]. The
//! file is never rewritten by reading it.
//!
//! # Read, not observed
//!
//! The operator lists of a **date**, **datetime**, **list** and **tags**
//! property were not captured — the fixture had no row on one. The readings,
//! both marked in [`operators_for`]: dates and datetimes take the number set
//! (the six comparisons, is empty, is not empty), with the comparison's
//! value read as the date's text; lists and tags take the text set, whose
//! *contains any of* / *contains all of* are list-shaped already. A formula
//! column and a property of unknown type take the text set too, which is the
//! widest.
//!
//! The statement each operator writes is read from the function table in
//! [[Specification#Observed Bases expression language, Obsidian 1.14.5 (installer build 1.13.7)]]
//! rather than from a write: `startsWith`, `endsWith`, `isEmpty`, `contains`
//! on the property; `containsAny` and `containsAll` **variadic**, one string
//! per argument, which is how the evaluator observed them (a single array
//! argument is unprobed and stays an advanced filter here); `file.hasTag`
//! with one argument per pill; `file.hasLink`, `file.inFolder` and
//! `file.hasProperty` with one string. A negated operator is a `!` in front
//! of the call — `!file.hasTag("x")`, `!status.isEmpty()` — because the
//! language has no other spelling for it and the panel's "does not ..." rows
//! must write something the evaluator reads. Function names are matched with
//! the spelling the table gives; `file.hastag("x")` evaluates but is not a
//! row. A `(` `)` around the whole statement is looked through on the way in
//! and never written on the way out.
//!
//! Numbers are kept as **typed text** ([`RuleValue::Number`] holds a
//! `String`), so `rating >= 1.50` survives a round trip as `1.50`; the
//! parser's `f64` is only the fallback when the statement's own spelling
//! cannot be recovered.

use crate::expr::{self, BinaryOp, Expr, Literal};

/// The property types the filter panel distinguishes when it fills a row's
/// operator combobox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyKind {
    /// The `file` object itself: `file.hasTag(...)` and its three siblings.
    File,
    Text,
    Number,
    Checkbox,
    Date,
    DateTime,
    List,
    Tags,
    /// A `formula.*` column.
    Formula,
    Unknown,
}

/// A row's operator. The labels are the combobox's own strings; the
/// variants are grouped by the property type whose list they belong to, in
/// the list's order. `is` / `is not` exist twice because a text row's `is`
/// takes a text and a checkbox row's `is` takes a checkbox, and the operator
/// alone has to say which control to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operator {
    // The file set.
    LinksTo,
    InFolder,
    HasTag,
    HasProperty,
    NotLinksTo,
    NotInFolder,
    NotHasTag,
    NotHasProperty,
    // The number set; `IsEmpty` / `IsNotEmpty` are shared with the text set.
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    IsEmpty,
    IsNotEmpty,
    // The text set.
    Is,
    IsNot,
    StartsWith,
    EndsWith,
    Contains,
    ContainsAny,
    ContainsAll,
    NotStartsWith,
    NotEndsWith,
    NotContains,
    NotContainsAny,
    NotContainsAll,
    // The checkbox set.
    CheckboxIs,
    CheckboxIsNot,
}

/// The control a row's value needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueKind {
    /// No value control: `is empty`, `is not empty`.
    None,
    Text,
    Number,
    Checkbox,
    /// Pills: `has tag`, `contains any of`, `contains all of`.
    List,
    /// A date picker: a comparison on a date property (read, not observed).
    Date,
    /// A note name: `links to`.
    Link,
    /// A folder path: `in folder`.
    Folder,
    /// A property name: `has property`.
    PropertyName,
}

/// The file set, in combobox order.
pub const FILE_OPERATORS: &[Operator] = &[
    Operator::LinksTo,
    Operator::InFolder,
    Operator::HasTag,
    Operator::HasProperty,
    Operator::NotLinksTo,
    Operator::NotInFolder,
    Operator::NotHasTag,
    Operator::NotHasProperty,
];

/// The number set, in combobox order. Dates and datetimes read this set.
pub const NUMBER_OPERATORS: &[Operator] = &[
    Operator::Eq,
    Operator::Ne,
    Operator::Lt,
    Operator::Le,
    Operator::Gt,
    Operator::Ge,
    Operator::IsEmpty,
    Operator::IsNotEmpty,
];

/// The text set, in combobox order. Lists, tags, formulas and properties of
/// unknown type read this set.
pub const TEXT_OPERATORS: &[Operator] = &[
    Operator::Is,
    Operator::IsNot,
    Operator::StartsWith,
    Operator::EndsWith,
    Operator::IsEmpty,
    Operator::Contains,
    Operator::ContainsAny,
    Operator::ContainsAll,
    Operator::NotStartsWith,
    Operator::NotEndsWith,
    Operator::IsNotEmpty,
    Operator::NotContains,
    Operator::NotContainsAny,
    Operator::NotContainsAll,
];

/// The checkbox set, in combobox order.
pub const CHECKBOX_OPERATORS: &[Operator] = &[Operator::CheckboxIs, Operator::CheckboxIsNot];

/// The operators a row on a property of this kind offers, in the order the
/// combobox lists them. File, Number, Text and Checkbox are observed; the
/// rest are the readings the module doc records.
pub fn operators_for(kind: PropertyKind) -> &'static [Operator] {
    match kind {
        PropertyKind::File => FILE_OPERATORS,
        PropertyKind::Number | PropertyKind::Date | PropertyKind::DateTime => NUMBER_OPERATORS,
        PropertyKind::Checkbox => CHECKBOX_OPERATORS,
        PropertyKind::Text
        | PropertyKind::List
        | PropertyKind::Tags
        | PropertyKind::Formula
        | PropertyKind::Unknown => TEXT_OPERATORS,
    }
}

impl Operator {
    /// The combobox's string for this operator.
    pub fn label(self) -> &'static str {
        match self {
            Operator::LinksTo => "links to",
            Operator::InFolder => "in folder",
            Operator::HasTag => "has tag",
            Operator::HasProperty => "has property",
            Operator::NotLinksTo => "does not link to",
            Operator::NotInFolder => "is not in folder",
            Operator::NotHasTag => "does not have tag",
            Operator::NotHasProperty => "does not have property",
            Operator::Eq => "=",
            Operator::Ne => "≠",
            Operator::Lt => "<",
            Operator::Le => "≤",
            Operator::Gt => ">",
            Operator::Ge => "≥",
            Operator::IsEmpty => "is empty",
            Operator::IsNotEmpty => "is not empty",
            Operator::Is | Operator::CheckboxIs => "is",
            Operator::IsNot | Operator::CheckboxIsNot => "is not",
            Operator::StartsWith => "starts with",
            Operator::EndsWith => "ends with",
            Operator::Contains => "contains",
            Operator::ContainsAny => "contains any of",
            Operator::ContainsAll => "contains all of",
            Operator::NotStartsWith => "does not start with",
            Operator::NotEndsWith => "does not end with",
            Operator::NotContains => "does not contain",
            Operator::NotContainsAny => "does not contain any of",
            Operator::NotContainsAll => "does not contain all of",
        }
    }

    /// The control the row's value needs for this operator. A comparison on
    /// a date property wants a date rather than a number; see
    /// [`Operator::value_kind_for`].
    pub fn value_kind(self) -> ValueKind {
        match self {
            Operator::LinksTo | Operator::NotLinksTo => ValueKind::Link,
            Operator::InFolder | Operator::NotInFolder => ValueKind::Folder,
            Operator::HasProperty | Operator::NotHasProperty => ValueKind::PropertyName,
            Operator::HasTag
            | Operator::NotHasTag
            | Operator::ContainsAny
            | Operator::ContainsAll
            | Operator::NotContainsAny
            | Operator::NotContainsAll => ValueKind::List,
            Operator::Eq
            | Operator::Ne
            | Operator::Lt
            | Operator::Le
            | Operator::Gt
            | Operator::Ge => ValueKind::Number,
            Operator::IsEmpty | Operator::IsNotEmpty => ValueKind::None,
            Operator::Is
            | Operator::IsNot
            | Operator::StartsWith
            | Operator::EndsWith
            | Operator::Contains
            | Operator::NotStartsWith
            | Operator::NotEndsWith
            | Operator::NotContains => ValueKind::Text,
            Operator::CheckboxIs | Operator::CheckboxIsNot => ValueKind::Checkbox,
        }
    }

    /// [`Operator::value_kind`], except that a comparison on a [`Date`] or
    /// [`DateTime`] property takes a [`ValueKind::Date`].
    ///
    /// [`Date`]: PropertyKind::Date
    /// [`DateTime`]: PropertyKind::DateTime
    pub fn value_kind_for(self, kind: PropertyKind) -> ValueKind {
        match (self.value_kind(), kind) {
            (ValueKind::Number, PropertyKind::Date | PropertyKind::DateTime) => ValueKind::Date,
            (value, _) => value,
        }
    }

    /// Whether the statement is the negation of a call: the "does not ..." /
    /// "is not ..." operators that write a leading `!`.
    pub fn is_negated(self) -> bool {
        matches!(
            self,
            Operator::NotLinksTo
                | Operator::NotInFolder
                | Operator::NotHasTag
                | Operator::NotHasProperty
                | Operator::IsNotEmpty
                | Operator::NotStartsWith
                | Operator::NotEndsWith
                | Operator::NotContains
                | Operator::NotContainsAny
                | Operator::NotContainsAll
        )
    }
}

/// A row's value. Numbers are the text as typed, so `1.50` is written back as
/// `1.50`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleValue {
    /// Not filled in yet, or an operator that takes none.
    None,
    Text(String),
    Number(String),
    Bool(bool),
    List(Vec<String>),
}

/// One filter row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// The property as the statement spells it: `status`, `note.status`,
    /// `file`, `formula.score`, `file.name`.
    pub property: String,
    pub operator: Operator,
    pub value: RuleValue,
}

/// The row "Add filter" makes for a property: the first operator of its kind
/// and no value.
pub fn new_rule(property: &str, kind: PropertyKind) -> Rule {
    Rule {
        property: property.to_string(),
        operator: operators_for(kind)[0],
        value: RuleValue::None,
    }
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

/// Reads a statement as a row, or `None` when it has no row shape and the
/// panel shows it as an advanced filter. `kind_of` answers the type of a
/// property as the statement spells it (`status`, `note.status`,
/// `file.name`, `formula.score`); the four `file.*` calls never ask it.
pub fn parse_rule(statement: &str, kind_of: &dyn Fn(&str) -> PropertyKind) -> Option<Rule> {
    let expr = expr::parse(statement).ok()?;
    let (expr, depth) = unparen(&expr);
    match expr {
        Expr::Binary { op, left, right } => comparison(statement, depth, *op, left, right, kind_of),
        Expr::Function {
            name,
            subject: Some(subject),
            args,
        } => call(name, subject, args, false, kind_of),
        Expr::Not(inner) => match unparen(inner).0 {
            Expr::Function {
                name,
                subject: Some(subject),
                args,
            } => call(name, subject, args, true, kind_of),
            _ => None,
        },
        _ => None,
    }
}

/// Looks through `Paren` nodes, counting how many were there.
fn unparen(mut expr: &Expr) -> (&Expr, usize) {
    let mut depth = 0;
    while let Expr::Paren(inner) = expr {
        expr = inner;
        depth += 1;
    }
    (expr, depth)
}

/// `a`, `a.b`, `a.b.c`: idents joined by dots, which is the only shape a
/// property has.
fn property_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(name) => Some(name.clone()),
        Expr::ObjectAccess { object, index } => {
            let mut path = property_path(object)?;
            path.push('.');
            path.push_str(index);
            Some(path)
        }
        _ => None,
    }
}

fn comparison(
    statement: &str,
    depth: usize,
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    kind_of: &dyn Fn(&str) -> PropertyKind,
) -> Option<Rule> {
    let property = property_path(left)?;
    let Expr::Primitive(literal) = right else {
        return None;
    };
    let value = match literal {
        Literal::String(s) => RuleValue::Text(s.clone()),
        Literal::Number(n) => RuleValue::Number(number_text(statement, depth, *n)),
        Literal::Bool(b) => RuleValue::Bool(*b),
        Literal::Null => return None,
    };
    let kind = kind_of(&property);
    let operator = match kind {
        PropertyKind::File => return None,
        PropertyKind::Number => {
            // A number row compares against a number; anything else is an
            // advanced filter rather than a number control holding text.
            if !matches!(value, RuleValue::Number(_)) {
                return None;
            }
            number_operator(op)
        }
        // Read: a date's comparison carries the date as text, and a number
        // (a timestamp) is not refused either.
        PropertyKind::Date | PropertyKind::DateTime => {
            if !matches!(value, RuleValue::Number(_) | RuleValue::Text(_)) {
                return None;
            }
            number_operator(op)
        }
        // Observed: `done != "nope"` is a checkbox row, whatever the right
        // side is.
        PropertyKind::Checkbox => match op {
            BinaryOp::Eq => Operator::CheckboxIs,
            BinaryOp::Ne => Operator::CheckboxIsNot,
            _ => return None,
        },
        PropertyKind::Text
        | PropertyKind::List
        | PropertyKind::Tags
        | PropertyKind::Formula
        | PropertyKind::Unknown => match op {
            BinaryOp::Eq => Operator::Is,
            BinaryOp::Ne => Operator::IsNot,
            _ => return None,
        },
    };
    Some(Rule {
        property,
        operator,
        value,
    })
}

fn number_operator(op: BinaryOp) -> Operator {
    match op {
        BinaryOp::Eq => Operator::Eq,
        BinaryOp::Ne => Operator::Ne,
        BinaryOp::Lt => Operator::Lt,
        BinaryOp::Le => Operator::Le,
        BinaryOp::Gt => Operator::Gt,
        BinaryOp::Ge => Operator::Ge,
        // The caller has matched a comparison; `&&`, `||` and the arithmetic
        // operators never reach here.
        _ => unreachable!("a comparison operator"),
    }
}

/// The right side of a `property OP number` statement as the author typed
/// it. The left side is a property path and so holds none of `< > = !`, which
/// makes the first of those the operator; `depth` outer parentheses are
/// stripped first. Falls back to the parsed value's own spelling when the
/// recovered text does not read as that number.
fn number_text(statement: &str, depth: usize, n: f64) -> String {
    let fallback = || format!("{n}");
    let mut text = statement.trim();
    for _ in 0..depth {
        let Some(inner) = text.strip_prefix('(') else {
            return fallback();
        };
        let Some(inner) = inner.trim_end().strip_suffix(')') else {
            return fallback();
        };
        text = inner.trim();
    }
    let Some(at) = text.find(['<', '>', '=', '!']) else {
        return fallback();
    };
    let rest = &text[at..];
    let rest = rest
        .strip_prefix("<=")
        .or_else(|| rest.strip_prefix(">="))
        .or_else(|| rest.strip_prefix("=="))
        .or_else(|| rest.strip_prefix("!="))
        .or_else(|| rest.strip_prefix('<'))
        .or_else(|| rest.strip_prefix('>'))
        .unwrap_or(rest)
        .trim();
    match rest.parse::<f64>() {
        Ok(parsed) if parsed == n => rest.to_string(),
        _ => fallback(),
    }
}

/// Every argument a string literal, else `None`.
fn string_args(args: &[Expr]) -> Option<Vec<String>> {
    args.iter()
        .map(|arg| match arg {
            Expr::Primitive(Literal::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

/// A list argument: no arguments is "no value" rather than an empty list, so
/// the statement a fresh row writes reads back as that row.
fn list_value(args: &[Expr]) -> Option<RuleValue> {
    let items = string_args(args)?;
    Some(if items.is_empty() {
        RuleValue::None
    } else {
        RuleValue::List(items)
    })
}

/// Exactly one string argument.
fn text_value(args: &[Expr]) -> Option<RuleValue> {
    match args {
        [Expr::Primitive(Literal::String(s))] => Some(RuleValue::Text(s.clone())),
        _ => None,
    }
}

fn call(
    name: &str,
    subject: &Expr,
    args: &[Expr],
    negated: bool,
    kind_of: &dyn Fn(&str) -> PropertyKind,
) -> Option<Rule> {
    fn pick(negated: bool, plain: Operator, negative: Operator) -> Operator {
        if negated {
            negative
        } else {
            plain
        }
    }

    if matches!(subject, Expr::Ident(s) if s == "file") {
        let file = |operator, value| {
            Some(Rule {
                property: "file".to_string(),
                operator,
                value,
            })
        };
        match name {
            "hasTag" => {
                return file(
                    pick(negated, Operator::HasTag, Operator::NotHasTag),
                    list_value(args)?,
                )
            }
            "hasLink" => {
                return file(
                    pick(negated, Operator::LinksTo, Operator::NotLinksTo),
                    text_value(args)?,
                )
            }
            "inFolder" => {
                return file(
                    pick(negated, Operator::InFolder, Operator::NotInFolder),
                    text_value(args)?,
                )
            }
            "hasProperty" => {
                return file(
                    pick(negated, Operator::HasProperty, Operator::NotHasProperty),
                    text_value(args)?,
                )
            }
            _ => {}
        }
    }

    let property = property_path(subject)?;
    let (operator, value) = match name {
        "startsWith" => (
            pick(negated, Operator::StartsWith, Operator::NotStartsWith),
            text_value(args)?,
        ),
        "endsWith" => (
            pick(negated, Operator::EndsWith, Operator::NotEndsWith),
            text_value(args)?,
        ),
        "contains" => (
            pick(negated, Operator::Contains, Operator::NotContains),
            text_value(args)?,
        ),
        "containsAny" => (
            pick(negated, Operator::ContainsAny, Operator::NotContainsAny),
            list_value(args)?,
        ),
        "containsAll" => (
            pick(negated, Operator::ContainsAll, Operator::NotContainsAll),
            list_value(args)?,
        ),
        "isEmpty" if args.is_empty() => (
            pick(negated, Operator::IsEmpty, Operator::IsNotEmpty),
            RuleValue::None,
        ),
        _ => return None,
    };
    // A number's combobox has no "starts with": the row would show an
    // operator the user could not have picked, so it is an advanced filter.
    if !operators_for(kind_of(&property)).contains(&operator) {
        return None;
    }
    Some(Rule {
        property,
        operator,
        value,
    })
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

impl Rule {
    /// The statement this row writes, in Obsidian's spelling: double-quoted
    /// strings with `"` and `\` escaped, numbers as typed, bare `true` /
    /// `false`, one space around a comparison, a leading `!` for a negated
    /// call. A row without a value writes a placeholder — `""`, `0`,
    /// `false`, or no arguments — so the file is always a valid statement.
    pub fn statement(&self) -> String {
        let op = self.operator;
        let not = if op.is_negated() { "!" } else { "" };
        let one = || self.scalar_text(op);
        let list = || self.list_text();
        match op {
            Operator::LinksTo | Operator::NotLinksTo => format!("{not}file.hasLink({})", one()),
            Operator::InFolder | Operator::NotInFolder => {
                format!("{not}file.inFolder({})", one())
            }
            Operator::HasTag | Operator::NotHasTag => format!("{not}file.hasTag({})", list()),
            Operator::HasProperty | Operator::NotHasProperty => {
                format!("{not}file.hasProperty({})", one())
            }
            Operator::Eq | Operator::Is | Operator::CheckboxIs => {
                format!("{} == {}", self.property, one())
            }
            Operator::Ne | Operator::IsNot | Operator::CheckboxIsNot => {
                format!("{} != {}", self.property, one())
            }
            Operator::Lt => format!("{} < {}", self.property, one()),
            Operator::Le => format!("{} <= {}", self.property, one()),
            Operator::Gt => format!("{} > {}", self.property, one()),
            Operator::Ge => format!("{} >= {}", self.property, one()),
            Operator::IsEmpty | Operator::IsNotEmpty => {
                format!("{not}{}.isEmpty()", self.property)
            }
            Operator::StartsWith | Operator::NotStartsWith => {
                format!("{not}{}.startsWith({})", self.property, one())
            }
            Operator::EndsWith | Operator::NotEndsWith => {
                format!("{not}{}.endsWith({})", self.property, one())
            }
            Operator::Contains | Operator::NotContains => {
                format!("{not}{}.contains({})", self.property, one())
            }
            Operator::ContainsAny | Operator::NotContainsAny => {
                format!("{not}{}.containsAny({})", self.property, list())
            }
            Operator::ContainsAll | Operator::NotContainsAll => {
                format!("{not}{}.containsAll({})", self.property, list())
            }
        }
    }

    /// The value as one literal, with the operator's placeholder when there
    /// is none.
    fn scalar_text(&self, op: Operator) -> String {
        match &self.value {
            RuleValue::Text(s) => quote(s),
            RuleValue::Number(n) => n.clone(),
            RuleValue::Bool(b) => b.to_string(),
            RuleValue::List(items) => quote(&items.join(", ")),
            RuleValue::None => match op.value_kind() {
                ValueKind::Number => "0".to_string(),
                ValueKind::Checkbox => "false".to_string(),
                _ => quote(""),
            },
        }
    }

    /// The value as an argument list.
    fn list_text(&self) -> String {
        match &self.value {
            RuleValue::List(items) => items
                .iter()
                .map(|s| quote(s))
                .collect::<Vec<_>>()
                .join(", "),
            RuleValue::Text(s) => quote(s),
            RuleValue::Number(n) => quote(n),
            RuleValue::Bool(b) => quote(&b.to_string()),
            RuleValue::None => String::new(),
        }
    }
}

/// A double-quoted string literal with `"` and `\` escaped, which is the
/// spelling the parser's JSON reading accepts back.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: &[PropertyKind] = &[
        PropertyKind::File,
        PropertyKind::Text,
        PropertyKind::Number,
        PropertyKind::Checkbox,
        PropertyKind::Date,
        PropertyKind::DateTime,
        PropertyKind::List,
        PropertyKind::Tags,
        PropertyKind::Formula,
        PropertyKind::Unknown,
    ];

    fn kinds(kind: PropertyKind) -> impl Fn(&str) -> PropertyKind {
        move |_| kind
    }

    /// A kind per property name, the way a host would answer.
    fn fixture(property: &str) -> PropertyKind {
        match property {
            "file" => PropertyKind::File,
            "rating" | "note.rating" => PropertyKind::Number,
            "done" => PropertyKind::Checkbox,
            "due" => PropertyKind::Date,
            "kinds" => PropertyKind::List,
            "tags" => PropertyKind::Tags,
            "formula.score" => PropertyKind::Formula,
            "status" | "file.name" | "type" => PropertyKind::Text,
            _ => PropertyKind::Unknown,
        }
    }

    fn sample(kind: ValueKind) -> RuleValue {
        match kind {
            ValueKind::None => RuleValue::None,
            ValueKind::Text => RuleValue::Text("a \"quoted\" \\ value".to_string()),
            ValueKind::Number => RuleValue::Number("1.50".to_string()),
            ValueKind::Checkbox => RuleValue::Bool(true),
            ValueKind::List => RuleValue::List(vec!["rock".to_string(), "b \"c\"".to_string()]),
            ValueKind::Date => RuleValue::Text("2025-01-31".to_string()),
            ValueKind::Link => RuleValue::Text("Sunset Crater".to_string()),
            ValueKind::Folder => RuleValue::Text("Rocks/Igneous".to_string()),
            ValueKind::PropertyName => RuleValue::Text("status".to_string()),
        }
    }

    #[test]
    fn every_operator_of_every_kind_round_trips() {
        for &kind in KINDS {
            let property = if kind == PropertyKind::File {
                "file"
            } else {
                "note.status"
            };
            for &operator in operators_for(kind) {
                let rule = Rule {
                    property: property.to_string(),
                    operator,
                    value: sample(operator.value_kind_for(kind)),
                };
                let statement = rule.statement();
                let back = parse_rule(&statement, &kinds(kind));
                assert_eq!(
                    back.as_ref(),
                    Some(&rule),
                    "{kind:?} {operator:?}: {statement}"
                );
            }
        }
    }

    #[test]
    fn a_fresh_row_writes_a_valid_statement_that_reads_back_as_itself() {
        for &kind in KINDS {
            for &operator in operators_for(kind) {
                let rule = Rule {
                    operator,
                    ..new_rule("file", kind)
                };
                let statement = rule.statement();
                assert!(
                    expr::parse(&statement).is_ok(),
                    "{kind:?} {operator:?}: {statement}"
                );
                let back = parse_rule(&statement, &kinds(kind)).expect(&statement);
                assert_eq!(back.property, "file");
                assert_eq!(back.operator, operator, "{statement}");
            }
        }
        assert_eq!(
            new_rule("rating", PropertyKind::Number).statement(),
            "rating == 0"
        );
        assert_eq!(
            new_rule("done", PropertyKind::Checkbox).statement(),
            "done == false"
        );
        assert_eq!(
            new_rule("status", PropertyKind::Text).statement(),
            "status == \"\""
        );
        assert_eq!(
            new_rule("file", PropertyKind::File).statement(),
            "file.hasLink(\"\")"
        );
        assert_eq!(
            new_rule("kinds", PropertyKind::List).statement(),
            "kinds == \"\""
        );
    }

    #[test]
    fn the_observed_pairs() {
        let pairs: &[(&str, Rule)] = &[
            (
                "file.hasTag(\"rock\")",
                Rule {
                    property: "file".into(),
                    operator: Operator::HasTag,
                    value: RuleValue::List(vec!["rock".into()]),
                },
            ),
            (
                "rating >= 1",
                Rule {
                    property: "rating".into(),
                    operator: Operator::Ge,
                    value: RuleValue::Number("1".into()),
                },
            ),
            (
                "status != \"archived\"",
                Rule {
                    property: "status".into(),
                    operator: Operator::IsNot,
                    value: RuleValue::Text("archived".into()),
                },
            ),
            (
                "done != \"nope\"",
                Rule {
                    property: "done".into(),
                    operator: Operator::CheckboxIsNot,
                    value: RuleValue::Text("nope".into()),
                },
            ),
            (
                "file.hasLink(\"Scoria\")",
                Rule {
                    property: "file".into(),
                    operator: Operator::LinksTo,
                    value: RuleValue::Text("Scoria".into()),
                },
            ),
        ];
        for (statement, rule) in pairs {
            assert_eq!(
                parse_rule(statement, &fixture).as_ref(),
                Some(rule),
                "{statement}"
            );
            assert_eq!(rule.statement(), *statement);
        }
    }

    #[test]
    fn the_observed_rows_label_as_the_panel_does() {
        let row = |statement: &str| {
            let rule = parse_rule(statement, &fixture).unwrap();
            (rule.property, rule.operator.label())
        };
        assert_eq!(row("file.hasTag(\"rock\")"), ("file".into(), "has tag"));
        assert_eq!(row("rating >= 1"), ("rating".into(), "≥"));
        assert_eq!(row("status != \"archived\""), ("status".into(), "is not"));
        assert_eq!(row("done != \"nope\""), ("done".into(), "is not"));
        assert_eq!(row("file.hasLink(\"x\")"), ("file".into(), "links to"));
    }

    #[test]
    fn the_operator_lists_in_combobox_order() {
        let labels = |kind| {
            operators_for(kind)
                .iter()
                .map(|op| op.label())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(PropertyKind::File),
            [
                "links to",
                "in folder",
                "has tag",
                "has property",
                "does not link to",
                "is not in folder",
                "does not have tag",
                "does not have property",
            ]
        );
        assert_eq!(
            labels(PropertyKind::Number),
            ["=", "≠", "<", "≤", ">", "≥", "is empty", "is not empty"]
        );
        assert_eq!(
            labels(PropertyKind::Text),
            [
                "is",
                "is not",
                "starts with",
                "ends with",
                "is empty",
                "contains",
                "contains any of",
                "contains all of",
                "does not start with",
                "does not end with",
                "is not empty",
                "does not contain",
                "does not contain any of",
                "does not contain all of",
            ]
        );
        assert_eq!(labels(PropertyKind::Checkbox), ["is", "is not"]);
        // The readings.
        assert_eq!(labels(PropertyKind::Date), labels(PropertyKind::Number));
        assert_eq!(labels(PropertyKind::DateTime), labels(PropertyKind::Number));
        for kind in [
            PropertyKind::List,
            PropertyKind::Tags,
            PropertyKind::Formula,
            PropertyKind::Unknown,
        ] {
            assert_eq!(labels(kind), labels(PropertyKind::Text), "{kind:?}");
        }
    }

    #[test]
    fn value_kinds_name_the_control() {
        assert_eq!(Operator::IsEmpty.value_kind(), ValueKind::None);
        assert_eq!(Operator::IsNotEmpty.value_kind(), ValueKind::None);
        assert_eq!(Operator::HasTag.value_kind(), ValueKind::List);
        assert_eq!(Operator::NotContainsAny.value_kind(), ValueKind::List);
        assert_eq!(Operator::ContainsAll.value_kind(), ValueKind::List);
        assert_eq!(Operator::LinksTo.value_kind(), ValueKind::Link);
        assert_eq!(Operator::NotInFolder.value_kind(), ValueKind::Folder);
        assert_eq!(Operator::HasProperty.value_kind(), ValueKind::PropertyName);
        assert_eq!(Operator::CheckboxIs.value_kind(), ValueKind::Checkbox);
        assert_eq!(Operator::CheckboxIsNot.value_kind(), ValueKind::Checkbox);
        assert_eq!(Operator::Is.value_kind(), ValueKind::Text);
        assert_eq!(Operator::Ge.value_kind(), ValueKind::Number);
        assert_eq!(
            Operator::Ge.value_kind_for(PropertyKind::Number),
            ValueKind::Number
        );
        assert_eq!(
            Operator::Ge.value_kind_for(PropertyKind::Date),
            ValueKind::Date
        );
        assert_eq!(
            Operator::Eq.value_kind_for(PropertyKind::DateTime),
            ValueKind::Date
        );
        assert_eq!(
            Operator::IsEmpty.value_kind_for(PropertyKind::Date),
            ValueKind::None
        );
    }

    #[test]
    fn statements_without_a_row_shape_are_advanced_filters() {
        for statement in [
            "type + \" \" + file.name",
            "rating * 2 > 3",
            "rating",
            "status",
            "file.hasTag(\"rock\") && rating >= 1",
            "1 <= rating",
            "rating >= due",
            "status == null",
            "!(rating >= 1)",
            "!status",
            "file.hasTag(rating)",
            "file.hasTag(1)",
            "file.hasLink(\"a\", \"b\")",
            "file.hastag(\"rock\")",
            "status.containsAny([\"a\", \"b\"])",
            "status.isEmpty(1)",
            "status.lower() == \"a\"",
            "status.startsWith(\"a\").isEmpty()",
            "file.name.length > 3",
            "density +",
        ] {
            assert_eq!(parse_rule(statement, &fixture), None, "{statement}");
        }
    }

    #[test]
    fn an_operator_the_combobox_does_not_offer_is_no_row() {
        // A text property has no `>`.
        assert_eq!(parse_rule("status > \"a\"", &fixture), None);
        assert_eq!(parse_rule("status <= 3", &fixture), None);
        // A number property has no "starts with", and compares against
        // numbers only.
        assert_eq!(parse_rule("rating.startsWith(\"1\")", &fixture), None);
        assert_eq!(parse_rule("rating >= \"1\"", &fixture), None);
        assert_eq!(parse_rule("rating == true", &fixture), None);
        // A checkbox has no `<`.
        assert_eq!(parse_rule("done < true", &fixture), None);
        // A formula column reads the text set: `formula.score > 3` is
        // advanced, `formula.score == 3` is a row.
        assert_eq!(parse_rule("formula.score > 3", &fixture), None);
        assert_eq!(
            parse_rule("formula.score == 3", &fixture),
            Some(Rule {
                property: "formula.score".into(),
                operator: Operator::Is,
                value: RuleValue::Number("3".into()),
            })
        );
        // `file` itself has no comparison and no text method.
        assert_eq!(parse_rule("file == \"a\"", &fixture), None);
        assert_eq!(parse_rule("file.startsWith(\"a\")", &fixture), None);
    }

    #[test]
    fn numbers_keep_their_spelling() {
        let number = |statement: &str| match parse_rule(statement, &fixture) {
            Some(Rule {
                value: RuleValue::Number(n),
                ..
            }) => n,
            other => panic!("{statement}: {other:?}"),
        };
        assert_eq!(number("rating >= 1.50"), "1.50");
        assert_eq!(number("rating>=1.50"), "1.50");
        assert_eq!(number("rating == -1"), "-1");
        assert_eq!(number("rating < 10."), "10.");
        assert_eq!(number("rating <= 007"), "007");
        assert_eq!(number("rating == .5"), ".5");
        assert_eq!(number("(rating >= 1.50)"), "1.50");
        assert_eq!(number("( (rating >= 1.50 ) )"), "1.50");
        // Where the spelling cannot be recovered the value's own is used.
        assert_eq!(number("rating >= - 1"), "-1");
        assert_eq!(
            parse_rule("rating >= 1.50", &fixture).unwrap().statement(),
            "rating >= 1.50"
        );
    }

    #[test]
    fn parentheses_are_looked_through_and_not_written() {
        let plain = parse_rule("file.hasTag(\"rock\")", &fixture).unwrap();
        assert_eq!(
            parse_rule("(file.hasTag(\"rock\"))", &fixture).unwrap(),
            plain
        );
        assert_eq!(
            parse_rule("((file.hasTag(\"rock\")))", &fixture).unwrap(),
            plain
        );
        let negated = parse_rule("!file.hasTag(\"rock\")", &fixture).unwrap();
        assert_eq!(negated.operator, Operator::NotHasTag);
        assert_eq!(
            parse_rule("!(file.hasTag(\"rock\"))", &fixture).unwrap(),
            negated
        );
        assert_eq!(
            parse_rule("(!(file.hasTag(\"rock\")))", &fixture).unwrap(),
            negated
        );
        assert_eq!(negated.statement(), "!file.hasTag(\"rock\")");
    }

    #[test]
    fn negated_calls_and_lists() {
        let rule = parse_rule("!status.startsWith(\"a\")", &fixture).unwrap();
        assert_eq!(rule.operator, Operator::NotStartsWith);
        assert_eq!(rule.statement(), "!status.startsWith(\"a\")");

        let rule = parse_rule("!status.isEmpty()", &fixture).unwrap();
        assert_eq!(rule.operator, Operator::IsNotEmpty);
        assert_eq!(rule.value, RuleValue::None);
        assert_eq!(rule.statement(), "!status.isEmpty()");

        let rule = parse_rule("kinds.containsAny(\"a\", \"b\")", &fixture).unwrap();
        assert_eq!(rule.property, "kinds");
        assert_eq!(rule.operator, Operator::ContainsAny);
        assert_eq!(rule.value, RuleValue::List(vec!["a".into(), "b".into()]));
        assert_eq!(rule.statement(), "kinds.containsAny(\"a\", \"b\")");

        let rule = parse_rule("file.hasTag(\"rock\", \"lava\")", &fixture).unwrap();
        assert_eq!(
            rule.value,
            RuleValue::List(vec!["rock".into(), "lava".into()])
        );
        assert_eq!(rule.statement(), "file.hasTag(\"rock\", \"lava\")");

        let rule = parse_rule("file.hasTag()", &fixture).unwrap();
        assert_eq!(rule.value, RuleValue::None);
        assert_eq!(rule.statement(), "file.hasTag()");

        let rule = parse_rule("!file.inFolder(\"Rocks\")", &fixture).unwrap();
        assert_eq!(rule.operator, Operator::NotInFolder);
        let rule = parse_rule("file.hasProperty(\"status\")", &fixture).unwrap();
        assert_eq!(rule.operator, Operator::HasProperty);
        assert_eq!(rule.value, RuleValue::Text("status".into()));
    }

    #[test]
    fn a_checkbox_row_keeps_whatever_it_compares_against() {
        let rule = parse_rule("done == true", &fixture).unwrap();
        assert_eq!(rule.operator, Operator::CheckboxIs);
        assert_eq!(rule.value, RuleValue::Bool(true));
        assert_eq!(rule.statement(), "done == true");
        let rule = parse_rule("done != false", &fixture).unwrap();
        assert_eq!(rule.operator, Operator::CheckboxIsNot);
        assert_eq!(rule.value, RuleValue::Bool(false));
        let rule = parse_rule("done == 1", &fixture).unwrap();
        assert_eq!(rule.value, RuleValue::Number("1".into()));
        assert_eq!(rule.statement(), "done == 1");
    }

    #[test]
    fn strings_escape_and_read_back() {
        let rule = Rule {
            property: "status".into(),
            operator: Operator::Is,
            value: RuleValue::Text("say \"hi\" \\ there\n".into()),
        };
        assert_eq!(
            rule.statement(),
            "status == \"say \\\"hi\\\" \\\\ there\\n\""
        );
        assert_eq!(parse_rule(&rule.statement(), &fixture).unwrap(), rule);
        let rule = parse_rule("status == \"a\\\"b\"", &fixture).unwrap();
        assert_eq!(rule.value, RuleValue::Text("a\"b".into()));
        assert_eq!(rule.statement(), "status == \"a\\\"b\"");
    }

    #[test]
    fn dotted_properties_keep_their_spelling() {
        let rule = parse_rule("note.rating >= 2", &fixture).unwrap();
        assert_eq!(rule.property, "note.rating");
        assert_eq!(rule.statement(), "note.rating >= 2");
        let rule = parse_rule("file.name.startsWith(\"2026\")", &fixture).unwrap();
        assert_eq!(rule.property, "file.name");
        assert_eq!(rule.operator, Operator::StartsWith);
        assert_eq!(rule.statement(), "file.name.startsWith(\"2026\")");
        // A date reads the number set with its value as text.
        let rule = parse_rule("due > \"2025-01-01\"", &fixture).unwrap();
        assert_eq!(rule.operator, Operator::Gt);
        assert_eq!(rule.value, RuleValue::Text("2025-01-01".into()));
        assert_eq!(rule.statement(), "due > \"2025-01-01\"");
    }
}
