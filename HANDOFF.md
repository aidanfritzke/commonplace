# Commonplace — Project Handoff

A self-contained brief so a fresh session can pick up the build with no back-reference.
Working title: **Commonplace**. Two starting files live in this directory: `index.html`
(the working v0) and `README.md` (end-user run steps). Don't rebuild them — extend them.

---

## 1. What we're building

A **digital commonplace book**: a personal note repository with a local LLM layered on
top, **fully air-gapped** (runs offline; no data leaves the machine). The interface must
feel **exactly like a text editor** (think Notepad++ / iA Writer), with the intelligence
ambient and summoned on demand — not a chat app, not a busy dashboard. For the user's own
use now; a **distributable product** later.

---

## 2. Decisions locked — do not re-litigate

- **Build from the ground up.** Not assembling existing tools.
- **Bundle a small model inside the installer** → fully offline the moment it launches
  (accepts a ~2–5 GB installer; weights dominate, the shell is tiny).
- **UI = exactly like a text editor.** Minimalism *is* the product. AI lives behind ⌘K
  and stays out of the way otherwise. Resist feature creep.
- **Rejected:** AnythingLLM (too much capability), Blinko (GUI too busy).
  **Aesthetic references:** iA Writer / Typora (refined minimal, focus mode).
- **Stack** is settled (next section).

---

## 3. Stack

- **Shell:** Tauri 2 (Rust core + web frontend, native webview). *Why:* lightest path to
  a native, cross-platform, single-installer app; far smaller than Electron.
- **Inference:** llama.cpp server bundled as a Tauri **sidecar** (per-OS binary).
  *Dev shortcut (current v0):* talk to **Ollama** on `localhost`.
- **Notes:** plain **Markdown files** in a user-picked folder (the "vault"). Plain text is
  the artifact — future-proof, inspectable. Never lock notes in a DB.
- **Index:** local embedding model (`nomic-embed-text`) → **in-process vector store**.
  Default **LanceDB** (Rust-native, file-based, desktop-oriented, crash-stable). Alt:
  **sqlite-vec** if we want one SQLite file + easy keyword+vector hybrid.
- **Editor component:** **CodeMirror 6** (target). v0 currently uses a styled `<textarea>`.
- **Default model:** **Phi-4-mini** or a small **Qwen3** (CPU-friendly).

---

## 4. UX / interaction model

- **Default screen is just the text** — no sidebar, no persistent chat pane.
- **⌘K command palette:** Ask about your notes (RAG, cites filenames) · Find related notes
  (by meaning) · Summarize this note · Continue writing.
- **⌘P** quick file switcher (replaces a sidebar). **⌘O** open folder. **⌘S** save.
  **⌘.** focus mode. **`/`** on an empty line → inline writing. **Esc** closes panels.
- **Aesthetic:** warm "paper + ink." Writing surface = Newsreader (serif); chrome/palette =
  JetBrains Mono; single oxblood accent; light + warm-dark themes. Centered ~720px column.
- A mockup of this was approved earlier: clean canvas + bottom status bar
  (`offline · <model>`) + a ⌘K palette overlay listing the actions above.

---

## 5. Roadmap

- **v0 — DONE.** Delivered as a single-file web app (`index.html`): editor + vault + the
  Ollama RAG loop (ask / find related / summarize / continue). Runs in a Chromium browser
  served over `http://localhost`, against local Ollama.
- **NEXT — productionize the shell into the real native app.** Wrap v0 in Tauri 2
  (steps in §8). This is the immediate work, *not* v1.
- **v1 — PARKED (future avenue). Zettelkasten / "How to Take Smart Notes" (Ahrens / Luhmann
  slip-box):** atomic, interconnected **permanent notes** with explicit links/backlinks;
  **bottom-up writing** where notes accumulate into essays; AI-assisted linking; notes as
  the engine of thinking and writing rather than passive storage. v0's cited, grounded
  answers already lean this direction. **Do not start v1 until the native shell is solid.**

---

## 6. Current code state (`index.html`)

Single self-contained HTML file. JS is organized into labeled sections:

- **CONFIG** — `OLLAMA` base URL, `CHAT_MODEL = "llama3.2"` (placeholder default),
  `EMBED_MODEL = "nomic-embed-text"`.
- **state** — vault handle, file list, current file, dirty flag, in-memory `index`.
- **`vault`** — File I/O via the browser File System Access API. **This is the seam:**
  to go native, replace this one object with Tauri's fs + dialog APIs; nothing else changes.
- **ollama** — `embed()` (`POST /api/embeddings`) and `chatStream()` (`POST /api/chat`,
  NDJSON streaming).
- **RAG** — paragraph chunking (≤900 chars), cosine similarity, in-memory `index`,
  `retrieve(query, k=5)`.
- **editor / overlays / drawer** — textarea wiring; quick-open (`#ov-open`) and AI palette
  (`#ov-ask`); a right-side result drawer; actions `ask` / `findRelated` / `summarize` /
  `continueWriting`; minimal inline `/` (`slashCheck`); keybindings + theme/focus toggles.

Runs only in Chromium (Chrome/Edge/Brave), served over localhost, with
`OLLAMA_ORIGINS=* ollama serve` and the two models pulled.

---

## 7. Known limitations / TODO (fold into the build)

- `<textarea>` → **CodeMirror 6** (syntax highlighting, paragraph-dimming focus mode,
  caret-pinned `/` menu).
- In-memory index → **LanceDB** (or sqlite-vec) **persistence** so it survives restarts.
- One embed per chunk (slow on big vaults) → batch via Ollama `/api/embed`.
- File System Access API → **Tauri fs** (also removes the Chromium-only limitation).
- Direct Ollama calls → **proxy through Rust** (drops the `OLLAMA_ORIGINS` requirement) →
  then **bundle llama.cpp sidecar + default model** for true air-gap on first launch.
- CDN fonts (and later CodeMirror) → **vendor locally** for offline.
- `llama3.2` is a safe placeholder; intended default is **Phi-4-mini / small Qwen3**.
- "Rewrite selection" is named in a comment but **not wired** — only "continue" exists.
  Add rewrite-selection when moving to CodeMirror.
- No persistence of UI state (theme, last file). Add later via app config — **not** browser
  storage.

---

## 8. Immediate next task for the new session

Wrap v0 in a **Tauri 2** desktop app:

1. Scaffold with `npm create tauri-app` (Rust + Node toolchain required).
2. Port the `index.html` frontend in; **replace the `vault` object** with Tauri's
   fs + dialog plugins (pick folder, read/write `.md`).
3. Add a **Rust command** that proxies POSTs to local Ollama, so the webview needs no CORS;
   keep `CHAT_MODEL` / `EMBED_MODEL` configurable.
4. Verify the full loop in the native window: open vault → index → ⌘K ask → cited answer.
5. Then: swap the in-memory index for **LanceDB**; later **bundle a llama.cpp sidecar +
   small model** and produce installers.

**BLOCKER — ask the user first:** their **primary OS** (Windows / macOS / Linux /
cross-platform from day one). This determines the sidecar binaries and installer targets.
Also confirm: **LanceDB vs sqlite-vec**, and the **app name** (working title "Commonplace").

---

## 9. Working with this user

- Style: **bottom line up front, concise, very professional, minimal formatting**
  (avoid heavy bullets/bolding in chat prose).
- "**context check**" → reply with one sentence on how full the context window is.
- The user transfers files manually. Deliver runnable artifacts with clear run steps.
