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

pub mod auth_widget;
pub mod game_model;
pub mod game_widget;
pub mod google_signin_button;
pub mod hud_widget;
pub mod leaderboard_widget;
pub mod life_lost_overlay;
pub mod menu_widget;
pub mod other_games_widget;
pub mod overlay_stack;
pub mod set_password_overlay;

use crate::db::auth::AuthClient;
use crate::db::inbox::{DbInboxEvent, Session};
use crate::game::state::Phase;
use auth_widget::SignInOverlay;
use set_password_overlay::SetPasswordOverlay;
#[cfg(test)]
use game_model::shared;
use game_model::{shared_with_config, MenuView, SharedModel, SupabaseConfig};
use game_widget::GameWidget;
use hud_widget::HudWidget;
use leaderboard_widget::LeaderboardOverlay;
use life_lost_overlay::LifeLostOverlay;
use menu_widget::{GameOverOverlay, LevelCompleteOverlay, MainMenuOverlay, PauseOverlay};
use other_games_widget::OtherGamesOverlay;
use overlay_stack::OverlayStack;

/// CascadiaCode is bundled into the binary so neither shell has to ship a
/// separate font file. ~388 KB; small enough for both native and wasm.
const FONT_BYTES: &[u8] = include_bytes!("../../../assets/CascadiaCode.ttf");

fn load_default_font() -> Arc<Font> {
    Arc::new(Font::from_slice(FONT_BYTES).expect("antidote default font"))
}

/// Build the shared Antidote application with the default Supabase config
/// (production publishable key + project URL). Returns `(App, SharedModel)`
/// — shells need the second value to drive `drain_db_inbox`,
/// `tick_score_sync`, `drain_pending_oauth`, and the OAuth-callback
/// session injection.
pub fn build_antidote_app() -> (App, SharedModel) {
    build_antidote_app_with_config(SupabaseConfig::antidote_default())
}

/// Same as [`build_antidote_app`] but with a caller-supplied Supabase
/// config. Tests + alternate deployments use this entry point.
pub fn build_antidote_app_with_config(config: SupabaseConfig) -> (App, SharedModel) {
    let model: SharedModel = shared_with_config(config);
    let font = load_default_font();

    let game_canvas = GameWidget::new(model.clone());
    let hud = HudWidget::new(model.clone(), font.clone());
    let main_menu = MainMenuOverlay::new(model.clone(), font.clone());
    let sign_in = SignInOverlay::new(model.clone(), font.clone());
    let set_password = SetPasswordOverlay::new(model.clone(), font.clone());
    let leaderboard = LeaderboardOverlay::new(model.clone(), font.clone());
    let other_games = OtherGamesOverlay::new(model.clone(), font.clone());
    let life_lost = LifeLostOverlay::new(model.clone(), font.clone());
    let level_complete = LevelCompleteOverlay::new(model.clone(), font.clone());
    let game_over = GameOverOverlay::new(model.clone(), font.clone());
    let pause = PauseOverlay::new(model.clone(), font);

    // Z-order matters here: front-to-back in painting, back-to-front in hit
    // testing. Game canvas is at the bottom; pause overlay (which the player
    // can summon at any time) sits on top so its buttons win over any other
    // overlay when phase happens to coincide. The sign-in / leaderboard /
    // other-games overlays live above the main menu so their buttons win
    // when their `MenuView` is active.
    let root = OverlayStack::new()
        .add(Box::new(game_canvas))
        .add(Box::new(hud))
        .add(Box::new(life_lost))
        .add(Box::new(main_menu))
        .add(Box::new(sign_in))
        .add(Box::new(set_password))
        .add(Box::new(leaderboard))
        .add(Box::new(other_games))
        .add(Box::new(level_complete))
        .add(Box::new(game_over))
        .add(Box::new(pause));

    let mut app = App::new(Box::new(root));

    // Esc/P toggles pause when in Playing or Paused; ignored otherwise.
    let key_model = model.clone();
    app.set_global_key_handler(move |key, _mods| toggle_pause_on_key(&key_model, &key));

    (app, model)
}

/// Drain the OAuth-button click signal, build a Supabase
/// `/auth/v1/authorize` URL for the requested provider, and stash it in
/// `pending_open_url` for the shell to actually navigate to. Call this
/// from each platform shell once per frame, passing the redirect URL
/// that's appropriate for that platform (e.g. on web the current page
/// origin, on native a `localhost:PORT` callback).
///
/// This stays out of `tick_score_sync` / `drain_db_inbox` because the
/// redirect URL is shell-specific — the core can't know it.
pub fn drain_pending_oauth(model: &SharedModel, redirect_to: &str) {
    let mut m = model.borrow_mut();
    let Some(provider) = m.auth.pending_oauth.take() else {
        return;
    };
    if m.services.config.url.is_empty() {
        m.auth.last_error = Some("Supabase URL not configured".to_owned());
        return;
    }
    let url = m.services.auth.oauth_url(provider, redirect_to);
    m.pending_open_url = Some(url);
}

/// Drain the "Forgot password?" click signal — calls
/// `/auth/v1/recover` with the email the user typed and the
/// shell-supplied `redirect_to` (the URL Supabase sends the user to in
/// the recovery email). Same shape as [`drain_pending_oauth`]: the email
/// is consumed, so the click only fires once.
pub fn drain_pending_password_reset(model: &SharedModel, redirect_to: &str) {
    let mut m = model.borrow_mut();
    let Some(email) = m.auth.pending_recover_email.take() else {
        return;
    };
    if m.services.config.url.is_empty() {
        m.auth.recover_pending = false;
        m.auth.last_error = Some("Supabase URL not configured".to_owned());
        return;
    }
    m.services
        .auth
        .request_password_reset_async(&email, redirect_to, &m.services.inbox);
}

/// Stash a recovery-flow access token (from a `type=recovery` callback
/// URL) without yet treating the user as fully signed in. The
/// `SetPasswordOverlay` reads this token, posts a new password to
/// `PUT /auth/v1/user`, and only THEN (in the `PasswordUpdated(Ok(_))`
/// drain branch) is the session installed for real. Until that point the
/// user can't navigate elsewhere — the recovery session is a single-use
/// token meant for resetting the password and nothing else.
pub fn record_recovery_token(
    model: &SharedModel,
    access_token: String,
    refresh_token: String,
    expires_in: i64,
) {
    let mut m = model.borrow_mut();
    m.auth.recovery_access_token = Some(access_token);
    m.auth.recovery_refresh_token = Some(refresh_token);
    m.auth.recovery_expires_in = Some(expires_in);
    m.auth.last_error = None;
    m.auth.notice = None;
    m.menu_view = MenuView::SetPassword;
}

/// Record a session that arrived via an OAuth redirect (web) or local
/// callback handler (native). Called by the platform shell after parsing
/// `#access_token=...&refresh_token=...&expires_in=...` out of the
/// callback URL. Pushes the same `DbInboxEvent::SignInResult(Ok(_))` the
/// email/password path uses, so the regular drain hook handles all the
/// downstream work (set bearer, navigate to Main, fetch games catalog).
pub fn record_oauth_session(
    model: &SharedModel,
    access_token: String,
    refresh_token: String,
    expires_in: i64,
) {
    let session = AuthClient::session_from_oauth_tokens(access_token, refresh_token, expires_in);
    let m = model.borrow();
    let _ = m
        .services
        .inbox
        .tx
        .send(DbInboxEvent::SignInResult(Ok(session)));
}

/// Manually inject a Session — used by tests and by storage-backed
/// "remember me" restores. Keeps the same downstream side effects as a
/// fresh sign-in.
#[allow(dead_code)]
pub fn restore_session(model: &SharedModel, session: Session) {
    let m = model.borrow();
    let _ = m
        .services
        .inbox
        .tx
        .send(DbInboxEvent::SignInResult(Ok(session)));
}

/// Push the current session's score to Supabase if the player just
/// finished a level or ran out of lives. Idempotent: tracks
/// `last_phase` + `last_synced_session_score` so the network call fires
/// exactly once per finalize transition, and only when the cumulative
/// session score has grown since the last sync.
///
/// Call once per frame, after `drain_db_inbox`, so the function sees the
/// freshest auth state (in case sign-in just landed). Cheap when not
/// signed in or when no transition occurred.
pub fn tick_score_sync(model: &SharedModel) {
    let mut m = model.borrow_mut();

    let phase = m.world.phase;
    let prev_phase = m.score_sync.last_phase;
    m.score_sync.last_phase = Some(phase);

    // Only act on the transition INTO LevelComplete or GameOver.
    let is_finalize = matches!(phase, Phase::LevelComplete | Phase::GameOver);
    let just_entered = is_finalize && prev_phase != Some(phase);
    if !just_entered {
        return;
    }
    if m.auth.session.is_none() {
        return;
    }

    let total_score = m.world.total_score;
    let already_synced = m.score_sync.last_synced_session_score;
    if total_score <= already_synced {
        return;
    }
    let delta = (total_score - already_synced) as i32;

    // Need a known game_id to call the RPC. If we don't have the games
    // catalog cached yet, fetch it now; we'll retry on the next finalize
    // (a future GameOver, etc.).
    let Some(game_id) = m.cached_game_id() else {
        if !m.menu_caches.games_pending && m.menu_caches.games.is_none() {
            m.menu_caches.games_pending = true;
            m.services.postgrest.list_games_async(&m.services.inbox);
        }
        return;
    };

    m.services.postgrest.add_game_score_async(&game_id, delta);
    m.score_sync.last_synced_session_score = total_score;
}

/// Drain queued REST responses from the db inbox into the shared model.
/// Call once per frame from the platform shell (or from a low-stack widget
/// that is guaranteed to paint every frame). Idempotent and cheap when the
/// inbox is empty.
pub fn drain_db_inbox(model: &SharedModel) {
    // Snapshot drained events under a short borrow so callbacks down below
    // (which take borrow_mut) don't collide.
    let mut events: Vec<DbInboxEvent> = Vec::new();
    {
        let m = model.borrow();
        while let Ok(e) = m.services.inbox.rx.try_recv() {
            events.push(e);
        }
    }
    if events.is_empty() {
        return;
    }
    let mut m = model.borrow_mut();
    for e in events {
        match e {
            DbInboxEvent::SignInResult(Ok(s)) | DbInboxEvent::SignUpResult(Ok(s)) => {
                m.services
                    .postgrest
                    .set_access_token(Some(s.access_token.clone()));
                m.auth.record_session(s);
                // Successful sign-in returns the user to the main menu so
                // they see their email + Sign-out button.
                m.menu_view = MenuView::Main;
                // Future: persist via `Storage::save_session(&s)`.
                // Pre-fetch the games catalog so `tick_score_sync` and the
                // leaderboard / other-games overlays don't have to wait
                // for an extra round-trip when the player first reaches a
                // level-complete or opens those panels.
                if m.menu_caches.games.is_none() && !m.menu_caches.games_pending {
                    m.menu_caches.games_pending = true;
                    m.services.postgrest.list_games_async(&m.services.inbox);
                }
            }
            DbInboxEvent::SignInResult(Err(err)) | DbInboxEvent::SignUpResult(Err(err)) => {
                m.auth.record_error(err);
            }
            DbInboxEvent::GamesList(Ok(games)) => {
                m.menu_caches.games = Some(games);
                m.menu_caches.games_pending = false;
                m.menu_caches.games_error = None;
            }
            DbInboxEvent::GamesList(Err(err)) => {
                m.menu_caches.games_pending = false;
                m.menu_caches.games_error = Some(err);
            }
            DbInboxEvent::TopScoresList(Ok(rows)) => {
                m.menu_caches.top_scores = Some(rows);
                m.menu_caches.top_scores_pending = false;
                m.menu_caches.top_scores_error = None;
            }
            DbInboxEvent::TopScoresList(Err(err)) => {
                m.menu_caches.top_scores_pending = false;
                m.menu_caches.top_scores_error = Some(err);
            }
            DbInboxEvent::PasswordResetRequested(Ok(email)) => {
                m.auth.recover_pending = false;
                m.auth.last_error = None;
                m.auth.notice = Some(format!(
                    "Reset link sent to {email}. Check your inbox, then come back."
                ));
            }
            DbInboxEvent::PasswordResetRequested(Err(err)) => {
                m.auth.recover_pending = false;
                m.auth.notice = None;
                m.auth.last_error = Some(format!("Reset failed: {err}"));
            }
            DbInboxEvent::PasswordUpdated(Ok(())) => {
                // Promote the recovery tokens into a real Session and install
                // them through the same code path email/password uses.
                m.auth.pending = false;
                let access = m.auth.recovery_access_token.take().unwrap_or_default();
                let refresh = m.auth.recovery_refresh_token.take().unwrap_or_default();
                let expires_in = m.auth.recovery_expires_in.take().unwrap_or(3600);
                m.auth.last_error = None;
                m.auth.notice = Some("Password updated. You're signed in.".to_owned());
                let session = AuthClient::session_from_oauth_tokens(access, refresh, expires_in);
                m.services
                    .postgrest
                    .set_access_token(Some(session.access_token.clone()));
                m.auth.record_session(session);
                // record_session clears the notice we just set; restore it.
                m.auth.notice = Some("Password updated. You're signed in.".to_owned());
                m.menu_view = MenuView::Main;
                if m.menu_caches.games.is_none() && !m.menu_caches.games_pending {
                    m.menu_caches.games_pending = true;
                    m.services.postgrest.list_games_async(&m.services.inbox);
                }
            }
            DbInboxEvent::PasswordUpdated(Err(err)) => {
                m.auth.pending = false;
                m.auth.notice = None;
                m.auth.last_error = Some(format!("Couldn't set password: {err}"));
            }
        }
    }
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
}
