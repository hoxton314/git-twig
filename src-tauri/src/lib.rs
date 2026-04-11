mod commands;
mod error;
mod git;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Repo management
            commands::repo::open_repo,
            commands::repo::close_repo,
            commands::repo::get_repo_info,
            commands::repo::list_open_repos,
            // Commit graph
            commands::graph::get_commit_graph,
            // Branches
            commands::branches::get_branches,
            commands::branches::checkout_branch,
            commands::branches::create_branch,
            commands::branches::rename_branch,
            commands::branches::delete_branch,
            commands::branches::push_branch,
            commands::branches::fetch_all,
            // Diffs
            commands::diff::get_commit_diff,
            commands::diff::get_working_diff,
            // Staging & working directory
            commands::staging::get_working_status,
            commands::staging::get_staged_diff,
            commands::staging::get_unstaged_diff,
            commands::staging::stage_files,
            commands::staging::unstage_files,
            commands::staging::create_commit,
            commands::staging::pull,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Twig");
}
