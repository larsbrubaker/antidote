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

  // Size the canvas from JS each time the viewport changes — both the CSS
  // box (`canvas.style.{width,height}`) and the backing store
  // (`canvas.{width,height}`). Prefer `visualViewport` because on mobile
  // browsers it tracks the *visible* area (URL bar collapsing in/out, IME
  // open, devtools device emulator) more honestly than `innerWidth/Height`.
  //
  // No artificial DPR cap — antidote-wasm raises its wgpu surface-dimension
  // limit to the adapter's reported max at init, and WebGL2 in any browser
  // we care about reports at least 4096 (Pixel + modern desktops are usually
  // 8192–16384). If a future device truly only supports 2048, the resulting
  // wgpu surface-configure error will at least be specific enough to surface
  // that here — better than silently downscaling for every device just in
  // case.
  const resizeCanvas = () => {
    const dpr = Math.max(0.5, window.devicePixelRatio || 1);
    const vw = window.visualViewport?.width ?? window.innerWidth;
    const vh = window.visualViewport?.height ?? window.innerHeight;
    const cssW = Math.max(1, Math.floor(vw));
    const cssH = Math.max(1, Math.floor(vh));
    canvas.style.width = `${cssW}px`;
    canvas.style.height = `${cssH}px`;
    canvas.width = Math.max(1, Math.floor(cssW * dpr));
    canvas.height = Math.max(1, Math.floor(cssH * dpr));
    wasm.set_device_pixel_ratio(dpr);
  };

  const canvasPoint = (event: PointerEvent) => {
    const rect = canvas.getBoundingClientRect();
    return {
      x: ((event.clientX - rect.left) / rect.width) * canvas.width,
      y: ((event.clientY - rect.top) / rect.height) * canvas.height,
    };
  };

  // First user gesture → request browser fullscreen so the URL bar (and on
  // Android, the system status / nav bars) collapse and the canvas fills the
  // whole screen. Requires a user-initiated event by the spec, so it has to
  // ride on the first pointerdown.
  //
  // Target the canvas itself rather than `document.documentElement`: Chrome
  // Android has shipped versions that quietly reject fullscreening the
  // <html> element on touch devices but accept it on a concrete child like
  // the canvas. The canvas is also the element we actually want to fill the
  // screen, so this is semantically right too.
  //
  // Failures surface to `console.warn` so they're visible in remote-debug
  // when something silently fails on a real device:
  //  - iOS Safari on iPhone has no Fullscreen API at all; those users can
  //    "Add to Home Screen" to get the same effect via the existing
  //    `apple-mobile-web-app-capable` meta tag.
  //  - Desktop browsers reject if the user previously declined the
  //    fullscreen prompt; we just leave them with the URL bar visible.
  let fullscreenAttempted = false;
  type FullscreenCapable = HTMLElement & {
    webkitRequestFullscreen?: (options?: FullscreenOptions) => Promise<void> | void;
  };
  const tryFullscreen = () => {
    if (fullscreenAttempted) return;
    fullscreenAttempted = true;
    if (document.fullscreenElement || (document as any).webkitFullscreenElement) {
      return;
    }
    const target = canvas as FullscreenCapable;
    const req = target.requestFullscreen?.bind(target)
      ?? target.webkitRequestFullscreen?.bind(target);
    if (!req) {
      console.warn("antidote: Fullscreen API not available on this browser");
      return;
    }
    // ONE synchronous call per user gesture — the second call wouldn't
    // count as user-activated and would be rejected anyway. Try with the
    // `navigationUI: "hide"` option (Android Chrome respects it to also
    // hide the system nav bar); if the call throws synchronously because
    // the options form isn't supported, fall back to the no-arg form
    // immediately so user activation is preserved.
    let promise: Promise<void> | void;
    try {
      promise = req({ navigationUI: "hide" });
    } catch (_err) {
      try {
        promise = req();
      } catch (err) {
        console.warn("antidote: requestFullscreen threw:", err);
        return;
      }
    }
    if (promise && typeof (promise as Promise<void>).catch === "function") {
      (promise as Promise<void>).catch((err) => {
        console.warn("antidote: fullscreen rejected:", err);
      });
    }
  };

  canvas.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    tryFullscreen();
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
  // Entering and exiting fullscreen also changes the available viewport;
  // the visualViewport resize sometimes fires before the new dimensions
  // settle, so explicitly re-sync on transition.
  document.addEventListener("fullscreenchange", resizeCanvas);
  document.addEventListener("webkitfullscreenchange", resizeCanvas);
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
