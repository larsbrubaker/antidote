use serde::{Deserialize, Serialize};

/// One row of the public `leaderboard` view. The view hides `user_id` UUIDs
/// behind the per-user `handle` chosen at signup; see
/// `db/migrations/0004_handles_and_leaderboard_view.sql`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub game_slug: String,
    pub handle: String,
    pub high_score: i32,
    pub total_score: i64,
    pub plays: i32,
    pub last_played: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub deploy_url: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserScore {
    pub user_id: String,
    pub game_id: String,
    pub high_score: i32,
    pub total_score: i64,
    pub plays: i32,
    pub last_played: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProgress {
    pub user_id: String,
    pub game_id: String,
    pub current_level: i32,
    pub lives_remaining: i32,
    pub state: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    pub user_id: String,
    pub game_id: String,
    pub settings: serde_json::Value,
}
