# Commonplace — native (Tauri 2)

A local-first Markdown editor with an ambient local LLM behind **Ctrl+K**.
Looks and behaves like a text editor; everything runs on your machine — no data
leaves it. This is the native desktop shell (the successor to the v0 web build):
file I/O and inference are handled by a Rust backend, so there's no browser
CORS / `OLLAMA_ORIGINS` requirement and no Chromium-only limitation.

## Prerequisites

- **Rust** toolchain (`rustc` + `cargo`), **Node.js LTS**, and on Windows the
  **Visual Studio C++ Build Tools** (MSVC + Windows SDK). WebView2 ships with
  Windows 11.
- **protoc** (Protocol Buffers compiler) on `PATH` — LanceDB's build needs it.
  Install with `winget install Google.Protobuf` (or your package manager).

Inference is **fully bundled** — no Ollama, no external services. The app ships
its own llama.cpp engine (`src-tauri/resources/engine/`) plus two GGUF models
(`src-tauri/resources/models/`): Qwen2.5-3B-Instruct (chat) and
nomic-embed-text-v1.5 (embeddings). On launch the app spawns two local
`llama-server` processes on `127.0.0.1:11500` (chat) and `:11501` (embed), and
kills them when the window closes. Nothing runs in the background once it's shut.

> Dev note: in `tauri dev` the engine/model dirs are located via the
> `COMMONPLACE_ENGINE_DIR` / `COMMONPLACE_MODEL_DIR` env vars (set in
> `_setup/dev.ps1`); in a packaged build they're resolved from the app's
> resource directory.

## Run it (dev)

From this `app/` folder:
```
npm install
npm run tauri dev
```
The first run compiles the Rust core (a few minutes); later runs are fast.
A native **Commonplace** window opens.

1. Press **Ctrl+O** (or "open folder") and pick a folder of `.md` files. It walks
   the folder, opens the first note, and indexes the rest in the background.
2. **Ctrl+K** → ask about your notes, find related, summarize, or continue writing.

## Build an installer

```
npm run tauri build
```
Produces a Windows installer (MSI / NSIS) under
`src-tauri/target/release/bundle/`. Cross-platform targets (macOS/Linux) build
from the same source on those OSes — nothing in the Rust layer is
Windows-specific.

## Keys

| Key | Action |
|-----|--------|
| Ctrl+O | Open a folder of notes |
| Ctrl+P | Jump to a note (quick switcher) |
| Ctrl+K | Command palette — ask / find related / summarize / continue |
| Ctrl+S | Save the current note |
| Ctrl+. | Focus mode (hide all chrome) |
| `/` on an empty line | Inline "continue writing" |
| Esc | Close any panel |

## How it's built

- **Shell:** Tauri 2 (Rust core + native WebView2). The frontend is a single
  self-contained `src/index.html` — no bundler. It uses Tauri's global bridge
  (`withGlobalTauri`), so the editor stays one buildless HTML file like v0.
- **Backend commands** (`src-tauri/src/lib.rs`):
  - `pick_folder`, `list_markdown`, `read_file`, `write_file` — native vault I/O.
  - `ollama_embed`, `ollama_chat` — proxy embeddings + streamed chat to local
    Ollama. Chat tokens stream to the UI over a Tauri `Channel`.
- **RAG:** paragraph chunking, cosine similarity, in-memory index (survives only
  for the session for now).

## Where it goes next

- Persist the vector index with **LanceDB** so it survives restarts (currently
  in-memory).
- Editor: `<textarea>` → **CodeMirror 6** (syntax highlight, paragraph-dimming
  focus, caret-pinned `/` menu, rewrite-selection).
- Batch embeddings via Ollama `/api/embed` for big vaults.
- Bundle a **llama.cpp sidecar + a small default model** (Phi-4-mini / small
  Qwen3) so the shipped app is fully air-gapped on first launch.
- Vendor the fonts locally (they load from a CDN in dev).
- Persist UI state (theme, last file) via app config.
