---
title: The graph view
description: Scoria's graph of your vault — what the nodes and edges are, the filters and color groups it reads from graph.json, the force simulation, and how to move around in it.
---

# The graph view

`Ctrl + G` (`graph:open`) opens the graph; `Ctrl + G` again returns to the note you were
reading. `graph:open-local` opens the local graph of the open note instead.

## What is in it

| Node | When |
|---|---|
| A note | Always |
| An attachment | With `showAttachments` on |
| An unresolved target | With `hideUnresolved` off — a `[[Name]]` nobody has created |
| A tag | With `showTags` on |
| An orphan | With `showOrphans` on — a note with no links either way |

Edges are links, in either direction, including links in frontmatter.

Every one of those switches comes from your vault's `graph.json`, so the graph opens looking
the way you left it in Obsidian. So do the `search` filter — an arbitrary [search
query](/docs/reference/search-syntax/) restricting which notes appear — and your
**color groups**, each a query and a color, evaluated with the same grammar.

The display and force settings in `graph.json` are read too: node size, link thickness, text
fade threshold, centre force, repel force, link force and link distance, along with the saved
`scale`, and `showArrow` draws an arrowhead on each edge. Changes made in the graph's own panel
are written back to `graph.json` a moment after the last one, so Obsidian opens the graph as
you left it here too.

## Moving around

| | |
|---|---|
| Zoom | Mouse wheel |
| Pan | Drag the background |
| Move a node | Drag the node — which reheats the simulation around it |
| Open a note | Click its node |
| Refit the view | `Ctrl + 0` |

The view auto-fits the graph until the first time you zoom or pan, after which it leaves your
framing alone.

Labels appear once you are zoomed in past a threshold — and that threshold rises with the
number of nodes, so a 5,000-note graph does not try to paint 5,000 labels at once.

## The local graph

`graph:open-local` shows the open note and the notes one link away from it, in either
direction. Opening another note re-centres it on that note.

Its panel adds Obsidian's **Depth** slider (one to five links out) and the three direction
toggles at the top of Filters — Incoming links, Outgoing links and Neighbor links — opening at
Obsidian's defaults: depth one, both directions, no neighbor links. These live in the local
graph's own state, never in `graph.json`, and Scoria keeps them for the session.

> **Note** The local graph is a view of the main area, like the global one; Obsidian's local
> graph as a side pane beside the note is still ahead.

## How the layout works

A force simulation, stepped on a background thread:

- **Repulsion** between all nodes, computed through a quadtree (Barnes–Hut), so the cost is
  not quadratic in node count.
- **Springs** along the links, at the `linkDistance` from your settings.
- **A weak pull to the centre**, so disconnected clusters do not drift off screen.
- **Simulated annealing**: the simulation cools and stops rather than jiggling forever.
  Dragging a node reheats it.

Painting is a single stroked path for all the edges plus circles for the nodes, which is why
a large graph stays smooth.

## Not yet

- The local graph as a side pane.
- `graph:animate` as a command (the panel's timelapse button replays the layout from its first
  positions).
- Screenshot export.

## Next

- [Managing notes](/docs/app/manage/)
- [Internal links](/docs/format/links/)
