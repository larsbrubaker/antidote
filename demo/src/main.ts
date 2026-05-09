// Loads the wasm-pack output, fetches runtime-config.json (Supabase URL + anon key),
// and drives the canvas. M5 fills in the actual render/input wiring.

type RuntimeConfig = {
  SUPABASE_URL: string;
  SUPABASE_ANON_KEY: string;
};

async function loadConfig(): Promise<RuntimeConfig> {
  const resp = await fetch("/runtime-config.json", { cache: "no-store" });
  if (!resp.ok) throw new Error(`runtime-config.json missing: ${resp.status}`);
  return await resp.json();
}

async function main() {
  const config = await loadConfig();
  console.log("antidote: loaded runtime config for", config.SUPABASE_URL);

  // M5: import("/pkg/antidote_wasm.js"), call init(), wire pointer events.
  const canvas = document.getElementById("antidote-canvas") as HTMLCanvasElement | null;
  if (!canvas) throw new Error("missing #antidote-canvas");

  const ctx = canvas.getContext("2d");
  if (ctx) {
    ctx.fillStyle = "#222";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.fillStyle = "#fff";
    ctx.font = "24px sans-serif";
    ctx.textAlign = "center";
    ctx.fillText("Antidote — M5 stub", canvas.width / 2, canvas.height / 2);
  }
}

main().catch((err) => {
  console.error(err);
});
