// CodeMirror 6 vendor entry — bundled by esbuild into src/vendor/codemirror.js
// as an IIFE that exposes `window.CM`. This keeps the app frontend buildless at
// runtime and fully offline (no CDN imports).

import {
  EditorView,
  keymap,
  highlightActiveLine,
  drawSelection,
  placeholder,
  Decoration,
  ViewPlugin,
} from "@codemirror/view";
import { EditorState, Compartment, RangeSetBuilder } from "@codemirror/state";
import {
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
} from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { syntaxHighlighting, HighlightStyle } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";
import { autocompletion } from "@codemirror/autocomplete";

window.CM = {
  EditorView,
  EditorState,
  Compartment,
  RangeSetBuilder,
  keymap,
  highlightActiveLine,
  drawSelection,
  placeholder,
  Decoration,
  ViewPlugin,
  defaultKeymap,
  history,
  historyKeymap,
  indentWithTab,
  markdown,
  markdownLanguage,
  syntaxHighlighting,
  HighlightStyle,
  tags: t,
  autocompletion,
};
