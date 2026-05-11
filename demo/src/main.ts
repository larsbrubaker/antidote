// Browser platform shell for Antidote.
//
// This file owns only DOM/canvas concerns: load wasm-pack output, resize the
// canvas, forward browser input, and drive requestAnimationFrame. Game rules,
// widget trees, menus, and layout live in antidote-core.

async function main() {
  const canvas = document.getElementById("antidote-canvas") as HTMLCanvasElement | null;
  if (!canvas) throw new Error("missing #antidote-canvas");

  // The production bundle lives under `assets/`, while Vite copies
  // `public/pkg` to the site root. Resolve through `import.meta.url` so both
  // dev (`/src/main.ts`) and Pages (`/assets/index-*.js`) find `/pkg`.
  const wasmJsUrl = new URL("../pkg/antidote_wasm.js", import.meta.url).href;
  const wasmBgUrl = new URL("../pkg/antidote_wasm_bg.wasm", import.meta.url).href;
  const wasm = await import(/* @vite-ignore */ wasmJsUrl);
  await wasm.default(wasmBgUrl);

  // antidote-wasm asks wgpu for the WebGL2 downlevel-baseline limits, which
  // cap surface (and texture) dimensions at 2048 in each axis. On a Pixel in
  // landscape (~700 × 422 CSS) × DPR 3 = 2100 × 1266 — `Surface::configure`
  // panics with a "max extent 2048" validation error and the renderer never
  // produces a frame, which is why the screen stayed black. Cap the effective
  // DPR so the larger axis lands at most at this value.
  const MAX_SURFACE_DIM = 2048;

  // Size the canvas from JS each time the viewport changes — both the CSS
  // box (`canvas.style.{width,height}`) and the backing store
  // (`canvas.{width,height}`). Prefer `visualViewport` because on mobile
  // browsers it tracks the *visible* area (URL bar collapsing in/out, IME
  // open, devtools device emulator) more honestly than `innerWidth/Height`.
  const resizeCanvas = () => {
    const rawDpr = Math.max(0.5, window.devicePixelRatio || 1);
    const vw = window.visualViewport?.width ?? window.innerWidth;
    const vh = window.visualViewport?.height ?? window.innerHeight;
    const cssW = Math.max(1, Math.floor(vw));
    const cssH = Math.max(1, Math.floor(vh));
    // Pull DPR down if the resulting buffer would exceed the surface cap.
    // CSS dimensions stay unchanged; the browser scales the (slightly
    // smaller) bitmap to fill them. The same effective DPR goes to
    // `set_device_pixel_ratio` so agg-gui's widget scaling stays in sync
    // with what we actually rendered.
    const maxAxis = Math.max(cssW, cssH) * rawDpr;
    const dpr =
      maxAxis > MAX_SURFACE_DIM ? rawDpr * (MAX_SURFACE_DIM / maxAxis) : rawDpr;
    canvas.style.width = `${cssW}px`;
    canvas.style.height = `${cssH}px`;
    canvas.width = Math.max(1, Math.min(MAX_SURFACE_DIM, Math.floor(cssW * dpr)));
    canvas.height = Math.max(1, Math.min(MAX_SURFACE_DIM, Math.floor(cssH * dpr)));
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
  // `visualViewport` fires its own resize when the mobile URL bar slides in
  // or out — handle it so the backing store grows into the reclaimed space
  // without waiting for the next orientation change.
  window.visualViewport?.addEventListener("resize", resizeCanvas);
  window.addEventListener("orientationchange", resizeCanvas);
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
