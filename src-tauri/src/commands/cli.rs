//! `twig [path…]` command-line launching.
//!
//! The first instance keeps its own paths until the frontend asks for them
//! (after it restored the session). Later invocations are forwarded by the
//! single-instance plugin to the running window as an `open-paths` event,
//! resolved against the caller's working directory.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{Emitter, Manager};

use crate::error::TwigError;

static STARTUP_PATHS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Paths given on a command line (`argv[0]` excluded). Flags (`-…`) are
/// ignored; relative paths resolve against `cwd`. No arguments means no
/// path: a desktop launcher's working directory is usually `$HOME`.
pub(crate) fn paths_from_args(args: &[String], cwd: &Path) -> Vec<String> {
    args.iter()
        .skip(1)
        .filter(|a| !a.is_empty() && !a.starts_with('-'))
        .map(|a| {
            let p = Path::new(a);
            let full: PathBuf = if p.is_absolute() { p.to_path_buf() } else { cwd.join(p) };
            full.canonicalize().unwrap_or(full).to_string_lossy().into_owned()
        })
        .collect()
}

/// Remember this process's own command-line paths for the frontend.
pub(crate) fn record_startup_args() {
    let args: Vec<String> = std::env::args().collect();
    let cwd = std::env::current_dir().unwrap_or_default();
    if let Ok(mut paths) = STARTUP_PATHS.lock() {
        *paths = paths_from_args(&args, &cwd);
    }
}

/// Another `twig …` was started: focus this window and hand it the paths.
///
/// Runs inside the D-Bus/IPC call the second process is blocked on, so the
/// work is queued and the call returns at once (window operations wait for
/// the main loop and would otherwise keep the second process alive).
pub(crate) fn on_second_instance(app: &tauri::AppHandle, args: Vec<String>, cwd: String) {
    let paths = paths_from_args(&args, Path::new(&cwd));
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
        if !paths.is_empty() {
            let _ = app.emit("open-paths", paths);
        }
    });
}

/// Paths from this process's command line; returned once.
#[tauri::command]
pub async fn take_startup_paths() -> Result<Vec<String>, TwigError> {
    let mut paths = STARTUP_PATHS.lock().map_err(|_| TwigError::Lock)?;
    Ok(std::mem::take(&mut *paths))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_paths_against_the_callers_cwd() {
        let cwd = std::env::temp_dir().join(format!("twig-cli-{}", std::process::id()));
        std::fs::create_dir_all(cwd.join("proj/src")).unwrap();
        let canon = cwd.canonicalize().unwrap();
        let args = |a: &[&str]| -> Vec<String> {
            std::iter::once("twig").chain(a.iter().copied()).map(String::from).collect()
        };
        assert!(paths_from_args(&args(&[]), &cwd).is_empty(), "no args → no path");
        assert_eq!(paths_from_args(&args(&["."]), &cwd), vec![canon.to_string_lossy().to_string()]);
        assert_eq!(
            paths_from_args(&args(&["proj/src", "--flag", "-x"]), &cwd),
            vec![canon.join("proj/src").to_string_lossy().to_string()]
        );
        let abs = canon.join("proj").to_string_lossy().to_string();
        assert_eq!(paths_from_args(&args(&[&abs]), Path::new("/somewhere/else")), vec![abs.clone()]);
        // A missing path is kept (the frontend reports it as not a repository).
        let missing = paths_from_args(&args(&["nope"]), &cwd);
        assert!(missing[0].ends_with("nope"));
        let _ = std::fs::remove_dir_all(&cwd);
    }
}
