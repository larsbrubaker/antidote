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
  // (`canvas.{width,height}`). Match Solitaire (the known-working
  // mobile sibling): read dimensions from
  // `documentElement.clientWidth/Height` rather than `visualViewport`
  // or `innerWidth/innerHeight`. `clientWidth/Height` reflect the
  // actual laid-out viewport; the other two race with the address bar
  // collapse on Android Chrome and reported 0 on the user's Pixel,
  // which is what was leaving the canvas blank.
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
    const root = document.documentElement;
    const cssWidth = root.clientWidth;
    const cssHeight = root.clientHeight;
    if (cssWidth === 0 || cssHeight === 0) return;
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

  // Mobile: on the first tap, ask the browser for fullscreen so the
  // URL/address bar disappears and the playfield gets the entire
  // viewport. Required to be called from a user gesture; we hook it
  // into the canvas pointerdown handler. No-ops if already fullscreen,
  // or if the device isn't touch-capable, or if requestFullscreen
  // isn't supported (iOS Safari has its own bag of quirks — there a
  // separate "Add to Home Screen" launch is the way to remove the URL
  // bar). Mirrors Solitaire, which is the known-working mobile
  // sibling: target `document.documentElement` (the canvas-targeted
  // variant we tried earlier didn't activate on the user's Pixel) and
  // reset `fullscreenAttempted` on rejection so a future tap can
  // retry.
  let fullscreenAttempted = false;
  const maybeRequestFullscreen = () => {
    if (fullscreenAttempted) return;
    if (document.fullscreenElement) {
      fullscreenAttempted = true;
      return;
    }
    const isTouch =
      (navigator.maxTouchPoints ?? 0) > 0 || "ontouchstart" in window;
    if (!isTouch) return;
    fullscreenAttempted = true;
    const el = document.documentElement as HTMLElement & {
      webkitRequestFullscreen?: () => Promise<void>;
    };
    const req = el.requestFullscreen ?? el.webkitRequestFullscreen;
    if (!req) return;
    Promise.resolve(req.call(el)).catch(() => {
      // User denied or browser doesn't allow it on this gesture — let
      // a future tap try again.
      fullscreenAttempted = false;
    });
  };

  canvas.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    maybeRequestFullscreen();
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

  // Drive layout from BOTH the `resize` event (for changes after first
  // paint) AND a requestAnimationFrame retry loop that runs until the
  // viewport reports a non-zero size. The retry guards against a race
  // where wasm finished loading before the host iframe got laid out —
  // happens reliably in Vite's preview iframe and intermittently on
  // Android Chrome when the URL bar is mid-collapse. ResizeObserver is
  // unreliable here (does not fire its initial observation in some
  // iframe contexts), so we don't depend on it.
  window.addEventListener("resize", resizeCanvas);
  document.addEventListener("fullscreenchange", resizeCanvas);
  document.addEventListener("webkitfullscreenchange", resizeCanvas);
  const tryInitialSize = () => {
    const root = document.documentElement;
    if (root.clientWidth > 0 && root.clientHeight > 0) {
      resizeCanvas();
      return;
    }
    requestAnimationFrame(tryInitialSize);
  };
  tryInitialSize();

  // File → Export… in the in-game menu sets `model.pending_export`. Drain
  // it each frame; when it flips true, fetch the JSON from wasm, wrap in a
  // Blob, and offer it as `antidote-save.json` via a transient anchor.
  const drainExport = () => {
    const json = wasm.drain_pending_export?.();
    if (typeof json !== "string") return;
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "antidote-save.json";
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  };

  // File → Import… sets `model.pending_import`. When drained, open a hidden
  // file input; the user-gesture context from the menu click carries through
  // because the wasm `pending_import` flag was set inside the same gesture
  // and we drain it on the very next animation frame.
  const drainImport = () => {
    if (!wasm.drain_pending_import?.()) return;
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "application/json,.json";
    input.style.display = "none";
    input.addEventListener("change", async () => {
      const file = input.files?.[0];
      if (!file) return;
      try {
        const text = await file.text();
        const ok = wasm.apply_settings_json?.(text);
        if (!ok) console.warn("antidote: import failed to parse JSON");
      } catch (err) {
        console.warn("antidote: import read error", err);
      } finally {
        document.body.removeChild(input);
      }
    });
    document.body.appendChild(input);
    input.click();
  };

  let last = performance.now();
  const frame = (now: number) => {
    const frameMs = now - last;
    last = now;
    if (wasm.needs_draw()) {
      wasm.render(canvas.width, canvas.height, frameMs);
    }
    drainExport();
    drainImport();
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}

main().catch((err) => {
  console.error(err);
});
