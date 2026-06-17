# Commonplace — v1 Build Plan

Scope: the four ideas raised for v1, in dependency order. Cross-checked against
existing tools (Obsidian, Smart Connections, the Zettelkasten community, iA
Writer, MCP prior art) — see "Research notes" at the end for what each decision
was validated against.

> **Build status (all phases implemented):**
> - **Phase 1** wikilinks / backlinks (`vault_links`) / quick-capture — done.
> - **Phase 2** Suggest links (`Ctrl+K`) — done, frontend-only.
> - **Phase 3** Knowledge map (`Ctrl+G`, vendored `force-graph` + `semantic_edges`,
>   both edge kinds; semantic threshold 0.7) — done.
> - **Phase 4** Folder-aware `Ctrl+P` + summoned tree (`Ctrl+\`) — done.
> - **Phase 5** Read-only MCP server (`src-tauri/src/bin/mcp.rs`, stdio JSON-RPC,
>   no SDK) — done; protocol + file tools smoke-tested; semantic tools verified
>   to degrade gracefully when the engine is down.
>
> Vendor build is reproducible via `npm run vendor`. UI changes need visual
> verification; MCP semantic tools need the running app to verify live.

Guiding constraints (from HANDOFF.md, do not violate):
- **UI stays "exactly like a text editor."** Minimalism is the product. New
  surfaces are summoned and auto-hide; nothing becomes persistent chrome.
- **Plain Markdown is the artifact.** No new lock-in. Links, tags, and IDs must
  be readable text in the `.md` files, portable to Obsidian/any editor.
- **Air-gapped.** Nothing added phones home. Anything network-facing binds to
  `127.0.0.1` and is opt-in.
- **Buildless frontend.** Stay a single `index.html` + vendored offline JS. No
  CDNs, no bundler step for app code.

Design decisions (confirmed against existing tools in the research pass):
- **D1 — Title-based links, not Luhmann numeric IDs.** `[[Note Title]]` resolves
  by filename. Confirmed: the Zettelkasten community itself concludes that in a
  digital system clickable links outweigh visible-sequence IDs. Visible "note
  sequences" are a deferred niche, not v1.
- **D2 — Reuse the existing embedding index for "connections."** Semantic
  neighbors already exist via `related_notes`; AI-linking and the semantic graph
  build on it. Confirmed: Smart Connections (the closest competitor) recommends
  nomic-embed-text 768-dim — exactly what we already run.
- **D3 — Vendor `force-graph` (vasturiano, MIT) for the map**, rendered on
  canvas, built on d3-force, vendored offline via the existing esbuild step.
  Revised from "hand-roll a force layout": the library is less code and gives
  zoom/pan/drag/click for free. WebGL libs (Sigma) are overkill at vault scale
  (hundreds of nodes, not 100k).

Dependency order: **1a wikilinks → 1b backlinks → 2 AI-linking → 3 map → 4 nav
→ 5 MCP.** Wikilinks are the atom the backlinks panel and the link-graph both
consume. AI-linking and MCP are independent of the graph/nav.

---

## Phase 1 — Zettelkasten core

### 1a. Wikilinks: `[[Note Title]]`

Goal: type `[[`, get autocomplete from the vault; click a rendered link to open
the target; unresolved links look distinct.

- **Frontend** ([app/src/index.html](app/src/index.html)):
  - A CodeMirror `autocompletion` extension that triggers on `[[` and completes
    from the in-memory `files` list (match on `name`/`rel`). Add `@codemirror/autocomplete`
    to the vendored bundle ([app/vendor-src/cm-entry.mjs](app/vendor-src/cm-entry.mjs)),
    re-run the esbuild vendor step.
  - A click handler (CM `EditorView.domEventHandlers`) that, on click inside a
    `[[...]]` span, resolves the title to a file and calls `openFile`. Resolve by
    exact `name` match first, then case-insensitive basename; ambiguous basenames
    (same name in two folders) resolve to the first match.
  - **Decision: unresolved link click = create-and-open** (Obsidian-style). If
    `[[X]]` resolves to no file, clicking creates `X.md` in the vault root via the
    existing create flow and opens it. Links double as a way to spawn notes.
  - Style resolved vs unresolved links via a small ViewPlugin decoration
    (unresolved = `--ink-faint`, dashed underline).
- **No new Rust** for 1a — resolution is client-side against `files`.

Verify:
- Type `[[ex` in a note with `existentialism.md` → suggestion appears; Enter
  inserts `[[existentialism]]`.
- Click a resolved `[[...]]` → the target opens.
- `[[Nonexistent]]` renders in the unresolved style and does not navigate.

Out of scope (1a): alias syntax `[[a|b]]`, heading/block links `[[a#h]]`,
embeds `![[a]]`. Note them; don't build them.

### 1b. Backlinks

Goal: for the open note, show "notes that link here," click to navigate.

- **Backend** ([app/src-tauri/src/index.rs](app/src-tauri/src/index.rs)):
  - **Decision: do NOT store links in LanceDB.** The `chunks` table is per-chunk
    and incremental-sync-coupled; threading links through it adds real
    complexity for no gain. Instead add one command `vault_links(dir) ->
    [{rel, name, links:[String]}]` that walks the markdown once and regex-extracts
    `[[...]]` targets. For a personal vault this is milliseconds. **This single
    command feeds both backlinks (Phase 1b) and the graph's explicit edges
    (Phase 3).**
  - A unit test for the link-extraction regex (matches `[[a]]`, ignores fenced
    code — keep the rule simple and documented).
- **Frontend**: derive backlinks by reverse-looking-up `vault_links` (notes whose
  `links` contain the open note's title).
- **Frontend**: a collapsed footer strip under the editor ("3 linked
  references") that expands into the existing drawer, reusing the `.rel` row
  style from `findRelated`. Refresh on note switch.

Verify:
- Note B contains `[[A]]` → opening A lists B in backlinks; clicking B opens it.
- Deleting B's link and reopening A drops it from the list after reindex.
- Link regex unit test passes.

### 1c. Quick capture (fleeting notes) — small

Goal: ⌘⇧N drops a timestamped note into `inbox/` and opens it, from anywhere.

- **Backend change required (verified).** `create_note`
  ([app/src-tauri/src/lib.rs:142](app/src-tauri/src/lib.rs:142)) today strips
  any directory via `.file_name()` and requires the parent to equal the vault
  root exactly — so it cannot make `inbox/note.md`. The minimal change: accept a
  one-level subdir, `create_dir_all` it, and swap the strict "parent == root"
  guard for a confine-style "canonical parent starts_with root" check. The
  existing traversal test (`../../escape` → `escape.md` in root,
  [lib.rs:409](app/src-tauri/src/lib.rs:409)) must still pass; add a test that a
  legit `inbox/x` is created and a `../x` escape is still refused.
- Frontend: **zero-friction (decision: user)** — ⌘⇧N auto-names
  `inbox/<date-time>.md` with no prompt and drops the caret in immediately.

Verify: ⌘⇧N from anywhere creates `inbox/2026-06-16-1432.md` (folder made if
absent) and opens it, caret ready, no dialog; traversal still refused; existing
+ new tests green.

Out of scope for Phase 1: `#tags` and a tag browser. The embedding search
already covers "stumble upon by meaning" (Ahrens' actual goal); add tags only if
the research pass shows they pull real weight. **Flag, don't build.**

---

## Phase 2 — AI-assisted connections

Goal: a "Suggest links" ⌘K action — semantic neighbors first, with an optional
one-line LLM rationale per candidate + one-click insertion of a `[[link]]`.

Emphasis (from research): the *similarity-ranked neighbor list is the product*
— it's the proven core of Smart Connections. The LLM rationale is a secondary
enhancement layered on top, because it leans on the small model's quality. Build
the list first; it must be fully useful with no prose at all.

- **Frontend only**, composing existing pieces:
  - Call `related_notes` (already exists) for the open note → ranked candidates.
    Render as `.rel` rows in the drawer with an "insert link" affordance that
    appends `[[title]]` at the caret. **This alone is the shippable feature.**
  - *Then* one `chatStream` call that, given the current note + candidate
    snippets, adds a one-sentence "why this connects" under each row. If the
    model underwhelms, this layer is dropped with zero loss to the core.
- No new Rust.

Verify:
- Neighbor list (no prose) runs on a substantive note → 3–6 ranked candidates;
  clicking "insert" places a working `[[link]]` that 1a can resolve/navigate.
- With an empty vault → graceful "no connections yet."
- Rationale layer, when on, produces a plausible one-liner per row and degrades
  cleanly to the bare list on model error.

---

## Phase 3 — Knowledge map

Goal: a summoned graph view; nodes = notes, edges = `[[links]]` (explicit) with an
optional toggle for semantic edges (cosine ≥ threshold).

- **Both edge kinds in v1** (decision: user). Explicit `[[link]]` edges come
  from `vault_links` (Phase 1b) — nearly free. Semantic edges come from a new
  command `semantic_edges(dir, k, threshold) -> [{s, d, score}]`: read all stored
  vectors for the vault from the `chunks` table, mean-pool per note into one
  vector, compute each note's top-k nearest above a cosine threshold, dedup into
  undirected edges. Cost is milliseconds for a personal vault (~80–100 lines of
  Rust + a test). Use **top-k-per-node + a floor threshold**, not a global
  threshold (nomic embeddings have a high similarity baseline).
- Frontend feeds `force-graph` both edge sets; a toggle shows/hides semantic
  edges (rendered lighter than explicit links).
- **Frontend**: a full-screen overlay (summoned by a ⌘K action and/or a key),
  rendered with vendored **`force-graph`** (canvas, MIT) — added to
  [app/vendor-src](app/vendor-src) and built into `vendor/` via the existing
  esbuild step, same as CodeMirror. Feed it `{nodes, links}`; it handles layout,
  zoom/pan/drag, and hover. Click a node → `openFile` and close. Esc closes. No
  persistent chrome. This shares the summoned-overlay pattern with Phase 4B (the
  tree) — both are "lenses" over the vault, opened on demand, gone on Esc.

Verify:
- Sample vault renders a connected graph; isolated notes show as loose nodes.
- Toggling semantic edges adds edges between unlinked-but-related notes.
- Clicking a node opens it; Esc returns to the editor.
- Layout stays interactive (no jank) on the sample vault scale.

Out of scope: zoom/pan polish, edge labels, saved layouts, time animation.
Ship the minimal readable graph first.

---

## Phase 4 — Directory viewing (both 4A and 4B)

Decision (user): do **both** — folder-aware ⌘P for fast jumping, and a summoned
tree view for browsing structure. Research clarified the constraint: iA Writer
(our reference) keeps the writing surface clear of chrome but *does* offer a
separate Library view. So neither piece is persistent chrome over the text; both
are summoned and dismissable. The tree shares the overlay pattern with the
Phase 3 map.

- **4A — folder-aware ⌘P (small).** Group the quick-open rows by top-level
  folder and show the folder in the hint. `list_markdown` already returns
  subfolder-aware `rel` paths — presentation-only in `renderOpenRows`, no
  backend change.
- **4B — summoned tree overlay.** A left overlay (summoned by a key, e.g. ⌘\;
  not docked chrome) showing a collapsible folder/file tree built from the `rel`
  paths in `files`. Click a file → `openFile` and close. Esc closes. Reuses the
  `.overlay` mechanics already in the app; the only new logic is grouping flat
  `rel` paths into a nested tree for rendering. No backend change — the data is
  already there.

Verify:
- 4A: with notes in subfolders, ⌘P groups them by folder; selecting opens the
  right file.
- 4B: the summon key opens a tree mirroring the on-disk folder structure;
  expand/collapse works; clicking a leaf opens it and closes the overlay; Esc
  closes without changing the open note.

---

## Phase 5 — MCP / API hooks (read-only for v1)

Goal: let *local* AI agents (Claude Desktop/Code) consult the vault as a tool,
without breaking the air-gap. **Read-only for v1** (decision: user) — no write
tools yet.

Architecture (settled in research):
- **A stdio MCP binary in the same Cargo workspace.** stdio is the dominant
  pattern for local notes MCP servers (runs as a subprocess of the client, no
  hosting, no open port). Build on the official `modelcontextprotocol/rust-sdk`,
  consistent with our Rust stack.
- **Data path:** the binary opens the existing **LanceDB table read-only**
  (concurrent reads are safe, and read-only sidesteps the single-writer
  constraint entirely) and, for semantic tools, calls the app's already-running
  embed server on `127.0.0.1:11501` to embed the query. So semantic search
  requires the Commonplace app (engine) to be running — the same "companion app
  must be up" pattern Obsidian REST-API MCP servers use.
- **Tools (read-only):** `search` (semantic — needs the app running),
  `read_note`, `list_notes`, `related_notes`. No `write`/`create`/`delete` in
  v1.
- **Safety:** read-only; stdio (no network surface); the embed call is
  localhost-only; document that enabling it lets a local agent read note
  contents. Air-gap is unchanged when the server isn't configured into a client.

Verify:
- From Claude Desktop/Code, the server lists exactly the read-only tools; a
  `search` call returns real vault hits; `read_note` returns a note's text.
- With the app closed, `read_note`/`list_notes` still work; `search` reports a
  clear "start Commonplace to enable semantic search" rather than hanging.
- No write tool is exposed.

Deferred to a later version: opt-in write tools (`create`/`append`/`edit`),
behind an explicit toggle.

---

## Cross-cutting verification

- `cargo test` (existing unit + lancedb smoke) stays green after each phase.
- esbuild vendor step succeeds (validates CM API usage) after any frontend dep
  change.
- Manual click-through on the installed-style release for each phase (per the
  project's verification habit).

## Explicit non-goals (simplicity guard)

Not in v1: link aliases/embeds/block-refs, a tag system, time-animated graphs,
saved graph layouts, multi-vault, sync, mobile, model switching, remote MCP,
MCP write tools, visible Luhmann/folgezettel note sequences.

---

## Research notes (what each decision was checked against)

- **D1 (title links, no IDs).** Zettelkasten community consensus: in digital
  systems clickable links outweigh visible-sequence IDs; folgezettel is a paper
  artifact. Sources: zettelkasten.de forum (Luhmann-IDs thread), "No, Luhmann
  Was Not About Folgezettel."
- **Wikilinks/backlinks (Phase 1).** Mirrors Obsidian: a metadata cache (reverse
  index) rebuilt on change; wikilinks tracked identically to md links; `[[`
  autocomplete via CodeMirror 6 `@codemirror/autocomplete`. Our plan matches a
  shipped implementation.
- **AI-linking (Phase 2).** Closest competitor is Smart Connections (local
  embeddings, connections list, drag-to-link). It validates the feature *and*
  our model choice — it recommends nomic-embed-text 768-dim, which we already
  run. Lesson applied: lead with the similarity list, treat LLM prose as an
  optional layer.
- **Graph (Phase 3, D3).** Library comparison: Sigma/WebGL is for 100k+ nodes
  (overkill); `force-graph` (canvas, d3-force, MIT) is right-sized and less code
  than hand-rolling. Revised the plan accordingly.
- **Directory viewing (Phase 4).** iA Writer keeps the *writing surface* clear
  but offers a separate Library/Organizer — so summoned navigation views don't
  violate the minimalism principle. Justifies doing both 4A and 4B as summoned,
  dismissable lenses.
- **MCP (Phase 5).** Many Obsidian MCP servers exist; stdio-subprocess is the
  dominant local pattern, and an official Rust SDK exists. Key constraint found:
  semantic tools need the embed model running, so the server opens LanceDB
  read-only and calls the app's local embed server — the "companion app must be
  up" pattern.
