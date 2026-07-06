//! Shared Antidote widget tree and UI builders.
//!
//! Native and WASM shells must build the game through this module instead of
//! constructing widgets directly. Platform crates own only OS/browser wiring;
//! every game screen, menu, layout, and widget tree lives here.
//!
//! Every widget shares ownership of the live game state through
//! [`game_model::SharedModel`] — an `Rc<RefCell<GameModel>>`. Widgets borrow
//! the model briefly inside paint / event handlers; no widget keeps the
//! borrow live across boundaries.

use std::sync::Arc;

use agg_gui::text::Font;
use agg_gui::{App, Key};

pub mod canvas_root;
pub mod file_overlay;
pub mod game_model;
pub mod game_widget;
pub mod help_overlay;
pub mod hud_widget;
pub mod life_lost_overlay;
pub mod menu_bar;
pub mod menu_widget;
pub mod overlay_stack;
pub mod paint_util;
pub mod rotate_overlay;

use crate::game::state::Phase;
use crate::platform::{in_memory_settings_store, SettingsStore};
use canvas_root::CanvasRoot;
pub use canvas_root::fixed_canvas_ux_scale;
use file_overlay::FileOverlay;
#[cfg(test)]
use game_model::shared;
use game_model::{shared_with_store, SharedModel};
use game_widget::GameWidget;
use help_overlay::HelpOverlay;
use hud_widget::HudWidget;
use life_lost_overlay::LifeLostOverlay;
use menu_bar::MenuBar;
use menu_widget::{GameOverOverlay, LevelCompleteOverlay, MainMenuOverlay, PauseOverlay};
use overlay_stack::OverlayStack;
use rotate_overlay::RotateOverlay;


/// Build the shared Antidote application with an in-memory settings store.
/// Tests use this; production shells pass a real platform-backed store via
/// [`build_antidote_app_with_store`].
pub fn build_antidote_app() -> (App, SharedModel) {
    build_antidote_app_with_store(in_memory_settings_store())
}

/// Build the shared Antidote application with a caller-supplied settings
/// store. Returns `(App, SharedModel)` — shells keep the model handle so
/// they can drive their own per-frame hooks.
pub fn build_antidote_app_with_store(store: Arc<dyn SettingsStore>) -> (App, SharedModel) {
    let model: SharedModel = shared_with_store(store);
    let fonts = crate::theme::Fonts::load();
    // Interim single-face handle for widgets not yet migrated to per-face
    // `theme::Fonts` fields; SemiBold reads well at label and body sizes.
    let font: Arc<Font> = fonts.semibold.clone();

    let game_canvas = GameWidget::new(model.clone());
    let hud = HudWidget::new(model.clone(), fonts.clone());
    let main_menu = MainMenuOverlay::new(model.clone(), font.clone());
    let menu_bar = MenuBar::new(model.clone(), font.clone());
    let file_overlay = FileOverlay::new(model.clone(), font.clone());
    let help_overlay = HelpOverlay::new(model.clone(), font.clone());
    let life_lost = LifeLostOverlay::new(model.clone(), fonts.clone());
    let level_complete = LevelCompleteOverlay::new(model.clone(), font.clone());
    let game_over = GameOverOverlay::new(model.clone(), font.clone());
    let pause = PauseOverlay::new(model.clone(), font.clone());
    let rotate = RotateOverlay::new(model.clone(), font);

    // Z-order matters here: front-to-back in painting, back-to-front in hit
    // testing. Game canvas is at the bottom; pause overlay (which the player
    // can summon at any time) sits on top so its buttons win over any other
    // overlay when phase happens to coincide. The menu bar sits ABOVE the
    // main-menu overlay so its top-strip buttons receive clicks before the
    // main-menu backdrop swallows them. The rotate-device prompt is topmost
    // of all — when a mobile device is in portrait, nothing else matters.
    let stack = OverlayStack::new()
        .add(Box::new(game_canvas))
        .add(Box::new(hud))
        .add(Box::new(life_lost))
        .add(Box::new(main_menu))
        .add(Box::new(menu_bar))
        .add(Box::new(file_overlay))
        .add(Box::new(help_overlay))
        .add(Box::new(level_complete))
        .add(Box::new(game_over))
        .add(Box::new(pause))
        .add(Box::new(rotate));

    // The whole app is authored at the fixed 1280×720 canvas; CanvasRoot
    // centers it and paints the letterbox bars. Shells keep the scale right
    // by feeding `fixed_canvas_ux_scale` into `set_ux_scale` on resize.
    let root = CanvasRoot::new(Box::new(stack));

    let mut app = App::new(Box::new(root));

    // Esc/P toggles pause when in Playing or Paused; ignored otherwise.
    let key_model = model.clone();
    app.set_global_key_handler(move |key, _mods| toggle_pause_on_key(&key_model, &key));

    (app, model)
}

/// Returns true when this key produced a phase change.
pub(crate) fn toggle_pause_on_key(model: &SharedModel, key: &Key) -> bool {
    let is_pause_key = match key {
        Key::Escape => true,
        Key::Char(c) => c.eq_ignore_ascii_case(&'p'),
        _ => false,
    };
    if !is_pause_key {
        return false;
    }
    let mut m = model.borrow_mut();
    match m.world.phase {
        Phase::Playing => {
            m.world.phase = Phase::Paused;
            true
        }
        Phase::Paused => {
            m.world.phase = Phase::Playing;
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::level::init_level;
    use crate::game::physics::PhysicsWorld;
    use crate::game::state::{Phase, World};
    use crate::game::update::{tick, LIFE_LOST_DURATION};

    /// Esc/P should flip Playing ↔ Paused, and be a no-op in other phases.
    #[test]
    fn esc_and_p_toggle_pause() {
        let model = shared();
        // From Playing → Paused on Esc.
        model.borrow_mut().world.phase = Phase::Playing;
        assert!(toggle_pause_on_key(&model, &Key::Escape));
        assert_eq!(model.borrow().world.phase, Phase::Paused);

        // Paused → Playing on lowercase 'p'.
        assert!(toggle_pause_on_key(&model, &Key::Char('p')));
        assert_eq!(model.borrow().world.phase, Phase::Playing);

        // Uppercase 'P' should also toggle.
        assert!(toggle_pause_on_key(&model, &Key::Char('P')));
        assert_eq!(model.borrow().world.phase, Phase::Paused);

        // Other keys do nothing.
        assert!(!toggle_pause_on_key(&model, &Key::Enter));
        assert_eq!(model.borrow().world.phase, Phase::Paused);
    }

    #[test]
    fn pause_no_op_when_not_in_playing_or_paused() {
        let model = shared();
        for phase in [
            Phase::Start,
            Phase::LevelComplete,
            Phase::LifeLost,
            Phase::GameOver,
        ] {
            model.borrow_mut().world.phase = phase;
            assert!(!toggle_pause_on_key(&model, &Key::Escape));
            assert_eq!(model.borrow().world.phase, phase);
        }
    }

    #[test]
    fn paused_phase_freezes_physics() {
        // Start a level so viruses are moving, then pause and confirm
        // they don't drift over a 1-second simulated tick.
        let mut world = World::new();
        let mut physics =
            PhysicsWorld::new(crate::consts::VIRTUAL_WIDTH, crate::consts::VIRTUAL_HEIGHT);
        world.level = 1;
        init_level(&mut world, &mut physics);
        let v0 = world.viruses[0];
        world.phase = Phase::Paused;

        // Run 60 fixed-timestep ticks (1 simulated second).
        for _ in 0..60 {
            tick(&mut world, &mut physics, 1.0 / 60.0);
        }

        let v1 = world.viruses[0];
        // Position must not have moved at all while paused.
        assert!((v1.x - v0.x).abs() < 1e-4, "x drifted by {}", v1.x - v0.x);
        assert!((v1.y - v0.y).abs() < 1e-4, "y drifted by {}", v1.y - v0.y);
    }

    #[test]
    fn level_start_score_snapshots_total_score() {
        let mut world = World::new();
        let mut physics =
            PhysicsWorld::new(crate::consts::VIRTUAL_WIDTH, crate::consts::VIRTUAL_HEIGHT);
        world.total_score = 250;
        world.level = 3;
        init_level(&mut world, &mut physics);
        assert_eq!(world.level_start_score, 250);
        // Simulate trapping a virus mid-level.
        world.total_score += 100;
        assert_eq!(world.current_level_score(), 100);
    }

    /// `start_new_game` from Phase::GameOver must wipe lives/level/score back
    /// to defaults — this is what GameOverOverlay's "Play again" button calls.
    #[test]
    fn start_new_game_resets_lives_level_and_score() {
        use crate::consts::BASE_LIVES;
        use crate::game::update::start_new_game;
        let mut world = World::new();
        let mut physics =
            PhysicsWorld::new(crate::consts::VIRTUAL_WIDTH, crate::consts::VIRTUAL_HEIGHT);
        world.lives = 0;
        world.level = 5;
        world.total_score = 999;
        world.phase = Phase::GameOver;

        start_new_game(&mut world, &mut physics);

        assert_eq!(world.lives, BASE_LIVES);
        assert_eq!(world.level, 1);
        assert_eq!(world.total_score, 0);
        assert_eq!(world.phase, Phase::Playing);
        assert!(!world.viruses.is_empty(), "level 1 should spawn a virus");
    }

    /// `advance_to_next_level` from Phase::LevelComplete bumps level and
    /// preserves total_score — the LevelCompleteOverlay's "Next level"
    /// button calls this.
    #[test]
    fn advance_to_next_level_bumps_level_keeps_score() {
        use crate::game::update::advance_to_next_level;
        let mut world = World::new();
        let mut physics =
            PhysicsWorld::new(crate::consts::VIRTUAL_WIDTH, crate::consts::VIRTUAL_HEIGHT);
        world.level = 2;
        world.total_score = 300;
        world.phase = Phase::LevelComplete;

        advance_to_next_level(&mut world, &mut physics);

        assert_eq!(world.level, 3);
        assert_eq!(world.total_score, 300);
        assert_eq!(world.phase, Phase::Playing);
        // level_start_score should have been re-snapshotted to current total.
        assert_eq!(world.level_start_score, 300);
    }

    /// When a life is lost, the death position is captured for the
    /// LifeLostOverlay's float-up animation. Sets up a virus right next to
    /// a freshly-clicked bubble so the collision fires on the very next tick.
    #[test]
    fn life_lost_records_death_position() {
        use crate::consts::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH, VIRUS_RADIUS};
        use crate::game::update::on_pointer_down;
        let mut world = World::new();
        let mut physics = PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);
        init_level(&mut world, &mut physics);

        // Move the lone virus to a known position and zero its velocity. The
        // physics step + maintain_virus_speeds may bump it later, but we'll
        // pop the bubble before it has a chance to drift.
        world.viruses[0].x = 200.0;
        world.viruses[0].y = 300.0;
        world.viruses[0].vx = 0.0;
        world.viruses[0].vy = 0.0;
        if let Some(h) = world.viruses[0].body {
            physics.set_body_position(h, 200.0, 300.0);
        }

        // Click at (400, 300) — well clear of the virus so the bubble starts.
        on_pointer_down(&mut world, &mut physics, 400.0, 300.0);
        assert!(
            world.growing.is_some(),
            "growing bubble should have started"
        );

        // Manually move the virus into the bubble so the next tick's
        // collision check sees an overlap. We use just-inside the bubble's
        // initial small radius + VIRUS_RADIUS to guarantee it.
        world.viruses[0].x = 400.0 + VIRUS_RADIUS * 0.5;
        world.viruses[0].y = 300.0;
        if let Some(h) = world.viruses[0].body {
            physics.set_body_position(h, world.viruses[0].x, world.viruses[0].y);
        }

        tick(&mut world, &mut physics, 1.0 / 60.0);

        assert_eq!(world.phase, Phase::LifeLost);
        let (dx, dy) = world.last_life_lost_at.expect("death pos recorded");
        // Death is recorded at the popped bubble center, ~(400, 300).
        assert!((dx - 400.0).abs() < 5.0, "death x = {dx}");
        assert!((dy - 300.0).abs() < 5.0, "death y = {dy}");

        // Wait out the LifeLost duration; transitioning back to Playing via
        // init_level must clear last_life_lost_at.
        world.lives = 2;
        for _ in 0..((LIFE_LOST_DURATION * 60.0) as usize + 5) {
            tick(&mut world, &mut physics, 1.0 / 60.0);
        }
        assert_eq!(world.phase, Phase::Playing);
        assert!(world.last_life_lost_at.is_none());
    }

    /// Beating the previous best score must persist via the
    /// `SettingsStore` trait so the next session starts with the new high.
    #[test]
    fn best_score_persists_when_session_beats_record() {
        let model = shared();
        model.borrow_mut().world.total_score = 500;
        model.borrow_mut().maybe_record_best_score();
        assert_eq!(model.borrow().settings.best_score, 500);
        // Smaller subsequent score must not overwrite.
        model.borrow_mut().world.total_score = 100;
        model.borrow_mut().maybe_record_best_score();
        assert_eq!(model.borrow().settings.best_score, 500);
    }

    /// `record_finished_session` should append to `recent_scores`,
    /// drop the oldest entry past the cap, and ignore zero-score
    /// abandons.
    #[test]
    fn recent_scores_capped_and_ordered_most_recent_first() {
        let model = shared();
        for n in 1..=10u64 {
            model
                .borrow_mut()
                .record_finished_session(n * 100, n as u32);
        }
        let m = model.borrow();
        // Cap is 8 — first two recordings (100/200) should have fallen off.
        assert_eq!(m.settings.recent_scores.len(), 8);
        // Most recent first.
        assert_eq!(m.settings.recent_scores[0].score, 1000);
        assert_eq!(m.settings.recent_scores[0].level, 10);
        // The newest in-range entry that survived is score=300, level=3.
        assert_eq!(m.settings.recent_scores.last().unwrap().score, 300);
        // Best score = highest recorded.
        assert_eq!(m.settings.best_score, 1000);
        drop(m);

        // Zero-score abandons should not show up.
        let len_before = model.borrow().settings.recent_scores.len();
        model.borrow_mut().record_finished_session(0, 1);
        assert_eq!(model.borrow().settings.recent_scores.len(), len_before);
    }

    /// `record_saved_session` / `clear_saved_session` round-trip drives
    /// the Resume button's visibility on the main menu.
    #[test]
    fn saved_session_round_trip() {
        let model = shared();
        {
            let mut m = model.borrow_mut();
            m.world.level = 4;
            m.world.total_score = 250;
            m.world.lives = 2;
            m.record_saved_session();
        }
        let snap = model.borrow().settings.saved_session.clone();
        assert!(snap.is_some());
        let snap = snap.unwrap();
        assert_eq!(snap.level, 4);
        assert_eq!(snap.total_score, 250);
        assert_eq!(snap.lives, 2);

        model.borrow_mut().clear_saved_session();
        assert!(model.borrow().settings.saved_session.is_none());
    }
}
