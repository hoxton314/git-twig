//! History search for code changes: `git log -S` (commits that change how
//! often a string occurs, i.e. add or remove it) and `git log -G` (commits
//! whose diff adds or removes lines matching a regex).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use tokio::process::Command;
use tokio::sync::Notify;

use crate::error::TwigError;

/// The running search per repository: starting another one (or
/// [`cancel`]) stops it, so full-history scans never pile up.
static RUNNING: Mutex<Vec<(PathBuf, Arc<Notify>)>> = Mutex::new(Vec::new());

fn take_running(repo_path: &Path) -> Option<Arc<Notify>> {
    let mut running = RUNNING.lock().ok()?;
    let pos = running.iter().position(|(p, _)| p == repo_path)?;
    Some(running.remove(pos).1)
}

/// Run `cmd` in `repo_path` as the only search for that repository: a run
/// already in progress is cancelled (its process killed), and this one is
/// cancelled in turn by the next run or by [`cancel`].
async fn run_exclusive(repo_path: &Path, mut cmd: Command) -> Result<std::process::Output, TwigError> {
    cancel(repo_path);
    let stop = Arc::new(Notify::new());
    if let Ok(mut running) = RUNNING.lock() {
        running.push((repo_path.to_path_buf(), stop.clone()));
    }
    let child = cmd
        .current_dir(repo_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| TwigError::GitCli(format!("Failed to execute git: {e}")));
    let result = match child {
        // Dropping the wait (when cancelled) kills the process.
        Ok(child) => tokio::select! {
            out = child.wait_with_output() => out.map_err(TwigError::from),
            _ = stop.notified() => Err(TwigError::InvalidArgument("search cancelled".into())),
        },
        Err(e) => Err(e),
    };
    if let Ok(mut running) = RUNNING.lock() {
        running.retain(|(_, n)| !Arc::ptr_eq(n, &stop));
    }
    result
}

/// Stop the code-change search running for `repo_path`, if any.
pub fn cancel(repo_path: &Path) {
    if let Some(n) = take_running(repo_path) {
        n.notify_one();
    }
}

/// Commits (newest first, as `git log` lists them) whose changes match
/// `query`, over the revisions in `tips`, limited to `paths`. Returns the
/// ids and whether more than `max` matched.
pub async fn pickaxe(
    repo_path: &Path,
    query: &str,
    regex: bool,
    ignore_case: bool,
    tips: &[String],
    paths: &[String],
    max: usize,
) -> Result<(Vec<String>, bool), TwigError> {
    if query.is_empty() {
        return Err(TwigError::InvalidArgument("enter something to search for".into()));
    }
    if tips.is_empty() {
        return Ok((Vec::new(), false));
    }
    let pick = if regex { format!("-G{query}") } else { format!("-S{query}") };
    let limit = format!("--max-count={}", max + 1);
    let mut args: Vec<&str> = vec![
        // Config that would add lines to the output or change what is walked.
        "-c",
        "log.showSignature=false",
        "-c",
        "log.follow=false",
        "log",
        // With paths, don't simplify away side branches whose merge leaves
        // the file unchanged: their commits still added or removed the text.
        "--full-history",
        "--format=%H",
        "--no-ext-diff",
        "--no-textconv",
        &limit,
        &pick,
    ];
    if ignore_case {
        args.push("--regexp-ignore-case");
    }
    args.extend(tips.iter().map(String::as_str));
    args.push("--");
    args.extend(paths.iter().map(|p| p.trim()).filter(|p| !p.is_empty()));
    let mut cmd = Command::new("git");
    cmd.args(&args).env("GIT_TERMINAL_PROMPT", "0");
    let output = run_exclusive(repo_path, cmd).await?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let msg = err.trim();
        return Err(TwigError::InvalidArgument(if msg.is_empty() { "git log failed".into() } else { msg.to_string() }));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut oids: Vec<String> = stdout
        .lines()
        .map(str::trim)
        .filter(|l| l.len() >= 40 && l.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(String::from)
        .collect();
    let truncated = oids.len() > max;
    oids.truncate(max);
    Ok((oids, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::graph::{graph_tip_args, matches_in_graph, GraphOptions};
    use crate::git::writer::run_git;
    use git2::Repository;

    async fn git(dir: &Path, args: &[&str]) -> String {
        let out = run_git(dir, args).await.unwrap();
        assert!(out.success, "git {args:?}: {}", out.stderr);
        out.stdout.trim().to_string()
    }

    async fn commit(dir: &Path, file: &str, body: &str, msg: &str) -> String {
        std::fs::write(dir.join(file), body).unwrap();
        git(dir, &["add", "."]).await;
        git(dir, &["commit", "-q", "-m", msg]).await;
        git(dir, &["rev-parse", "HEAD"]).await
    }

    #[tokio::test]
    async fn finds_commits_that_add_or_remove_code() {
        let dir = std::env::temp_dir().join(format!("twig-pickaxe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]).await;
        git(&dir, &["config", "user.name", "t"]).await;
        git(&dir, &["config", "user.email", "t@t"]).await;
        git(&dir, &["config", "commit.gpgsign", "false"]).await;
        let add = commit(&dir, "a.rs", "fn helper() {}\n", "add helper").await;
        let _touch = commit(&dir, "a.rs", "fn helper() {}\n// note\n", "unrelated").await;
        let other = commit(&dir, "b.rs", "call helper()\n", "use it elsewhere").await;
        let remove = commit(&dir, "a.rs", "// note\n", "remove helper").await;
        git(&dir, &["checkout", "-q", "-b", "side", &add]).await;
        let side = commit(&dir, "c.rs", "HELPER\n", "side branch").await;
        git(&dir, &["checkout", "-q", "main"]).await;

        let repo = Repository::open(&dir).unwrap();
        let opts = GraphOptions::default();
        let tips = graph_tip_args(&repo, &opts);
        assert_eq!(tips, ["--branches", "--remotes", "HEAD"]);

        let (oids, truncated) = pickaxe(&dir, "fn helper", false, false, &tips, &[], 100).await.unwrap();
        assert!(!truncated);
        let mut found = oids.clone();
        found.sort();
        let mut want = vec![add.clone(), remove.clone()];
        want.sort();
        assert_eq!(found, want, "-S: added and removed, not the unrelated edit");

        // Path filter, regex (-G), case-insensitive, other branches.
        let (oids, _) = pickaxe(&dir, "helper", false, false, &tips, &["b.rs".into()], 100).await.unwrap();
        assert_eq!(oids, std::slice::from_ref(&other));
        let (oids, _) = pickaxe(&dir, "^HELPER$", true, false, &tips, &[], 100).await.unwrap();
        assert_eq!(oids, std::slice::from_ref(&side));
        let (oids, _) = pickaxe(&dir, "helper", false, true, &tips, &["c.rs".into()], 100).await.unwrap();
        assert_eq!(oids, std::slice::from_ref(&side));
        let current_only = graph_tip_args(&repo, &GraphOptions { current_branch_only: true, ..opts.clone() });
        let (oids, _) = pickaxe(&dir, "HELPER", false, false, &current_only, &[], 100).await.unwrap();
        assert!(oids.is_empty(), "side branch hidden in current-branch mode");

        // Limit, and mapping onto graph rows (newest first).
        let (oids, truncated) = pickaxe(&dir, "helper", false, true, &tips, &[], 2).await.unwrap();
        assert!(truncated && oids.len() == 2);
        let (all, _) = pickaxe(&dir, "helper", false, true, &tips, &[], 100).await.unwrap();
        let r = matches_in_graph(&repo, &all, &opts, 100, false).unwrap();
        assert_eq!(r.matches.len(), 4);
        assert!(r.matches.windows(2).all(|w| w[0].index < w[1].index));
        let r = matches_in_graph(&repo, &all, &opts, 3, false).unwrap();
        assert!(r.truncated && r.matches.len() == 3);

        // A merged side branch whose merge leaves the file unchanged still
        // counts with a path filter; signature config adds no output lines.
        git(&dir, &["checkout", "-q", "-b", "twin"]).await;
        let twin = commit(&dir, "d.rs", "twin text\n", "twin adds").await;
        git(&dir, &["checkout", "-q", "main"]).await;
        let mainline = commit(&dir, "d.rs", "twin text\n", "main adds").await;
        git(&dir, &["merge", "-q", "--no-edit", "twin"]).await;
        git(&dir, &["branch", "-q", "-D", "twin"]).await;
        git(&dir, &["config", "log.showSignature", "true"]).await;
        let (oids, _) = pickaxe(&dir, "twin text", false, false, &tips, &["d.rs".into()], 100).await.unwrap();
        let mut found = oids.clone();
        found.sort();
        let mut want = vec![twin, mainline];
        want.sort();
        assert_eq!(found, want);

        assert!(pickaxe(&dir, "(", true, false, &tips, &[], 10).await.is_err(), "bad regex");
        assert!(pickaxe(&dir, "", false, false, &tips, &[], 10).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn searches_are_cancelled_by_newer_ones_and_by_cancel() {
        let dir = std::env::temp_dir().join(format!("twig-pickaxe-cancel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sleeper = || {
            let mut c = Command::new("sleep");
            c.arg("5");
            c
        };
        let registered = |d: &Path| RUNNING.lock().unwrap().iter().any(|(p, _)| p == d);

        // cancel() stops a running search and kills its process quickly.
        let d = dir.clone();
        let started = std::time::Instant::now();
        let first = tokio::spawn(async move { run_exclusive(&d, sleeper()).await });
        while !registered(&dir) {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        cancel(&dir);
        let err = first.await.unwrap().unwrap_err().to_string();
        assert!(err.contains("cancelled"), "{err}");
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
        assert!(!registered(&dir));

        // A newer search cancels the older one and runs to completion itself.
        let d = dir.clone();
        let older = tokio::spawn(async move { run_exclusive(&d, sleeper()).await });
        while !registered(&dir) {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let mut quick = Command::new("true");
        quick.arg("ok");
        let newer = run_exclusive(&dir, quick).await.unwrap();
        assert!(newer.status.success());
        assert!(older.await.unwrap().unwrap_err().to_string().contains("cancelled"));
        assert!(!registered(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
