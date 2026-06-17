// Builds the offline vendor bundles in src/vendor/ (window.CM, window.ForceGraph).
//   npm run vendor
// Buildless at runtime: this is the one offline build step. Each bundle is a
// minified IIFE with a license banner (legalComments:"none" strips inline notices,
// so the banner carries the required attribution).
import { build } from "esbuild";

const cmBanner = `/*! Vendored bundle for Commonplace.
    Contains CodeMirror 6 (https://codemirror.net) and its dependencies,
    Copyright (C) by Marijn Haverbeke and others, MIT License.
    Full third-party license texts: see THIRD-PARTY-LICENSES.md in the repo root. */`;

const fgBanner = `/*! Vendored bundle for Commonplace.
    Contains force-graph (https://github.com/vasturiano/force-graph) and its d3
    dependencies, MIT/ISC/BSD licensed. See THIRD-PARTY-LICENSES.md in the repo root. */`;

const common = { bundle: true, format: "iife", minify: true, legalComments: "none" };

await build({ ...common, entryPoints: ["vendor-src/cm-entry.mjs"], banner: { js: cmBanner }, outfile: "src/vendor/codemirror.js" });
await build({ ...common, entryPoints: ["vendor-src/fg-entry.mjs"], banner: { js: fgBanner }, outfile: "src/vendor/force-graph.js" });
