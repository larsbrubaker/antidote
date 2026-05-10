// Browser platform shell for Antidote.
//
// This file owns only DOM/canvas concerns: load wasm-pack output, fetch runtime
// config, resize the canvas, forward browser input, and drive requestAnimationFrame.
// Game rules, widget trees, menus, and layout live in antidote-core.

type RuntimeConfig = {
  SUPABASE_URL: string;
  SUPABASE_ANON_KEY: string;
};

async function loadConfig(): Promise<RuntimeConfig> {
  // Relative path so this works under any base URL (e.g. larsbrubaker.github.io/antidote/).
  const resp = await fetch("runtime-config.json", { cache: "no-store" });
  if (!resp.ok) throw new Error(`runtime-config.json missing: ${resp.status}`);
  return await resp.json();
}

async function main() {
  const config = await loadConfig();
  console.log("antidote: loaded runtime config for", config.SUPABASE_URL);

  const canvas = document.getElementById("antidote-canvas") as HTMLCanvasElement | null;
  if (!canvas) throw new Error("missing #antidote-canvas");

  // The production bundle lives under `assets/`, while Vite copies
  // `public/pkg` to the site root. Resolve through `import.meta.url` so both
  // dev (`/src/main.ts`) and Pages (`/assets/index-*.js`) find `/pkg`.
  const wasmJsUrl = new URL("../pkg/antidote_wasm.js", import.meta.url).href;
  const wasmBgUrl = new URL("../pkg/antidote_wasm_bg.wasm", import.meta.url).href;
  const wasm = await import(/* @vite-ignore */ wasmJsUrl);
  await wasm.default(wasmBgUrl);

  // OAuth + password-recovery callback handler. Supabase redirects back to
  // this page with `#access_token=...&refresh_token=...&expires_in=...&type=...`
  // appended to the URL. `type=recovery` means the user followed a
  // password-reset email — route to the SetPassword overlay instead of
  // signing them in directly. Anything else (oauth, magiclink, signup,
  // invite) installs the session as a normal sign-in. Either way, we strip
  // the hash so a refresh doesn't try to install the tokens twice.
  if (window.location.hash.startsWith("#access_token=")) {
    const params = new URLSearchParams(window.location.hash.slice(1));
    const accessToken = params.get("access_token");
    const refreshToken = params.get("refresh_token") ?? "";
    const expiresIn = parseInt(params.get("expires_in") ?? "3600", 10);
    const tokenType = params.get("type") ?? "";
    if (accessToken) {
      const ttl = isNaN(expiresIn) ? 3600 : expiresIn;
      if (tokenType === "recovery") {
        wasm.enter_recovery_mode(accessToken, refreshToken, ttl);
      } else {
        wasm.oauth_complete(accessToken, refreshToken, ttl);
      }
      history.replaceState(null, "", window.location.pathname + window.location.search);
    }
  }

  const resizeCanvas = () => {
    const dpr = Math.max(0.5, window.devicePixelRatio || 1);
    const maxWidth = window.innerWidth;
    const maxHeight = window.innerHeight;
    const aspect = 800 / 600;
    const cssWidth = Math.floor(
      maxWidth / aspect <= maxHeight ? maxWidth : maxHeight * aspect,
    );
    const cssHeight = Math.floor(cssWidth / aspect);

    canvas.style.width = `${cssWidth}px`;
    canvas.style.height = `${cssHeight}px`;
    canvas.width = Math.max(1, Math.floor(cssWidth * dpr));
    canvas.height = Math.max(1, Math.floor(cssHeight * dpr));
    wasm.set_device_pixel_ratio(dpr);
  };

  const canvasPoint = (event: PointerEvent) => {
    const rect = canvas.getBoundingClientRect();
    return {
      x: ((event.clientX - rect.left) / rect.width) * canvas.width,
      y: ((event.clientY - rect.top) / rect.height) * canvas.height,
    };
  };

  canvas.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    canvas.setPointerCapture(event.pointerId);
    const point = canvasPoint(event);
    wasm.on_mouse_down(point.x, point.y, event.button);
  });
  canvas.addEventListener("pointermove", (event) => {
    event.preventDefault();
    const point = canvasPoint(event);
    wasm.on_mouse_move(point.x, point.y);
  });
  canvas.addEventListener("pointerup", (event) => {
    event.preventDefault();
    const point = canvasPoint(event);
    wasm.on_mouse_up(point.x, point.y, event.button);
  });
  canvas.addEventListener("pointercancel", (event) => {
    event.preventDefault();
    wasm.on_mouse_leave();
  });
  canvas.addEventListener("pointerleave", () => {
    wasm.on_mouse_leave();
  });

  // Keyboard forwarding — agg-gui's TextField widget needs key events to
  // accept typing, and the global Esc/P pause handler reads them too.
  // Listen on `window` (not the canvas) so focus on the page lets typing
  // through even before the user has clicked into a TextField; the wasm
  // side gates by widget focus internally.
  window.addEventListener("keydown", (event) => {
    const handled = wasm.on_key_down(
      event.key,
      event.shiftKey,
      event.ctrlKey,
      event.altKey,
      event.metaKey,
    );
    if (handled) {
      event.preventDefault();
    }
  });
  window.addEventListener("keyup", (event) => {
    wasm.on_key_up(
      event.key,
      event.shiftKey,
      event.ctrlKey,
      event.altKey,
      event.metaKey,
    );
  });

  window.addEventListener("resize", resizeCanvas);
  resizeCanvas();

  let last = performance.now();
  const frame = (now: number) => {
    const frameMs = now - last;
    last = now;
    if (wasm.needs_draw()) {
      wasm.render(canvas.width, canvas.height, frameMs);
    }
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}

main().catch((err) => {
  console.error(err);
});
