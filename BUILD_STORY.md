# How Commonplace was built

This file is a record of how Commonplace came to exist: a complete, installable,
offline AI notebook built **from scratch in a single working session of a few
hours**, by one person directing [Claude](https://claude.com) (Anthropic's
agentic coding tool, "Claude Code") to do the implementation, debugging, and
verification.

It's kept in the repo as an artifact — partly because the *way* it was built is as
interesting as the thing itself. The human made the decisions and called the
shots; the AI wrote the Rust and JavaScript, drove the Windows toolchain, ran the
builds, and tested its own work in the live app. Below is roughly what happened,
in order.

## The starting point

The session began with a one-page design brief (`HANDOFF.md`) and a v0 web
prototype: a single HTML file that talked to a local Ollama server. The goal was
to turn that idea into a *real* native, fully-offline desktop app — built from the
ground up, not assembled from existing tools.

Three decisions were locked in up front: **Windows first**, **LanceDB** for the
vector index, and the name **Commonplace**.

## The build, milestone by milestone

1. **Toolchain from nothing.** The machine had no Rust, Node, C++ build tools, or
   Ollama. All of it was installed via `winget`, scripted and run in the
   background.

2. **The native shell.** A Tauri 2 app (Rust core + native WebView2) was
   scaffolded. The v0 frontend was ported in, and its browser File-System-Access
   code was replaced with native Rust commands. Inference was proxied through Rust
   so the webview needed no CORS workarounds.

3. **Persistent retrieval.** The in-memory vector search was replaced with a real
   **LanceDB** index that survives restarts, with an incremental sync that only
   re-embeds files that actually changed.

4. **Cutting the cord (the big one).** Ollama was removed entirely. The app now
   bundles its own **llama.cpp** engine and spawns two local servers (chat +
   embeddings) on launch, killing them on exit — guaranteed even on a crash via a
   Windows **Job Object**. This is what makes it genuinely self-contained.

5. **A real editor.** The plain `<textarea>` became **CodeMirror 6** — markdown
   highlighting, line wrapping, a paragraph-dimming focus mode, a caret-pinned
   slash menu, and a "rewrite selection" AI action. It was vendored into a single
   offline bundle so the app stays buildless and CDN-free.

6. **Security + stress.** A hardening pass: file access confined to the open vault
   (path-traversal-proof), a locked-down Content-Security-Policy, and an audit
   confirming the model is text-only with no path to the system. Then abuse tests
   — oversized inputs, prompt-injection attempts — to confirm nothing breaks or
   escapes.

7. **Notebook ergonomics.** New-note creation, an empty-folder flow, notebook-style
   **autosave** (write and it's saved), **delete-to-Recycle-Bin**, and a live
   filesystem watcher so external edits appear automatically.

8. **An installer.** `tauri build` produced an NSIS installer — and immediately hit
   a real wall: the 32-bit installer compiler can't package a >2 GB payload, and
   the model alone was ~2 GB. The fix: ship a small (~42 MB) installer and have the
   app **download the model itself on first launch**, with a progress UI.

9. **Making it actually shippable.** The original chat model (Qwen2.5-3B) turned
   out to carry a research-only license. It was swapped for **Phi-3.5-mini (MIT)**
   so the whole stack is free and open-source. An MIT `LICENSE` and a third-party
   license breakdown were added, and the dependency tree (~790 crates) was audited:
   all permissive, zero copyleft.

10. **Bugs caught before release.** Driving the *installed* app to verify it,
    several issues surfaced and were fixed:
    - The CSP silently broke CodeMirror's styling in release builds (a Tauri
      style-nonce quirk) — the editor looked unstyled until it was diagnosed and
      fixed.
    - A subtle **data-loss bug**: opening a note switched the "current file"
      pointer *before* the file's contents finished loading, so an autosave firing
      in that split-second could write the wrong (or empty) text to the new file.
      It was caught during stress testing — when a sample note got wiped to 0
      bytes — traced to the race, and fixed (load before switch, plus a guard so a
      save can only ever write a buffer that belongs to the current file).

## What it took

The session ranged across Rust (Tauri commands, LanceDB/Arrow, FFI to the Windows
Job Object API, an HTTP model downloader), buildless frontend JavaScript
(CodeMirror, autosave, a file watcher), Windows packaging (NSIS, code signing
considerations, app-data layout), and a lot of running-and-watching: dozens of
background builds, health checks against the live inference servers, and finally
driving the real installed window — opening a vault, asking a question, watching
the model cite the right notes — to confirm it all worked end to end.

The result is in this repository: a small, private, offline notebook that thinks
with you, and that anyone can install, read, fork, and ship.

## Shipped

In the same session, it was published. A pre-release legal pass audited every
dependency (~790 Rust crates plus the JS bundle and the two models) and confirmed
the whole stack is permissively licensed with no copyleft — clear to release under
MIT. License files and full third-party attribution were added, the README was
expanded into a proper project page, and then it went live:

- **Repository:** https://github.com/aidanfritzke/commonplace (public, MIT)
- **Release:** [`v0.1.0`](https://github.com/aidanfritzke/commonplace/releases/tag/v0.1.0)
  with the ~42 MB Windows installer attached as a downloadable asset.

The repo was created, the history pushed, and the installer uploaded as a GitHub
release — all driven from the same session, using the GitHub CLI. Start to finish —
empty machine to a public, installable, open-source application — in a single
afternoon.

---

*Built with Claude. The decisions were human; the keystrokes were not.*
