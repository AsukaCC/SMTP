import {build} from "esbuild";
import {fileURLToPath} from "node:url";

await build({
  absWorkingDir: fileURLToPath(new URL(".", import.meta.url)),
  entryPoints: ["glass.jsx"],
  outfile: "../static/vendor/liquid-glass.js",
  bundle: true,
  minify: true,
  format: "iife",
  target: ["es2020"],
  jsx: "automatic",
  define: {"process.env.NODE_ENV": '"production"'},
  legalComments: "inline",
});
