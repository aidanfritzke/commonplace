# Commonplace — v0

A minimal, local-first Markdown editor that can answer questions grounded in your own
notes. Looks and behaves like a text editor; the intelligence lives behind ⌘K and stays
out of the way otherwise. Everything runs on your machine — no data leaves it.

## Prerequisites

1. **Ollama** installed and running, with two models pulled:
   ```
   ollama pull nomic-embed-text     # embeddings (semantic search over your notes)
   ollama pull llama3.2             # the chat model (swap for phi4-mini, qwen2.5:3b, …)
   ```
2. A **Chromium browser** (Chrome, Edge, or Brave). v0 uses the File System Access API
   to read/write a real folder; Firefox/Safari don't support it yet. The packaged
   (Tauri) build removes this limitation.

## Run it

1. Start Ollama so the page is allowed to reach it:
   ```
   OLLAMA_ORIGINS=* ollama serve
   ```
2. Serve this folder over localhost (needed for folder access to work):
   ```
   python3 -m http.server 8000
   ```
3. Open `http://localhost:8000` in Chrome/Edge/Brave.
4. Press **⌘O** (Ctrl+O on Windows/Linux) and pick a folder of `.md` files. It indexes
   in the background, then you can ask.

> Note: this won't work in an in-app preview — it needs *your* local Ollama and *your*
> file system. Download it and run it locally with the steps above.

## Keys

| Key | Action |
|-----|--------|
| ⌘O | Open a folder of notes |
| ⌘P | Jump to a note (quick switcher) |
| ⌘K | Command palette — ask / find related / summarize / continue |
| ⌘S | Save the current note |
| ⌘. | Focus mode (hide all chrome) |
| `/` on an empty line | Inline "continue writing" |
| Esc | Close any panel |

## What v0 does

- Edit Markdown files in a clean, distraction-free writing surface.
- **Ask about your notes** — retrieves the most relevant passages from your vault and
  answers from them, citing filenames. (Retrieval-augmented; grounded, not made up.)
- **Find related notes** — surfaces other notes close in meaning to the current one.
- **Summarize** the current note; **continue writing** at the cursor.

## How it's built (and where it goes next)

- Single self-contained HTML file. The editor is a styled `<textarea>` for v0 — the
  planned upgrade is **CodeMirror 6** for syntax highlighting and paragraph-dimming focus.
- File I/O is isolated in the `vault` object; swapping to a native **Tauri** app means
  replacing that one object with Tauri's fs APIs — the rest is unchanged.
- Inference goes straight to **Ollama** for now. Next steps: proxy it through Tauri's
  Rust backend (drops the `OLLAMA_ORIGINS` requirement), then bundle a **llama.cpp**
  sidecar + a default small model so the shipped app is fully air-gapped on first launch.
- The fonts and (later) editor library load from a CDN in dev; the packaged offline
  build vendors them.

## Parked for later — v1: Zettelkasten / "Smart Notes"

Not built yet, logged as the next avenue: atomic, interconnected **permanent notes**
with explicit links, bottom-up writing where notes accumulate into essays, and
AI-assisted linking/backlinks. v0's grounded, citation-based answers already lean this
direction; v1 would make the links and note atomicity first-class.
