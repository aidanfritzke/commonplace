# Commonplace

A local-first, **fully offline** Markdown notebook with a small AI model living
quietly underneath it. It looks and behaves like a plain text editor — the
intelligence is summoned with **Ctrl+K** and stays out of the way otherwise.
Your notes and the model both run entirely on your machine. Nothing is ever sent
anywhere.

> [!TIP]
> **Try it.** Install it, open the included [`sample-vault/`](sample-vault) folder,
> and press **Ctrl+K**. Ask something cross-topic like *"what's a good prompt for
> extracting action items, and which meeting has open ones?"* — it answers from your
> notes and **cites the files it used, across folders.**

> A *commonplace book* is a centuries-old practice: a personal collection of what's
> worth keeping — quotations, arguments, ideas, questions you don't yet have the
> answer to. Commonplace is that, with a local model that can answer from your own
> notes, surface related ones, and help you write — always grounded in what you've
> actually written.

---

## Features

- **A calm writing surface** — Markdown with syntax highlighting, line wrapping, a
  monospace (Courier New) page, and a focus mode that dims every paragraph except
  the one you're in.
- **Ask your notes** (Ctrl+K) — retrieval-augmented answers grounded in your vault,
  citing the filenames they came from. Also: find related notes by meaning,
  summarize the current note, continue writing at the cursor, rewrite a selection.
- **Link your thinking** — `[[wikilinks]]` with autocomplete and Ctrl-click to
  follow (or create a missing note); a backlinks strip showing what links here; and
  **Suggest links** (Ctrl+K), which proposes connections by meaning and inserts them
  with one click.
- **See the shape of your vault** — a **knowledge map** (Ctrl+G) of your notes:
  explicit `[[link]]` edges plus meaning-based (semantic) edges you can toggle. Plus
  a summoned **file tree** (Ctrl+\) and a folder-aware quick switcher.
- **Quick-capture** (Ctrl+Shift+N) — drop a timestamped fleeting note into `inbox/`
  from anywhere, no friction.
- **Use it from your AI agent** — an optional, read-only local [MCP](https://modelcontextprotocol.io)
  server lets Claude Desktop / Claude Code search and read your vault (see below).
- **Notebook-style autosave** — you write, it's saved. No save ritual, no
  "unsaved changes" prompts.
- **Delete to the Recycle Bin** — removing a note is recoverable, not permanent.
- **Live** — edits made to the folder by other programs show up automatically.
- **Truly self-contained** — the app ships its own inference engine and downloads
  its model once on first launch. No Ollama, no Python, no servers, no accounts —
  and nothing left running in the background after you close the window.

## How it works

Commonplace is a small native desktop app (a few MB) wrapped around three ideas:

1. **Your notes are just Markdown files** in a folder you choose (the "vault").
   Plain text you own — inspectable, greppable, future-proof. The app never locks
   them in a database.
2. **A local LLM, used as a thinking layer.** On launch the app starts two small
   [llama.cpp](https://github.com/ggml-org/llama.cpp) servers on `localhost`: one
   running a chat model ([Phi-3.5-mini](https://huggingface.co/microsoft/Phi-3.5-mini-instruct)),
   one running an embedding model ([nomic-embed-text](https://huggingface.co/nomic-ai/nomic-embed-text-v1.5)).
   They're owned by the app and shut down when it closes.
3. **Retrieval over your own writing.** Your notes are split into passages,
   embedded, and stored in a local [LanceDB](https://lancedb.com) vector index. When
   you ask a question, the most relevant passages are pulled in and the chat model
   answers from *them* — citing the source files — rather than from memory. The
   index persists, so reopening a vault is instant.

Importantly, the model is **text-in / text-out only.** It has no tools, no shell,
no ability to touch files or the network. The worst a confused or "jailbroken"
model can do is produce misleading text in a side panel — it cannot act on your
computer.

**Stack:** [Tauri 2](https://tauri.app) (Rust core + native WebView2) · a buildless
HTML/[CodeMirror 6](https://codemirror.net) frontend · llama.cpp for inference ·
LanceDB for the index.

## What it runs on

- **Windows 10/11, x64.** (The codebase is cross-platform via Tauri, but only
  Windows is built and tested today.)
- **CPU-only inference** — no GPU required. A modern multi-core CPU runs the 3.8B
  chat model at a usable speed; more cores / RAM is better. Budget ~4 GB RAM while
  the model is loaded.
- **~2.4 GB of disk** for the model (downloaded once into your app-data folder).
- **Internet** is needed exactly once — the first launch downloads the model. After
  that it is 100% offline.

## Install

1. Download `Commonplace_x64-setup.exe` from the
   [Releases](https://github.com/aidanfritzke/commonplace/releases) page (~42 MB) and run it.
2. On first launch it downloads the AI model (~2.3 GB, one time, with a progress
   bar) into `%APPDATA%\com.commonplace.app\models`. Keep the window open until it
   finishes.
3. Press **Ctrl+O**, pick a folder of `.md` files (or an empty folder — it'll offer
   to create your first note), and start writing.

To uninstall: **Settings → Apps → Commonplace → Uninstall** (or the Start-menu
"Uninstall Commonplace"). It removes the app cleanly; delete
`%APPDATA%\com.commonplace.app` if you also want to remove the downloaded model and
the index.

## Keys

| Key | Action |
|-----|--------|
| Ctrl+O | Open a folder of notes |
| Ctrl+N | New note |
| Ctrl+Shift+N | Quick-capture a fleeting note into `inbox/` |
| Ctrl+P | Jump to a note (folder-aware quick switcher) |
| Ctrl+K | Command palette — ask · suggest links · related · summarize · continue · rewrite · delete |
| Ctrl+G | Knowledge map |
| Ctrl+\ | File tree |
| `[[` | Link to another note (autocomplete; Ctrl-click a link to follow) |
| Ctrl+S | Save & re-index now (notes autosave anyway) |
| Ctrl+. | Focus mode |
| `/` on an empty line | Inline command palette |
| Esc | Close any panel / exit focus mode |

## Privacy & security

- **Everything is local.** No telemetry, no analytics, no accounts. The only
  network request the app ever makes is the one-time model download from Hugging
  Face; after that it runs with no network access.
- **File access is confined to the open vault** — the backend refuses any read or
  write outside it (path traversal included).
- **The webview is locked down** with a Content-Security-Policy that blocks loading
  external code and blocks any network egress from the UI.
- **The model cannot act** — it only emits text (see "How it works").
- Provided "as is," without warranty (see `LICENSE`).

## Use your vault from an AI agent (MCP)

Commonplace ships a small, **read-only** [Model Context Protocol](https://modelcontextprotocol.io)
server so a local AI agent — Claude Desktop or Claude Code — can read and search
your notes as a tool. It is optional, local-only, and cannot write or delete:
the tools are `list_notes`, `read_note`, `search`, and `related_notes`.

It opens the same on-disk index read-only and uses the running app's local
embedder, so two things matter:

- **Commonplace must be running** for the semantic tools (`search`,
  `related_notes`) — the app serves the embedding model on `127.0.0.1:11501`.
  `list_notes` and `read_note` are plain file reads and work either way.
- Point it at the **same vault folder** you open in the app (the index is keyed by
  that path).

The server binary, `mcp.exe`, **ships with Commonplace** — it's installed in the
app's program folder under `resources\mcp.exe`. A default per-user install puts it
at `%LOCALAPPDATA%\Commonplace\resources\mcp.exe`; an all-users install at
`C:\Program Files\Commonplace\resources\mcp.exe`. Point your MCP client's `command`
at that path (use the full absolute path — config files don't expand
`%LOCALAPPDATA%`). Building from source, it's `app/src-tauri/target/release/mcp.exe`.

**Claude Code** (one command):

```powershell
claude mcp add commonplace -- "$env:LOCALAPPDATA\Commonplace\resources\mcp.exe" --vault "C:\path\to\your\vault"
```

**Claude Desktop** — add to `%APPDATA%\Claude\claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "commonplace": {
      "command": "C:\\Users\\<you>\\AppData\\Local\\Commonplace\\resources\\mcp.exe",
      "args": ["--vault", "C:\\path\\to\\your\\vault"]
    }
  }
}
```

Then ask your agent things like *"search my commonplace notes for prompt
patterns"* or *"what notes relate to `ai/project-ideas.md`?"*.

## Build from source

Prerequisites: **Rust**, **Node.js LTS**, the Windows **Visual Studio C++ Build
Tools** (MSVC + Windows SDK), and **protoc** (`winget install Google.Protobuf`).
WebView2 ships with Windows 11.

```powershell
cd app
npm install
pwsh scripts/fetch-engine.ps1        # downloads the llama.cpp engine into resources/engine
npm run tauri dev                    # run in dev (see _setup/dev.ps1 for the env it expects)
npm run tauri build                  # produce the NSIS installer in src-tauri/target/release/bundle/nsis
```

Notes:
- The GGUF models are **not** in the repo (~2.3 GB). They download on first launch;
  for dev you can point `COMMONPLACE_MODEL_DIR` at a folder that already has them.
- The chat model is intentionally permissively licensed (MIT). To swap it, change
  the filename/URL constants in `app/src-tauri/src/engine.rs`.

## Project layout

```
app/                       the Tauri application
  src/index.html           the entire frontend (buildless, single file)
  src/vendor/codemirror.js vendored CodeMirror 6 bundle
  src/vendor/force-graph.js vendored force-graph bundle (knowledge map)
  vendor-src/build.mjs     the one offline build step (npm run vendor)
  src-tauri/src/
    lib.rs                 vault file I/O + chat streaming + commands
    index.rs               embeddings + LanceDB search + links/semantic edges
    engine.rs              engine lifecycle + first-run model download
    bin/mcp.rs             read-only MCP server (separate binary)
sample-vault/              a few demo notes to try it on
HANDOFF.md                 the original design brief
OPEN_ITEMS.md              tracked follow-ups
BUILD_STORY.md             how this was built (with Claude, in a few hours)
```

## License

**MIT** — see `LICENSE`. All third-party components and the downloaded models are
permissively licensed; see `THIRD-PARTY-LICENSES.md` for the full breakdown
(there are no copyleft dependencies).

## Acknowledgments

Built on the work of [llama.cpp](https://github.com/ggml-org/llama.cpp),
[Tauri](https://tauri.app), [CodeMirror](https://codemirror.net),
[LanceDB](https://lancedb.com), Microsoft's [Phi-3.5](https://huggingface.co/microsoft/Phi-3.5-mini-instruct),
and Nomic's [embedding model](https://huggingface.co/nomic-ai/nomic-embed-text-v1.5).
The design lineage (iA Writer / Typora) and the commonplace-book idea are noted in
`HANDOFF.md`.
