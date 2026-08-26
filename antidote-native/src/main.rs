//! Native shell for the Antidote game.
//!
//! # Platform-split policy (kept identical across `antidote-native`, `antidote-wasm`)
//!
//! This crate is a **platform shell only** — window, event loop, input
//! forwarding, wgpu present and device-loss recovery are all `agg-gui-shell`'s;
//! GPU rendering is `agg-gui-wgpu`'s. It contains **no game or UI content**:
//! every game rule, widget tree, menu, layout, and interface the user sees is
//! shared via `antidote-core` (game logic + widget tree).
//!
//! - **Game / widget / layout code** → `antidote-core`
//! - **GPU renderers (WGSL shaders, geometry, draw calls)** → `agg-gui-wgpu`
//! - **Window, event loop, input, present** → `agg-gui-shell`
//! - **What is left here** → disk-backed state persistence
//!   ([`platform::FileSettingsStore`]) and the [`AntidoteHost`] hooks: the
//!   fixed-canvas UX scale, the paint-cost logger, and opening hyperlinks in
//!   the default browser.
//!
//! Future screens, menus, HUDs, dialogs, leaderboards, and gameplay UI must be
//! added to `antidote-core`, not here.

use std::time::Instant;

use agg_gui::{App, Size};
use agg_gui_shell::{
    run, Frame, RedrawPolicy, ShellConfig, ShellControl, ShellError, ShellHost, WgpuGfxCtx,
};
use antidote_core::ui::{build_antidote_app_with_store, game_model::SharedModel};

mod platform;

use platform::FileSettingsStore;

fn main() -> Result<(), ShellError> {
    let config = ShellConfig::new("Antidote")
        .with_logical_size(800.0, 600.0)
        // Continuous animation — the game redraws every frame, matching the
        // old hand-rolled loop's unconditional request_redraw.
        .with_redraw_policy(RedrawPolicy::Continuous)
        .with_device_label("antidote-native-wgpu");

    run(config, |_init| {
        let (app, model) = build_antidote_app_with_store(FileSettingsStore::into_shared());
        let host = AntidoteHost {
            model,
            frame_count: 0,
            paint_time_sum_ms: 0.0,
        };
        Ok((app, host))
    })
}

/// App-specific shell hooks: the fixed-canvas UX scale, the light-touch
/// paint-cost logger, and the pending-URL drain (Help panel's SOURCE link).
struct AntidoteHost {
    model: SharedModel,
    /// Light-touch frame-time logger. Sums *paint work* only — a stopwatch
    /// around layout+paint, NOT wall-clock between log dumps, so vsync sleeps
    /// and surface acquire don't get reported as render cost. Prints once
    /// every 60 painted frames.
    frame_count: u32,
    paint_time_sum_ms: f64,
}

impl ShellHost for AntidoteHost {
    fn paint(&mut self, app: &mut App, ctx: &mut WgpuGfxCtx, frame: &Frame) {
        let work_start = Instant::now();
        ctx.reset(frame.width as f32, frame.height as f32);
        ctx.set_lcd_mode(agg_gui::font_settings::lcd_enabled());
        // Fixed 1280×720 design canvas: fit it to the window every frame so
        // the whole tree lays out in design units (CanvasRoot centers the
        // slack). Layout runs unconditionally — the game animates every frame.
        agg_gui::ux_scale::set_ux_scale(antidote_core::ui::fixed_canvas_ux_scale(
            frame.width as f64,
            frame.height as f64,
        ));
        app.layout(Size::new(frame.width as f64, frame.height as f64));
        app.paint(ctx);

        self.paint_time_sum_ms += work_start.elapsed().as_secs_f64() * 1000.0;
        self.frame_count += 1;
        if self.frame_count >= 60 {
            let avg_ms = self.paint_time_sum_ms / self.frame_count as f64;
            eprintln!("antidote: {avg_ms:.2} ms/frame paint cost");
            self.frame_count = 0;
            self.paint_time_sum_ms = 0.0;
        }
    }

    fn on_idle(&mut self, _app: &mut App, _control: &mut ShellControl<'_>) {
        // Hyperlink clicks (Help panel's SOURCE line) → default browser.
        // Same drain-each-frame pattern as the wasm shell.
        if let Some(url) = self.model.borrow_mut().pending_open_url.take() {
            open_in_browser(&url);
        }
    }
}

/// Open `url` in the system default browser. Best-effort: failures are
/// logged, not fatal — the URL is also visible on-screen for hand-typing.
fn open_in_browser(url: &str) {
    // Only ever launch plain https URLs; anything else smells like
    // argument injection into the shell below.
    if !url.starts_with("https://") {
        eprintln!("antidote: refusing to open non-https URL: {url}");
        return;
    }
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(err) = result {
        eprintln!("antidote: failed to open {url} in a browser: {err}");
    }
}
