//! Guardrail for Antidote's platform-shell split.
//!
//! Native and WASM crates may wire windows, canvases, render surfaces, input,
//! and persistence, but visible game/UI behavior belongs in `antidote-core`.

use std::fs;
use std::path::Path;

const SHELL_CRATES: &[&str] = &["antidote-native/src", "antidote-wasm/src"];

const FORBIDDEN_SNIPPETS: &[&str] = &[
    "antidote_core::game::",
    "antidote_core::render::",
    "antidote_core::ui::auth_widget",
    "antidote_core::ui::game_widget",
    "antidote_core::ui::leaderboard_widget",
    "antidote_core::ui::menu_widget",
    "antidote_core::ui::other_games_widget",
    "GameWidget::",
    "AuthWidget::",
    "LeaderboardWidget::",
    "MenuWidget::",
    "OtherGamesWidget::",
];

#[test]
fn platform_shells_do_not_construct_game_or_ui_content() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("antidote-core crate should live under the workspace root");

    let mut violations = Vec::new();
    for shell in SHELL_CRATES {
        visit_rs_files(&workspace_root.join(shell), &mut |path| {
            let text = fs::read_to_string(path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
            for forbidden in FORBIDDEN_SNIPPETS {
                if text.contains(forbidden) {
                    violations.push((path.to_path_buf(), *forbidden));
                }
            }
        });
    }

    if !violations.is_empty() {
        violations.sort();
        let details = violations
            .into_iter()
            .map(|(path, forbidden)| {
                let rel = path.strip_prefix(workspace_root).unwrap_or(&path);
                format!("{} contains `{forbidden}`", rel.display())
            })
            .collect::<Vec<_>>()
            .join("\n");
        panic!(
            "platform shells must not construct game/UI content directly; use shared antidote-core builders instead:\n{details}"
        );
    }
}

fn visit_rs_files(dir: &Path, on_file: &mut impl FnMut(&Path)) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read directory {}: {err}", dir.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| {
            panic!("failed to read directory entry in {}: {err}", dir.display());
        });
        let path = entry.path();
        let file_type = entry
            .file_type()
            .unwrap_or_else(|err| panic!("failed to read file type for {}: {err}", path.display()));
        if file_type.is_dir() {
            visit_rs_files(&path, on_file);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            on_file(&path);
        }
    }
}
