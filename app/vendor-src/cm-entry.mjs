// CodeMirror 6 vendor entry — bundled by esbuild into src/vendor/codemirror.js
// as an IIFE that exposes `window.CM`. This keeps the app frontend buildless at
// runtime and fully offline (no CDN imports).

import {
  EditorView,
  keymap,
  highlightActiveLine,
  drawSelection,
  placeholder,
} from "@codemirror/view";
import { EditorState, Compartment } from "@codemirror/state";
import {
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
} from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { syntaxHighlighting, HighlightStyle } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";

window.CM = {
  EditorView,
  EditorState,
  Compartment,
  keymap,
  highlightActiveLine,
  drawSelection,
  placeholder,
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
  markdown,
  markdownLanguage,
  syntaxHighlighting,
  HighlightStyle,
  tags: t,
};
