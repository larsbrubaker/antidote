import { defineConfig } from "vite";

export default defineConfig({
  // GitHub Pages serves the demo at https://larsbrubaker.github.io/antidote/
  // so all asset paths must be prefixed accordingly. Local `vite dev` is fine
  // with a relative base too.
  base: "./",
});
