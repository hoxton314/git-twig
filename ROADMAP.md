# Twig Roadmap

Twig 0.3.0 shipped a large feature set (conflict resolution, interactive rebase, hunk/line staging,
command palette, file history & blame, hosting integrations, and more). Most of it has not had
real-world use yet, so the next step is making it trustworthy before growing it further.

Each release has a matching [GitHub milestone](https://github.com/hoxton314/git-twig/milestones).
Order within a release is rough priority.

## v0.3.x — Stabilize

- [x] Hands-on QA pass over every 0.3 feature; fix what breaks. Highest risk: interactive rebase,
  line staging, conflict resolution, history pagination.
- [ ] Verify hosting integrations against live services: pull requests, CI status, GitLab, Gitea.
  (GitHub verified by an opt-in live test; GitLab and Gitea still need accounts.)
- [x] CI on every push and pull request (type check, build, clippy, tests) via `.github/workflows/ci.yml`.
- [ ] Settle open decisions: ~~whether whitespace-only changes are hidden by default~~ (shown, like git); register the
  OAuth app to enable "Sign in with GitHub" (the build reads `TWIG_GITHUB_OAUTH_CLIENT_ID`;
  the app itself still needs registering).

## v0.4 — Foundations and everyday gaps

- [x] `git init` and clone from any URL (currently clone only works from GitHub / GitLab / Gitea).
- [x] `twig [path]` command-line launcher that opens a tab in the already-running window.
- [x] Open repository in terminal / editor.
- [x] Commit signing end to end: GPG or SSH signing setup and a "verified" badge in the graph
  (covers SSH key management).
- [x] Large-repo performance: profile on a kernel-sized repo; true virtualization for huge diffs
  (today there is only a 2,000-line "show anyway" cutoff).
- [x] Code health: split `src/lib/tauri.ts` and `src/lib/types/git.ts` by area; add frontend unit
  tests (Vitest) and end-to-end smoke tests against the real app.

## v0.5 — Power tools

- [x] Bisect UI (bisect state is already detected and shown in the status bar).
- [x] Multi-select in the graph: cherry-pick a range, squash selected commits, diff any two commits.
- [x] Content search: search file contents, and search history for when a string was added or removed.
- [x] Patches: create and apply patch files.
- [x] Git LFS management: track, lock/unlock, fetch, prune.
- [x] Git hooks: show hook output; "skip hooks" option on commit.

## v0.6 — Workspace and distribution

- [x] Multiple windows, each with its own saved tabs.
- [x] Per-repository settings overrides.
- [x] Repository groups with a dashboard to fetch all repos at once.
- [x] System notifications (new upstream commits, CI finished).
- [x] Custom themes and translations (Polish is the first translation).
- [x] Packaging: Flatpak or AppImage (in-app updates for distros without deb/rpm/AUR), signed and
  notarized macOS and Windows builds, Linux ARM64 builds. (AppImage and ARM64 ship; the Flatpak
  manifest is built in CI but not yet on Flathub; signing is wired up and needs the certificates
  described in `docs/SIGNING.md`.)

## v1.0 — Stable

1.0 is a quality bar rather than a feature list:

- Core flows covered by end-to-end tests.
- No open bugs that can lose data.
- Verified on large repositories.
- Signed builds on every platform.
- The updater has worked across at least two real releases.
- A user guide and an accessibility audit.

## Later, if wanted

- Reviewing pull requests inside Twig (reading and writing comments).
- GitLab merge requests from forks; HTTPS sign-in for GitLab and Gitea.
- AI-written commit messages: opt-in only, using your own API key.
