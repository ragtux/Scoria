//! The graph timelapse, `graph:animate` ("Start graph timelapse
//! animation"): the graph empties and its nodes come back a step at a time.
//!
//! What was observed ([[Specification#Observed graph timelapse, Obsidian
//! 1.15.0 (installer build 1.13.7)]]): the files come back in the order of
//! their creation time, one a step, whatever their names or modification
//! times; the gaps between the creation times do not show, so the clock is
//! the order alone. An unresolved node came in together with the node after
//! its linker. Edges are drawn between nodes already shown.
//!
//! What was not: Obsidian's steps are irregular (0.2 to 0.6 s) and repeat
//! run to run, and nothing observed explains them. Scoria's step is one
//! fixed length, [`STEP`], the mean of the observed intervals.

use std::time::Duration;

/// Scoria's step: 0.39 s, the mean interval of the two observed runs (seven
/// intervals in 2.83 s, twelve in 4.63 s). Obsidian's own vary from about
/// 0.2 to 0.6 s.
pub const STEP: Duration = Duration::from_millis(390);

/// The step each node comes back at, and how many steps there are.
///
/// `ctimes[i]` is node `i`'s file creation time, `None` for a node that is
/// not a file (an unresolved link or a tag); `ids` break a tie between
/// equal times, so a run is the same each time. `edges` are `(from, to)`:
/// `from` is the file whose link makes the edge.
///
/// Files take one step each in creation order. A node that is not a file
/// comes in one step after the first of its linkers to be shown, on that
/// later file's step, or on a step of its own past the last file's when
/// the linker is the last. One no file links to comes in on the last step.
pub fn schedule(ctimes: &[Option<u64>], ids: &[&str], edges: &[(u32, u32)]) -> (Vec<usize>, usize) {
    let mut files: Vec<usize> = (0..ctimes.len()).filter(|&i| ctimes[i].is_some()).collect();
    files.sort_by(|&a, &b| ctimes[a].cmp(&ctimes[b]).then_with(|| ids[a].cmp(ids[b])));
    let mut steps = vec![usize::MAX; ctimes.len()];
    for (step, &i) in files.iter().enumerate() {
        steps[i] = step;
    }
    let mut total = files.len();
    let mut linked = vec![usize::MAX; ctimes.len()];
    for &(from, to) in edges {
        let (from, to) = (from as usize, to as usize);
        for (node, other) in [(to, from), (from, to)] {
            if ctimes[node].is_none() && ctimes[other].is_some() {
                linked[node] = linked[node].min(steps[other]);
            }
        }
    }
    let mut orphans = Vec::new();
    for i in 0..ctimes.len() {
        if ctimes[i].is_some() {
            continue;
        }
        if linked[i] == usize::MAX {
            orphans.push(i);
        } else {
            steps[i] = linked[i] + 1;
            total = total.max(steps[i] + 1);
        }
    }
    for i in orphans {
        steps[i] = total.saturating_sub(1);
        total = total.max(1);
    }
    (steps, total)
}

/// How many steps are on screen `elapsed` into a run: the first at once,
/// then one more each [`STEP`].
pub fn shown_at(elapsed: Duration) -> usize {
    (elapsed.as_millis() / STEP.as_millis()) as usize + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The probe's fixture: eight notes made a second apart in the order
    /// Mango, Apple, Zebra, Kiwi, Banana, Orphan, Cherry, Lemon, and Kiwi's
    /// link to the unresolved Missing.
    #[test]
    fn files_come_back_in_creation_order_and_missing_with_the_note_after_its_linker() {
        let ids = [
            "Apple.md",
            "Banana.md",
            "Cherry.md",
            "Kiwi.md",
            "Lemon.md",
            "Mango.md",
            "Orphan.md",
            "Zebra.md",
            "Missing",
        ];
        let made = [
            "Mango.md",
            "Apple.md",
            "Zebra.md",
            "Kiwi.md",
            "Banana.md",
            "Orphan.md",
            "Cherry.md",
            "Lemon.md",
        ];
        let ctimes: Vec<Option<u64>> = ids
            .iter()
            .map(|id| {
                made.iter()
                    .position(|m| m == id)
                    .map(|p| 1000 + p as u64 * 1100)
            })
            .collect();
        let edges = [(3, 8), (5, 0), (0, 1)];
        let (steps, total) = schedule(&ctimes, &ids, &edges);
        assert_eq!(total, 8);
        let order: Vec<&str> = (0..total)
            .map(|s| ids[steps.iter().position(|&x| x == s).unwrap()])
            .collect();
        assert_eq!(
            order,
            [
                "Mango.md",
                "Apple.md",
                "Zebra.md",
                "Kiwi.md",
                "Banana.md",
                "Orphan.md",
                "Cherry.md",
                "Lemon.md"
            ]
        );
        // Kiwi is step 3; Missing comes in with Banana, step 4.
        assert_eq!(steps[8], 4);
        assert_eq!(steps[1], 4);
    }

    #[test]
    fn the_gaps_between_creation_times_do_not_show() {
        let ids = ["a", "b", "c"];
        let (steps, total) = schedule(&[Some(0), Some(500), Some(6_500)], &ids, &[]);
        assert_eq!((steps, total), (vec![0, 1, 2], 3));
    }

    #[test]
    fn equal_times_fall_back_to_the_path() {
        let ids = ["b", "a"];
        let (steps, _) = schedule(&[Some(7), Some(7)], &ids, &[]);
        assert_eq!(steps, vec![1, 0]);
    }

    #[test]
    fn a_node_linked_from_the_last_file_takes_a_step_of_its_own() {
        let ids = ["a", "b", "x"];
        let (steps, total) = schedule(&[Some(1), Some(2), None], &ids, &[(1, 2)]);
        assert_eq!(steps, vec![0, 1, 2]);
        assert_eq!(total, 3);
    }

    #[test]
    fn a_node_linked_from_two_files_follows_the_first_shown() {
        let ids = ["a", "b", "c", "x"];
        let (steps, _) = schedule(&[Some(1), Some(2), Some(3), None], &ids, &[(2, 3), (0, 3)]);
        assert_eq!(steps[3], 1);
    }

    #[test]
    fn the_first_step_is_there_at_once_and_one_more_comes_each_step() {
        assert_eq!(shown_at(Duration::ZERO), 1);
        assert_eq!(shown_at(STEP - Duration::from_millis(1)), 1);
        assert_eq!(shown_at(STEP), 2);
        assert_eq!(shown_at(STEP * 7), 8);
    }
}
