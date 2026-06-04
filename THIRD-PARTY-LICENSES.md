# Third-Party Licenses

Commonplace is licensed under the MIT License (see `LICENSE`). It is built with,
links against, and at runtime downloads, the third-party components listed below.
Every component is under a permissive license (MIT, Apache-2.0, BSD, ISC, Zlib,
Unicode, CC0, or similar). There are **no copyleft (GPL/AGPL/LGPL/SSPL)
dependencies** anywhere in the project.

What is and isn't redistributed:

- **In this repository:** the CodeMirror 6 editor is vendored as
  `app/src/vendor/codemirror.js` (a minified bundle). Its attribution banner is
  at the top of that file. Everything else listed here is *referenced*, not
  copied into the repo.
- **Fetched at build time:** the llama.cpp engine binaries
  (`app/scripts/fetch-engine.ps1` → `app/src-tauri/resources/engine/`).
- **Downloaded by the app on first launch:** the two model files, into the
  per-user app-data folder. No model weights are shipped in the repo or installer.

## Components

| Component | Role | License | Redistributed? |
|-----------|------|---------|----------------|
| [CodeMirror 6](https://codemirror.net/) | editor | MIT | yes (`app/src/vendor/codemirror.js`) |
| [llama.cpp](https://github.com/ggml-org/llama.cpp) | inference engine | MIT | in the installer (fetched at build) |
| [Tauri](https://tauri.app/) (+ plugins) | app shell | MIT / Apache-2.0 | compiled into the binary |
| [LanceDB](https://github.com/lancedb/lancedb) / Lance / Apache Arrow / DataFusion | vector index | Apache-2.0 | compiled into the binary |
| Rust crates (reqwest, tokio, serde, notify, trash, windows-sys, …) | backend | MIT / Apache-2.0 / BSD / ISC / Zlib / Unicode-3.0 / CC0 / MPL-2.0 | compiled into the binary |
| esbuild, @tauri-apps/cli | build tooling | MIT | not shipped |

The Rust dependency tree (≈790 crates) was audited: all are permissive. Five
crates are **MPL-2.0** (`cssparser`, `cssparser-macros`, `dtoa-short`,
`option-ext`, `selectors`) — MPL-2.0 is file-level weak copyleft and imposes no
obligation on this project's own source when used unmodified.

## Models (downloaded on first launch — not redistributed here)

| Model | Role | License |
|-------|------|---------|
| [Phi-3.5-mini-instruct](https://huggingface.co/microsoft/Phi-3.5-mini-instruct) (GGUF via [bartowski](https://huggingface.co/bartowski/Phi-3.5-mini-instruct-GGUF)) | chat / RAG | MIT |
| [nomic-embed-text v1.5](https://huggingface.co/nomic-ai/nomic-embed-text-v1.5) | embeddings | Apache-2.0 |

Both model licenses permit free use, commercial use, and redistribution.

---

## MIT License

Applies to Commonplace, CodeMirror, llama.cpp, Phi-3.5-mini, Tauri, and the many
MIT-licensed crates and packages above.

```
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Relevant copyright holders include (non-exhaustive): CodeMirror — Copyright (C)
by Marijn Haverbeke and others; llama.cpp — Copyright (c) the ggml authors;
Phi-3.5 — Copyright (c) Microsoft Corporation; Tauri — Copyright (c) Tauri
Programme within The Commons Conservancy.

## Apache License 2.0

Applies to LanceDB, Lance, Apache Arrow, DataFusion, nomic-embed-text v1.5, and
many dual MIT/Apache-2.0 crates. Full text:
https://www.apache.org/licenses/LICENSE-2.0

The Apache-2.0 components are used unmodified; their NOTICE attributions are
preserved by reference to their upstream repositories linked above.
