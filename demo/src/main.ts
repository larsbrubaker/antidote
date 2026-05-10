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
