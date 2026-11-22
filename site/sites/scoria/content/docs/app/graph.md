---
title: The graph view
description: Scoria's graph of your vault — what the nodes and edges are, the filters and color groups it reads from graph.json, the force simulation, and how to move around in it.
---

# The graph view

`Ctrl + G` (`graph:open`) opens the graph; `Ctrl + G` again returns to the note you were
reading. `graph:open-local` ("Open local graph") opens a local graph of the open file in a new
pane to its right.

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

**Timelapse.** "Start graph timelapse animation" (`graph:animate`, or the wand under the cog
while the panel is closed) empties the global graph and brings the notes back one at a
step in the order they were created, an unresolved link arriving with the note after the one
that made it. Scoria's steps are a fixed 0.39 s; Obsidian's vary between about 0.2 and 0.6 s.

## The local graph

`graph:open-local` splits the main area: a new pane to the right of the note holds a tab
**Graph of** *the note's name* (the graph's fork icon), and that tab is linked to the note's.
It shows the note and the notes one link away from it, in either direction, and follows the
note's tab: open another note, a canvas or an image there and it re-centres on that, while
other tabs leave it alone. Run the command again and you get another linked pane. Close the
note's tab and the graph is unlinked — its tab reads **Local graph** — and from then on
follows whatever file is open. Click a node to open that note in the linked tab; `Ctrl` +
click opens it in a new tab beside the graph. The graph's tab menu has [Link with
tab... and Unlink tab](/docs/app/window/#linked-tabs): link an unlinked graph with a note's tab
and it re-centres on that note at once.

A vault Obsidian has laid out may also have a **Local graph** tab in the right sidebar (in
Obsidian you drag the graph's tab there). Scoria shows it, at the end of the sidebar's tabs,
following whatever file is open; right-click its tab and choose **Close** to take it away.

Each local graph's panel adds Obsidian's **Depth** slider (one to five links out) and the three
direction toggles at the top of Filters — Incoming links, Outgoing links and Neighbor links —
opening at Obsidian's defaults: depth one, both directions, no neighbor links. Everything you set
in a local graph's panel, the global graph's settings included, belongs to that graph: it is saved
with it in `workspace.json`, as Obsidian saves it, never in `graph.json`, so it reopens as you left
it, in Scoria and in Obsidian. A local graph opens at its own zoom of 1, zoomed out further if the
graph would not fit.

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

- Screenshot export.

## Next

- [Managing notes](/docs/app/manage/)
- [Internal links](/docs/format/links/)
