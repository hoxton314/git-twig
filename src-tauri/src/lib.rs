mod commands;
mod credentials;
mod error;
mod git;
mod github;
// Hosting integrations
mod hosting;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    commands::cli::record_startup_args();
    tauri::Builder::default()
        // Must be the first plugin: a second launch exits here and forwards
        // its arguments to the running instance.
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            commands::cli::on_second_instance(app, args, cwd);
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Repo management
            commands::repo::open_repo,
            commands::cli::take_pending_paths,
            commands::repo::init_repository,
            commands::repo::clone_repository,
            commands::repo::close_repo,
            commands::repo::get_repo_info,
            commands::repo::list_open_repos,
            commands::repo::list_repos_in_dir,
            // Commit graph
            commands::graph::get_commit_graph,
            // Branches
            commands::branches::get_branches,
            commands::branches::checkout_branch,
            commands::branches::checkout_remote_branch,
            commands::branches::create_branch,
            commands::branches::rename_branch,
            commands::branches::delete_branch,
            commands::branches::delete_remote_branch,
            commands::branches::push_branch,
            commands::branches::merge_branch,
            commands::branches::fetch_all,
            // Diffs
            commands::diff::get_commit_diff,
            commands::diff::get_compare_diff,
            commands::diff::get_working_diff,
            commands::diff::get_file_blob,
            // Staging & working directory
            commands::staging::get_working_status,
            commands::staging::get_staged_diff,
            commands::staging::get_unstaged_diff,
            commands::staging::stage_files,
            commands::staging::unstage_files,
            commands::staging::undo_commit,
            commands::staging::discard_files,
            commands::staging::create_commit,
            commands::staging::pull,
            // Session persistence
            commands::session::save_session,
            commands::session::load_session,
            // Settings
            commands::settings::load_settings,
            commands::settings::save_settings,
            // Git config
            commands::git_config::get_git_config,
            commands::git_config::set_git_config,
            // Stash
            commands::stash::stash_list,
            commands::stash::stash_push,
            // GitHub
            commands::github::github_validate_token,
            commands::github::github_set_token,
            commands::github::github_has_token,
            commands::github::github_list_repos,
            commands::github::github_clone_repo,
            commands::github::github_create_repo,
            commands::github::github_detect_remote,
            commands::github::github_create_pull_request,
            commands::github::github_list_branches,
            // Window
            commands::window::is_tiling_wm,
            commands::updater::updater_supported,
            // Commit context menu & undo history (reflog)
            commands::commit_ops::checkout_commit,
            commands::commit_ops::cherry_pick_commit,
            commands::commit_ops::cherry_pick_commits,
            commands::commit_ops::revert_commit,
            commands::commit_ops::reset_to_commit,
            commands::commit_ops::get_head_reflog,
            commands::commit_ops::restore_head,
            // Tags
            commands::tags::get_tags,
            commands::tags::create_tag,
            commands::tags::delete_tag,
            commands::tags::push_tag,
            commands::tags::delete_remote_tag,
            // Staging panel: commit helpers & file actions
            commands::commit_tools::create_commit_with_options,
            commands::commit_tools::get_head_commit_info,
            commands::commit_tools::get_recent_authors,
            commands::commit_tools::get_commit_template,
            commands::file_ops::open_repo_file,
            commands::file_ops::reveal_repo_file,
            commands::file_ops::add_to_gitignore,
            commands::file_ops::open_external_diff,
            // Branch list / remotes
            commands::branch_ops::set_branch_upstream,
            commands::branch_ops::unset_branch_upstream,
            commands::branch_ops::fast_forward_branch,
            commands::branch_ops::push_local_branch,
            commands::branch_ops::rebase_branch,
            commands::branch_ops::create_branch_at,
            commands::branch_ops::checkout_remote_tracking,
            commands::branch_ops::compare_branches,
            commands::remotes::list_remotes,
            commands::remotes::add_remote,
            commands::remotes::remove_remote,
            commands::remotes::rename_remote,
            commands::remotes::set_remote_urls,
            commands::remotes::fetch_remote,
            commands::remotes::prune_remote,
            // Conflict resolution & history rewriting (rebase, force push)
            commands::conflicts::get_operation_state,
            commands::conflicts::get_conflict_versions,
            commands::conflicts::continue_operation,
            commands::conflicts::abort_operation,
            commands::conflicts::skip_operation,
            commands::conflicts::resolve_take_side,
            commands::conflicts::mark_resolved,
            commands::conflicts::save_resolved_file,
            commands::conflicts::open_merge_tool,
            commands::history::rebase_onto,
            commands::history::list_rebase_commits,
            commands::history::interactive_rebase,
            commands::history::plan_squash,
            commands::history::squash_commits,
            commands::history::force_push_with_lease,
            // Commit graph: search & locate
            commands::graph::search_commits,
            commands::graph::locate_commit,
            // File history & blame
            commands::file_views::get_file_history,
            commands::file_views::get_file_diff_at_commit,
            commands::file_views::get_blame,
            commands::file_views::list_tracked_files,
            // Stash extras
            commands::stash_extra::stash_list_detailed,
            commands::stash_extra::stash_show,
            commands::stash_extra::stash_act,
            commands::stash_extra::stash_rename,
            commands::stash_extra::stash_branch,
            commands::stash_extra::stash_push_ext,
            // Submodules
            commands::submodules::list_submodules,
            commands::submodules::submodule_update,
            commands::submodules::submodule_sync,
            // Worktrees
            commands::worktrees::list_worktrees,
            commands::worktrees::worktree_add,
            commands::worktrees::worktree_remove,
            commands::worktrees::worktree_prune,
            // App shell: status bar, recent repos, settings import/export
            commands::app_shell::get_repo_status_summary,
            commands::app_shell::load_repo_history,
            commands::app_shell::save_repo_history,
            commands::app_shell::repo_paths_exist,
            commands::app_shell::open_in_file_manager,
            commands::launch::open_in_terminal,
            commands::signing::list_signing_keys,
            commands::signing::test_signing,
            commands::signing::commit_signatures,
            commands::launch::open_in_editor,
            commands::app_shell::open_settings_folder,
            commands::app_shell::export_settings,
            commands::app_shell::import_settings,
            // Hosting integrations (PRs, CI, OAuth device flow, GitLab/Gitea)
            commands::hosting::hosting_list_remotes,
            commands::hosting::hosting_info,
            commands::hosting::hosting_list_prs,
            commands::hosting::hosting_get_pr,
            commands::hosting::hosting_pr_files,
            commands::hosting::hosting_checkout_pr,
            commands::hosting::hosting_create_pr,
            commands::hosting::hosting_list_branches,
            commands::hosting::hosting_ci_status,
            commands::hosting::hosting_set_token,
            commands::hosting::hosting_has_token,
            commands::hosting::hosting_validate_token,
            commands::hosting::hosting_list_repos,
            commands::hosting::github_device_start,
            commands::hosting::github_device_wait,
            commands::hosting::github_device_cancel,
            // Diff viewer: partial (hunk/line) staging
            commands::hunks::apply_diff_selection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Twig");
}
