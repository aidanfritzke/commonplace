// Force-graph vendor entry — bundled offline into src/vendor/force-graph.js as an
// IIFE exposing window.ForceGraph (the canvas knowledge-map renderer). Built by
// the same esbuild step as CodeMirror (see build.mjs); keeps the app offline.
import ForceGraph from "force-graph";
window.ForceGraph = ForceGraph;
