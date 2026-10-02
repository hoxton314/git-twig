//! Open a repository in a terminal or an editor.
//!
//! Commands come from settings (`terminal_command` / `editor_command`) or a
//! platform default. They are split into program + arguments here and run
//! directly — never through a shell — so paths are always single arguments.

use std::path::{Path, PathBuf};

use crate::commands::settings::load_settings_from_disk;
use crate::error::TwigError;
use crate::git::conflicts::validate_rel_path;

/// Split a command line into words. Supports `"double"` and `'single'`
/// quotes and backslash escapes outside single quotes; no other shell
/// syntax (no variables, globs or pipes).
pub(crate) fn split_command(s: &str) -> Result<Vec<String>, TwigError> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(ch) => cur.push(ch),
                        None => return Err(TwigError::InvalidArgument("unterminated ' in command".into())),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        // On Windows `\` is the path separator, even in quotes.
                        Some('\\') if !cfg!(windows) => match chars.next() {
                            Some(ch) => cur.push(ch),
                            None => return Err(TwigError::InvalidArgument("trailing \\ in command".into())),
                        },
                        Some(ch) => cur.push(ch),
                        None => return Err(TwigError::InvalidArgument("unterminated \" in command".into())),
                    }
                }
            }
            // Backslash escapes only on Unix: on Windows it is the path separator.
            '\\' if !cfg!(windows) => {
                in_word = true;
                match chars.next() {
                    Some(ch) => cur.push(ch),
                    None => return Err(TwigError::InvalidArgument("trailing \\ in command".into())),
                }
            }
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            c => {
                in_word = true;
                cur.push(c);
            }
        }
    }
    if in_word {
        words.push(cur);
    }
    Ok(words)
}

/// Find `name` on `PATH` (with `PATHEXT` on Windows). A name with a path
/// separator is resolved against `cwd` (where it will run) and returned as
/// an absolute path when it exists.
pub(crate) fn find_program(name: &str, cwd: &Path) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.components().count() > 1 {
        let full = cwd.join(p);
        return full.is_file().then_some(full);
    }
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
            .split(';')
            .map(|e| e.to_ascii_lowercase())
            .chain(std::iter::once(String::new()))
            .collect()
    } else {
        vec![String::new()]
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        exts.iter().find_map(|ext| {
            let cand = dir.join(format!("{name}{ext}"));
            cand.is_file().then_some(cand)
        })
    })
}

/// Program + args for a terminal opened in a folder (the folder is the
/// working directory; macOS also gets it as an argument).
fn default_terminal(dir: &Path) -> Option<Vec<String>> {
    if cfg!(target_os = "macos") {
        return Some(vec!["open".into(), "-a".into(), "Terminal".into(), dir.to_string_lossy().into()]);
    }
    if cfg!(windows) {
        if find_program("wt", dir).is_some() {
            return Some(vec!["wt".into(), "-d".into(), dir.to_string_lossy().into()]);
        }
        return Some(vec!["cmd".into(), "/c".into(), "start".into(), "cmd".into()]);
    }
    if let Ok(t) = std::env::var("TERMINAL") {
        if let Ok(words) = split_command(&t) {
            if words.first().is_some_and(|p| find_program(p, dir).is_some()) {
                return Some(words);
            }
        }
    }
    [
        "x-terminal-emulator",
        "gnome-terminal",
        "konsole",
        "xfce4-terminal",
        "kitty",
        "alacritty",
        "wezterm",
        "foot",
        "xterm",
    ]
    .iter()
    .find(|t| find_program(t, dir).is_some())
    .map(|t| vec![t.to_string()])
}

/// Replace `{path}` in every word with `path`; returns whether any did.
fn substitute_path(words: &mut [String], path: &str) -> bool {
    let mut any = false;
    for w in words.iter_mut() {
        if w.contains("{path}") {
            *w = w.replace("{path}", path);
            any = true;
        }
    }
    any
}

/// Program + args for a configured terminal command: `{path}` is replaced by
/// the folder (for launchers that ignore the working directory, e.g.
/// `open -a iTerm {path}`); otherwise only the working directory is set.
pub(crate) fn terminal_args(configured: &str, dir: &Path) -> Result<Vec<String>, TwigError> {
    let mut words = split_command(configured)?;
    substitute_path(&mut words, &dir.to_string_lossy());
    Ok(words)
}

/// Program + args for an editor opening `target`. `{path}` in the command is
/// replaced by the target; otherwise the target is appended.
pub(crate) fn editor_args(configured: Option<&str>, target: &Path, cwd: &Path) -> Result<Vec<String>, TwigError> {
    let target_s = target.to_string_lossy().to_string();
    let words = match configured.map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => split_command(c)?,
        None if find_program("code", cwd).is_some() => vec!["code".into()],
        None if cfg!(target_os = "macos") => vec!["open".into()],
        None if cfg!(windows) => vec!["explorer".into()],
        None => vec!["xdg-open".into()],
    };
    if words.is_empty() {
        return Err(TwigError::Config("editor command is empty".into()));
    }
    let mut words = words;
    if !substitute_path(&mut words, &target_s) {
        words.push(target_s);
    }
    Ok(words)
}

/// Spawn `args[0] args[1..]` in `cwd`, detached (not waited for here).
fn spawn(args: &[String], cwd: &Path) -> Result<(), TwigError> {
    let (prog, rest) = args
        .split_first()
        .ok_or_else(|| TwigError::Config("empty command".into()))?;
    let exe = find_program(prog, cwd).ok_or_else(|| {
        TwigError::Config(format!("'{prog}' was not found. Set the command in Settings > Editor & Diff."))
    })?;
    let mut child = tokio::process::Command::new(exe)
        .args(rest)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| TwigError::Config(format!("could not run {prog}: {e}")))?;
    // Reap in the background; terminals and editors outlive this call.
    tauri::async_runtime::spawn(async move {
        let _ = child.wait().await;
    });
    Ok(())
}

fn require_dir(path: &str) -> Result<PathBuf, TwigError> {
    let dir = PathBuf::from(path);
    if !dir.is_dir() {
        return Err(TwigError::InvalidArgument(format!("not a directory: {path}")));
    }
    Ok(dir)
}

/// Open a terminal in the repository folder.
#[tauri::command]
pub async fn open_in_terminal(app: tauri::AppHandle, path: String) -> Result<(), TwigError> {
    let dir = require_dir(&path)?;
    let configured = load_settings_from_disk(&app).terminal_command;
    let args = match configured.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => terminal_args(c, &dir)?,
        None => default_terminal(&dir).ok_or_else(|| {
            TwigError::Config("No terminal found. Set one in Settings > Editor & Diff.".into())
        })?,
    };
    spawn(&args, &dir)
}

/// Open the repository folder, or `file` (repository-relative) in it, in the
/// configured editor.
#[tauri::command]
pub async fn open_in_editor(
    app: tauri::AppHandle,
    path: String,
    file: Option<String>,
) -> Result<(), TwigError> {
    let dir = require_dir(&path)?;
    let target = match file.as_deref() {
        Some(f) => {
            validate_rel_path(f)?;
            dir.join(f)
        }
        None => dir.clone(),
    };
    let configured = load_settings_from_disk(&app).editor_command;
    let args = editor_args(configured.as_deref(), &target, &dir)?;
    spawn(&args, &dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_like_a_simple_shell() {
        let s = |x: &str| split_command(x).unwrap();
        assert_eq!(s("code --new-window"), vec!["code", "--new-window"]);
        assert_eq!(s(r#""/opt/My Editor/bin/ed" -w '{path}'"#), vec!["/opt/My Editor/bin/ed", "-w", "{path}"]);
        assert_eq!(s("  a   b  "), vec!["a", "b"]);
        assert_eq!(s(r#"x "" y"#), vec!["x", "", "y"]);
        assert!(split_command("a 'b").is_err());
        assert!(split_command("a \"b").is_err());
        assert!(s("").is_empty());
        if cfg!(windows) {
            assert_eq!(
                s(r#""C:\Program Files\Microsoft VS Code\Code.exe" -n"#),
                vec![r"C:\Program Files\Microsoft VS Code\Code.exe", "-n"]
            );
        } else {
            assert_eq!(s(r"a\ b c"), vec!["a b", "c"]);
            assert_eq!(s(r#""say \"hi\"""#), vec![r#"say "hi""#]);
        }
    }

    #[test]
    fn editor_args_append_or_substitute_the_path() {
        let t = Path::new("/r/my file.rs");
        let cwd = Path::new("/r");
        assert_eq!(editor_args(Some("subl -w"), t, cwd).unwrap(), vec!["subl", "-w", "/r/my file.rs"]);
        assert_eq!(
            editor_args(Some("idea --line 1 {path}"), t, cwd).unwrap(),
            vec!["idea", "--line", "1", "/r/my file.rs"]
        );
        // `$(...)` and `;` are just characters: nothing runs a shell.
        assert_eq!(editor_args(Some("ed; rm -rf ~"), t, cwd).unwrap()[0], "ed;");
        assert!(editor_args(Some("   "), t, cwd).unwrap().len() >= 2, "blank falls back to a default");
    }

    #[test]
    fn terminal_args_substitute_the_folder_only_when_asked() {
        let d = Path::new("/r/my repo");
        assert_eq!(terminal_args("open -a iTerm {path}", d).unwrap(), vec!["open", "-a", "iTerm", "/r/my repo"]);
        assert_eq!(terminal_args("kitty --single-instance", d).unwrap(), vec!["kitty", "--single-instance"]);
        assert_eq!(terminal_args("wezterm start --cwd={path}", d).unwrap()[2], "--cwd=/r/my repo");
    }

    #[test]
    fn find_program_on_path() {
        let here = std::env::temp_dir();
        assert!(find_program(if cfg!(windows) { "cmd" } else { "sh" }, &here).is_some());
        assert!(find_program("definitely-not-a-real-program-xyz", &here).is_none());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn spawn_runs_in_the_folder_without_a_shell() {
        let dir = std::env::temp_dir().join(format!("twig-launch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A name with shell metacharacters must arrive as one literal argument.
        spawn(&["touch".into(), "a b;$(x)".into()], &dir).unwrap();
        // A relative program path resolves against the folder it runs in.
        std::fs::create_dir_all(dir.join("tools")).unwrap();
        std::fs::write(dir.join("tools/mark.sh"), "#!/bin/sh\ntouch from-script\n").unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.join("tools/mark.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(find_program("./tools/mark.sh", &dir), Some(dir.join("./tools/mark.sh")));
        spawn(&["./tools/mark.sh".into()], &dir).unwrap();
        for _ in 0..50 {
            if dir.join("a b;$(x)").exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert!(dir.join("a b;$(x)").exists());
        for _ in 0..50 {
            if dir.join("from-script").exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert!(dir.join("from-script").exists(), "relative script did not run in the folder");
        assert!(spawn(&["definitely-not-a-real-program-xyz".into()], &dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
