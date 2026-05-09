//! Shared Antidote widget tree and UI builders.
//!
//! Native and WASM shells must build the game through this module instead of
//! constructing game widgets directly. Platform crates own only OS/browser
//! wiring; every game screen, menu, layout, and widget tree lives here.

use agg_gui::App;

pub mod auth_widget;
pub mod game_widget;
pub mod leaderboard_widget;
pub mod menu_widget;
pub mod other_games_widget;

/// Build the shared Antidote application.
///
/// This is the single entry point platform shells use for the game UI. As menus,
/// auth, leaderboard, and game-over screens grow, they should be composed here
/// or in sibling `antidote-core::ui` modules, never in `antidote-native` or
/// `antidote-wasm`.
pub fn build_antidote_app() -> App {
    App::new(Box::new(game_widget::GameWidget::new()))
}
