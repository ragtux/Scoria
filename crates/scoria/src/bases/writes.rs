//! What a control writes into a view: `sort`, `groupBy`, `order` and
//! `limit`.
//!
//! Two surfaces write these — the toolbar's sort, group, properties and
//! results panels, and the column header's own context menu — and they must
//! write the same bytes, so there is one implementation of each write here
//! rather than one per surface. The layout keys are the same idea one level
//! down, in [`scoria_bases::layout`].
//!
//! What is observed, from
//! [[Specification#Observed Bases write behavior, Obsidian 1.14.5 (installer build 1.13.7)]]:
//!
//! - *Sort Z → A* **prepends** `{property, direction: DESC}` to the view's
//!   `sort` list — it does not append and it does not replace the list, and
//!   it is the only insertion into `sort` the run ever pinned down;
//! - *Hide column* **removes the property from `order`**, and `order` is
//!   exactly the columns the view shows;
//! - the limit input writes `limit` as a bare number.
//!
//! **Nothing here qualifies an id.** The writer does not either — "a
//! hand-written bare `status` came back `status`" — so the caller passes the
//! id it means: the header menu passes the column's own id as `order`
//! carries it, and a panel passes what its picker produced, which is already
//! a qualified id (`note.status`, `file.name`, `formula.ppu`) because that is
//! how the picker's rows are built. The one observed picker that qualifies
//! for itself is the layout `image` key's, in [`scoria_bases::layout`].

use scoria_bases::table::qualified_id;
use scoria_formats::base::{GroupBy, Sort, View};

/// The two directions Obsidian writes, labelled "A → Z" and "Z → A". An
/// absent direction is ascending.
pub const ASC: &str = "ASC";
pub const DESC: &str = "DESC";

/// Whether two property ids name the same property.
///
/// `order`, `sort` and `groupBy` carry both spellings — a hand-written bare
/// `status` and the qualified `note.status` Obsidian writes — so every
/// comparison goes through the normalizer rather than over the bytes, and
/// case-insensitively, which is how the vault compares every other name.
pub fn same_property(a: &str, b: &str) -> bool {
    qualified_id(a).eq_ignore_ascii_case(&qualified_id(b))
}

// ---------------------------------------------------------------------------
// sort
// ---------------------------------------------------------------------------

/// Whether the view already sorts by `property`.
pub fn is_sorted(view: &View, property: &str) -> bool {
    view.sort
        .iter()
        .any(|s| same_property(&s.property, property))
}

/// Puts `{property, direction}` at the **head** of the view's `sort`.
///
/// **Observed** for *Sort Z → A*, which prepended `{property, direction:
/// DESC}`. *Sort A → Z* and the sort panel's own add row are the mirror and
/// prepend too, which is **unprobed**; nothing in the run inserted a sort any
/// other way.
///
/// **Unprobed**: whether Obsidian drops an entry the list already had for the
/// same property. It is dropped here, because otherwise sorting the same
/// column twice leaves a second entry that can never win and the file grows
/// on every click.
pub fn prepend_sort(view: &mut View, property: &str, direction: &str) {
    view.sort.retain(|s| !same_property(&s.property, property));
    view.sort.insert(
        0,
        Sort {
            property: property.to_string(),
            direction: Some(direction.to_string()),
        },
    );
}

/// *Clear sort*: drops this property's entries and leaves the rest of the
/// list alone, which is the reading that matches a menu belonging to one
/// column. **Unprobed** — the row's write was never read.
pub fn clear_sort(view: &mut View, property: &str) {
    view.sort.retain(|s| !same_property(&s.property, property));
}

/// The sort panel's `lucide-trash-2`: drops the `ix`th entry.
pub fn remove_sort(view: &mut View, ix: usize) {
    if ix < view.sort.len() {
        view.sort.remove(ix);
    }
}

/// The sort panel's property combobox.
pub fn set_sort_property(view: &mut View, ix: usize, property: &str) {
    if let Some(entry) = view.sort.get_mut(ix) {
        entry.property = property.to_string();
    }
}

/// The sort panel's direction combobox.
pub fn set_sort_direction(view: &mut View, ix: usize, direction: &str) {
    if let Some(entry) = view.sort.get_mut(ix) {
        entry.direction = Some(direction.to_string());
    }
}

// ---------------------------------------------------------------------------
// groupBy
// ---------------------------------------------------------------------------

/// `groupBy.property`. A view with no `groupBy` gets one in the ascending
/// direction, which is the direction an absent one means.
///
/// `groupOrder` is left exactly as it was found: whether the UI ever writes
/// or clears one is **unprobed**, so nothing here does.
pub fn set_group_property(view: &mut View, property: &str) {
    match view.group_by.as_mut() {
        Some(group) => group.property = property.to_string(),
        None => {
            view.group_by = Some(GroupBy {
                property: property.to_string(),
                direction: Some(ASC.to_string()),
            })
        }
    }
}

/// The group panel's direction combobox.
pub fn set_group_direction(view: &mut View, direction: &str) {
    if let Some(group) = view.group_by.as_mut() {
        group.direction = Some(direction.to_string());
    }
}

// ---------------------------------------------------------------------------
// order
// ---------------------------------------------------------------------------

/// *Hide column*, and the properties panel's unchecking.
///
/// **Observed**: it removes the property from `order`, and the properties
/// panel's *Hide all* writes `order: []` — an explicit empty list, which is
/// the view with no columns at all, where an absent `order` is the view with
/// every implied column. So a view with no `order` first materializes
/// `implied` without the hidden id, and hiding the last column leaves the
/// empty list rather than dropping the key.
pub fn hide_column(view: &mut View, id: &str, implied: &[String]) {
    let current = view.order.clone().unwrap_or_else(|| implied.to_vec());
    view.order = Some(
        current
            .into_iter()
            .filter(|other| !same_property(other, id))
            .collect(),
    );
}

/// Appends a property to `order`, which is what "Add *name* property" and
/// checking a box in the properties panel do. `shown` is the ids the view
/// currently resolves to: a view with no `order` names them first, or the
/// one id written would be the only column left.
pub fn add_property(view: &mut View, shown: &[String], id: &str) {
    scoria_bases::layout::add_column(view, shown, id);
}

/// Checks or unchecks one property of the properties panel.
///
/// Observed: `order` is exactly the shown columns, so checking appends the id
/// and unchecking removes it. `shown` is the ids the view currently resolves
/// to, which is what makes either work on a view with no `order`.
pub fn toggle_property(view: &mut View, shown: &[String], id: &str) {
    let in_order = view
        .order
        .as_ref()
        .is_some_and(|order| order.iter().any(|o| same_property(o, id)));
    let implied = view.order.is_none() && shown.iter().any(|s| same_property(s, id));
    if in_order || implied {
        hide_column(view, id, shown);
    } else {
        add_property(view, shown, id);
    }
}

// ---------------------------------------------------------------------------
// limit
// ---------------------------------------------------------------------------

/// What the "Limit number of results" input writes: a bare number, or no
/// `limit` key at all for an empty or unreadable field. *Show all (n)* is the
/// empty case.
pub fn parse_limit(text: &str) -> Option<u64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    text.parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoria_formats::base::Base;

    fn view_of(yaml: &str) -> View {
        Base::from_yaml(yaml).expect("parses").views.remove(0)
    }

    fn a_table() -> View {
        view_of("views:\n  - type: table\n    name: V\n")
    }

    #[test]
    fn a_sort_goes_to_the_head_of_a_list_that_already_had_one() {
        let mut view = view_of(
            "views:\n  - type: table\n    name: V\n    sort:\n      - property: file.name\n        direction: ASC\n",
        );
        prepend_sort(&mut view, "status", DESC);
        assert_eq!(view.sort[0].property, "status");
        assert_eq!(view.sort[0].direction.as_deref(), Some("DESC"));
        assert_eq!(view.sort[1].property, "file.name");
    }

    #[test]
    fn sorting_the_same_property_twice_leaves_one_entry() {
        let mut view = a_table();
        prepend_sort(&mut view, "status", ASC);
        prepend_sort(&mut view, "note.status", DESC);
        assert_eq!(view.sort.len(), 1);
        assert_eq!(view.sort[0].direction.as_deref(), Some("DESC"));
    }

    #[test]
    fn an_id_is_written_exactly_as_the_caller_spelled_it() {
        // The writer does not qualify a bare id, so neither does this.
        let mut view = a_table();
        prepend_sort(&mut view, "status", ASC);
        add_property(&mut view, &[], "status");
        set_group_property(&mut view, "status");
        assert_eq!(view.sort[0].property, "status");
        assert_eq!(view.order, Some(vec!["status".to_string()]));
        assert_eq!(view.group_by.as_ref().unwrap().property, "status");
    }

    #[test]
    fn clear_sort_drops_one_columns_entries_only() {
        let mut view = a_table();
        prepend_sort(&mut view, "file.name", ASC);
        prepend_sort(&mut view, "note.status", DESC);
        assert!(is_sorted(&view, "status"));
        clear_sort(&mut view, "status");
        assert!(!is_sorted(&view, "note.status"));
        assert!(is_sorted(&view, "file.name"));
    }

    #[test]
    fn a_new_group_is_ascending_and_keeps_a_group_order_it_found() {
        let mut view = view_of(
            "views:\n  - type: table\n    name: V\n    groupOrder:\n      - todo\n      - null\n",
        );
        set_group_property(&mut view, "note.status");
        assert_eq!(
            view.group_by.as_ref().unwrap().direction.as_deref(),
            Some("ASC")
        );
        assert_eq!(view.group_order.len(), 2, "groupOrder is never rewritten");
        set_group_direction(&mut view, DESC);
        assert_eq!(
            view.group_by.as_ref().unwrap().direction.as_deref(),
            Some("DESC")
        );
    }

    #[test]
    fn hide_column_drops_exactly_one_id_from_order() {
        let mut view = view_of(
            "views:\n  - type: table\n    name: V\n    order:\n      - file.name\n      - status\n",
        );
        hide_column(&mut view, "note.status", &[]);
        assert_eq!(view.order, Some(vec!["file.name".to_string()]));
    }

    #[test]
    fn hiding_a_column_of_an_implied_order_materializes_the_rest() {
        let mut view = a_table();
        let implied = vec!["note.status".to_string(), "note.density".to_string()];
        hide_column(&mut view, "status", &implied);
        assert_eq!(view.order, Some(vec!["note.density".to_string()]));
    }

    #[test]
    fn hiding_the_last_column_writes_an_empty_order() {
        // Observed for *Hide all*: `order: []`, the view with no columns,
        // which is not the absent key that means every implied column.
        let mut one = view_of("views:\n  - type: table\n    name: V\n    order:\n      - status\n");
        hide_column(&mut one, "status", &[]);
        assert_eq!(one.order, Some(Vec::new()));
        let mut implied = a_table();
        hide_column(&mut implied, "status", &["status".to_string()]);
        assert_eq!(implied.order, Some(Vec::new()));
        let mut panel = a_table();
        toggle_property(&mut panel, &["status".to_string()], "status");
        assert_eq!(panel.order, Some(Vec::new()));
    }

    #[test]
    fn a_checkbox_appends_and_removes_the_same_id() {
        let mut view = a_table();
        let shown = vec!["note.status".to_string()];
        toggle_property(&mut view, &shown, "note.density");
        assert_eq!(
            view.order,
            Some(vec!["note.status".to_string(), "note.density".to_string()]),
            "checking first materializes the implied order"
        );
        toggle_property(&mut view, &shown, "note.density");
        assert_eq!(view.order, Some(vec!["note.status".to_string()]));
    }

    #[test]
    fn a_bare_id_already_in_order_is_the_same_property_as_its_qualified_form() {
        let mut view =
            view_of("views:\n  - type: table\n    name: V\n    order:\n      - status\n");
        add_property(&mut view, &[], "note.status");
        assert_eq!(
            view.order.as_ref().map(Vec::len),
            Some(1),
            "it is already there"
        );
    }

    #[test]
    fn a_limit_is_a_bare_number_and_an_empty_field_is_none() {
        assert_eq!(parse_limit(" 103 "), Some(103));
        assert_eq!(parse_limit(""), None);
        assert_eq!(parse_limit("ten"), None);
    }

    #[test]
    fn the_trash_button_drops_one_entry_and_tolerates_a_stale_index() {
        let mut view = a_table();
        prepend_sort(&mut view, "status", ASC);
        remove_sort(&mut view, 0);
        assert!(view.sort.is_empty());
        remove_sort(&mut view, 0);
    }
}
