# Commonplace

A local-first, **fully offline** Markdown notebook with an ambient AI layered on top.
It looks and behaves like a plain text editor; the intelligence lives behind **Ctrl+K**
and stays out of the way otherwise. Everything — your notes and the model — runs on your
machine. Nothing leaves it.

> A commonplace book is a personal repository of what's worth keeping: quotes, arguments,
> ideas, questions. Commonplace is that, with a local LLM that can answer from your own
> notes, find related ones, and help you write — grounded in what you've actually written.

## What it does

- **Edit Markdown** in a calm, paper-and-ink writing surface (CodeMirror 6: syntax
  highlighting, line wrapping, a paragraph-dimming focus mode).
- **Ask your notes** (Ctrl+K) — retrieval-augmented answers grounded in your vault, citing
  filenames. Also: find related notes (by meaning), summarize, continue writing, rewrite a
  selection.
- **Notebook-style autosave** — write and it's saved; no save ritual. Delete sends a note to
  the Recycle Bin (recoverable).
- **Live** — external edits to the folder show up automatically.
- **Self-contained** — bundles its own inference engine; no Ollama, no servers, nothing left
  running after you close the window.

## Architecture

- **Shell:** Tauri 2 (Rust core + native WebView2). The frontend is a single buildless
  `app/src/index.html` using Tauri's global bridge.
- **Editor:** CodeMirror 6, vendored offline (`app/src/vendor/codemirror.js`, built from
  `app/vendor-src/`).
- **Inference:** two bundled **llama.cpp** `llama-server` processes the app spawns on launch
  (chat + embeddings) and kills on exit — guaranteed even on crash via a Windows Job Object.
  Chat model: Phi-3.5-mini-instruct (MIT); embeddings: nomic-embed-text v1.5 (Apache-2.0).
- **Index:** **LanceDB** vector store in the app-data dir; incremental, survives restarts.
- **Backend commands** (`app/src-tauri/src/`): `lib.rs` (vault file I/O, chat streaming),
  `index.rs` (embeddings + LanceDB search), `engine.rs` (engine lifecycle + first-run
  model download).

## Security model

- The LLM is **text-in / text-out** — no tools, no shell, no file access. It can only emit
  text shown in the UI.
- File reads/writes are **confined to the open vault** (canonicalized; traversal refused).
- Strict **CSP**: no external script/object loading, no network egress from the webview
  (only IPC + the font CDN).
- Inference servers bind `127.0.0.1` only. See `OPEN_ITEMS.md` for remaining hardening.

## Build from source

Prerequisites: **Rust**, **Node.js LTS**, Windows **VS C++ Build Tools** (MSVC + SDK),
**protoc** (`winget install Google.Protobuf`). WebView2 ships with Windows 11.

```powershell
cd app
npm install
pwsh scripts/fetch-engine.ps1     # downloads the llama.cpp engine into resources/engine
npm run tauri dev                 # run in dev (set COMMONPLACE_ENGINE_DIR / _MODEL_DIR, see scripts)
npm run tauri build               # produce the NSIS installer
```

The GGUF models are **not** in the repo (~2 GB). The app downloads them on first launch into
`%APPDATA%\com.commonplace.app\models`. For dev, point `COMMONPLACE_MODEL_DIR` at a folder
containing them, or just let the first-run download fetch them.

## Install / try it

Run the NSIS installer produced by `npm run tauri build`
(`app/src-tauri/target/release/bundle/nsis/Commonplace_<version>_x64-setup.exe`, ~42 MB).
On first launch (with internet) it downloads the model once, then runs fully offline.

## Status & roadmap

The native app is complete and runnable; see `OPEN_ITEMS.md` for the remaining polish
(resumable download + checksums, local fonts, persisted UI state) and `HANDOFF.md` for the
design history. A parked future direction is Zettelkasten-style linked permanent notes.

## License

**MIT** — see `LICENSE`. Third-party components (llama.cpp, CodeMirror, Tauri, LanceDB)
and the downloaded models (Phi-3.5-mini · MIT, nomic-embed-text · Apache-2.0) carry their
own permissive licenses; see `THIRD-PARTY-LICENSES.md`.
