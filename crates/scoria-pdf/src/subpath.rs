//! The subpath of a PDF link: `[[x.pdf#page=3&height=400]]`, and the
//! `selection=a,b,c,d` of one Obsidian copied from a selection.

/// A `selection=` range: from character `start_offset` of text item
/// `start_item` to character `end_offset` of item `end_item`, on the link's
/// page, items numbered as [`crate::TextItem`]s are and offsets in UTF-16
/// units (observed: `selection=8,0,8,17` is all of `This is page one.`, the
/// ninth item of the page).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub start_item: usize,
    pub start_offset: usize,
    pub end_item: usize,
    pub end_offset: usize,
}

impl Selection {
    /// `a,b,c,d`; anything else (too few numbers, a sign, a letter) is `None`.
    pub fn parse(value: &str) -> Option<Selection> {
        let mut parts = value.split(',').map(|p| p.trim().parse::<usize>());
        let mut next = || parts.next()?.ok();
        let selection = Selection {
            start_item: next()?,
            start_offset: next()?,
            end_item: next()?,
            end_offset: next()?,
        };
        Some(selection)
    }

    /// The value a link writes: `a,b,c,d`.
    pub fn to_value(self) -> String {
        format!(
            "{},{},{},{}",
            self.start_item, self.start_offset, self.end_item, self.end_offset
        )
    }
}

/// A parsed PDF link subpath. `page` is zero-based here: a link's `page=3`
/// opens the third page as a human counts, so it parses to `Some(2)`. `rest`
/// keeps every other `key=value` pair in order, undecoded.
#[derive(Debug, Clone, PartialEq)]
pub struct Subpath {
    pub page: Option<usize>,
    pub height: Option<f32>,
    pub selection: Option<Selection>,
    pub rest: Vec<(String, String)>,
}

/// Parse `"#page=3&height=400"`, `"page=3"`, `""` and the like. A leading `#` is
/// optional. A `page` that is not a positive integer is `None`; `height` parses as
/// a finite float, anything else is `None`. A repeated key wins last; a pair
/// without `=` has an empty value. Percent-encoding is left as written.
pub fn parse_subpath(subpath: &str) -> Subpath {
    let body = subpath.strip_prefix('#').unwrap_or(subpath);
    let mut parsed = Subpath {
        page: None,
        height: None,
        selection: None,
        rest: Vec::new(),
    };
    for pair in body.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "page" => {
                parsed.page = value
                    .parse::<usize>()
                    .ok()
                    .filter(|&page| page >= 1)
                    .map(|page| page - 1);
            }
            "height" => {
                parsed.height = value.parse::<f32>().ok().filter(|h| h.is_finite());
            }
            "selection" => parsed.selection = Selection::parse(value),
            _ => parsed.rest.push((key.to_string(), value.to_string())),
        }
    }
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rest(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn page_and_height() {
        let s = parse_subpath("#page=3&height=400");
        assert_eq!(s.page, Some(2));
        assert_eq!(s.height, Some(400.0));
        assert!(s.rest.is_empty());
    }

    #[test]
    fn hash_is_optional() {
        assert_eq!(parse_subpath("page=3").page, Some(2));
        assert_eq!(parse_subpath("#page=3").page, Some(2));
    }

    #[test]
    fn height_alone() {
        let s = parse_subpath("height=400");
        assert_eq!(s.page, None);
        assert_eq!(s.height, Some(400.0));
        assert_eq!(parse_subpath("height=12.5").height, Some(12.5));
    }

    #[test]
    fn page_must_be_a_positive_integer() {
        assert_eq!(parse_subpath("page=0").page, None);
        assert_eq!(parse_subpath("page=-1").page, None);
        assert_eq!(parse_subpath("page=abc").page, None);
        assert_eq!(parse_subpath("page=1").page, Some(0));
        assert_eq!(parse_subpath("page=").page, None);
        assert_eq!(parse_subpath("page").page, None);
    }

    #[test]
    fn height_must_be_a_number() {
        assert_eq!(parse_subpath("height=abc").height, None);
        assert_eq!(parse_subpath("height=").height, None);
        assert_eq!(parse_subpath("height=inf").height, None);
    }

    #[test]
    fn other_keys_go_to_rest_in_order() {
        let s = parse_subpath("#foo=bar&page=2&baz=1");
        assert_eq!(s.page, Some(1));
        assert_eq!(s.rest, rest(&[("foo", "bar"), ("baz", "1")]));
    }

    #[test]
    fn empty_inputs() {
        let empty = Subpath {
            page: None,
            height: None,
            selection: None,
            rest: vec![],
        };
        assert_eq!(parse_subpath(""), empty);
        assert_eq!(parse_subpath("#"), empty);
        assert_eq!(parse_subpath("&&"), empty);
    }

    #[test]
    fn duplicate_keys_last_wins() {
        assert_eq!(parse_subpath("page=2&page=5").page, Some(4));
        assert_eq!(parse_subpath("page=2&page=abc").page, None);
        assert_eq!(parse_subpath("height=1&height=2").height, Some(2.0));
        let s = parse_subpath("a=1&a=2");
        assert_eq!(s.rest, rest(&[("a", "1"), ("a", "2")]));
    }

    #[test]
    fn percent_encoding_is_left_as_is() {
        let s = parse_subpath("#search=hello%20world&page=4");
        assert_eq!(s.rest, rest(&[("search", "hello%20world")]));
        assert_eq!(s.page, Some(3));
    }

    #[test]
    fn value_may_contain_equals() {
        let s = parse_subpath("q=a=b");
        assert_eq!(s.rest, rest(&[("q", "a=b")]));
    }

    #[test]
    fn a_selection_is_four_numbers() {
        let s = parse_subpath("#page=1&selection=8,0,8,17");
        assert_eq!(s.page, Some(0));
        let sel = s.selection.unwrap();
        assert_eq!(
            (
                sel.start_item,
                sel.start_offset,
                sel.end_item,
                sel.end_offset
            ),
            (8, 0, 8, 17)
        );
        assert_eq!(sel.to_value(), "8,0,8,17");
        assert!(s.rest.is_empty());
        assert_eq!(parse_subpath("selection=1,2,3").selection, None);
        assert_eq!(parse_subpath("selection=1,2,3,x").selection, None);
        assert_eq!(parse_subpath("selection=-1,2,3,4").selection, None);
        // Observed: `&color=red` is kept but changes nothing.
        let c = parse_subpath("page=1&selection=8,0,8,17&color=red");
        assert!(c.selection.is_some());
        assert_eq!(c.rest, rest(&[("color", "red")]));
    }

    #[test]
    fn keys_are_case_sensitive() {
        let s = parse_subpath("Page=3");
        assert_eq!(s.page, None);
        assert_eq!(s.rest, rest(&[("Page", "3")]));
    }
}
