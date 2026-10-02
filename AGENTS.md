# Agents Guide for Twig

This file provides context for AI coding agents working on this codebase.

## What is Twig?

A lightweight Git GUI desktop app built with Tauri v2 (Rust backend + Svelte 5 frontend). Tagline: "Lighter than the rest." Primary target: Linux Wayland.

## Quick Orientation

- **Rust backend**: `src-tauri/src/` -- all git operations, app state, Tauri command handlers
- **Svelte frontend**: `src/` -- UI components, stores, typed IPC wrappers
- **No direct git logic in the frontend.** Everything goes through Tauri commands.

## Architecture Rules

1. **All git operations happen in Rust**, never in the frontend
2. **Read operations** use `git2` crate (`src-tauri/src/git/reader.rs`)
3. **Write operations** use system `git` CLI via `tokio::process` (`src-tauri/src/git/writer.rs`)
4. **No `unwrap()` in Rust** -- use `TwigError` and return `Result` from all commands
5. **All Tauri commands must be `async`**
6. **TypeScript types** must mirror Rust structs exactly (see `src/lib/types/git/<area>.ts`)
7. **Every Tauri command** gets a typed wrapper in `src/lib/api/<area>.ts` (re-exported by `src/lib/tauri.ts`) -- no direct `invoke()` elsewhere

## Key Files to Understand First

| File | Purpose |
|------|---------|
| `src-tauri/src/lib.rs` | Tauri builder -- all commands registered here |
| `src-tauri/src/state.rs` | `AppState` with `Mutex<HashMap<String, OpenRepo>>` |
| `src-tauri/src/error.rs` | `TwigError` enum -- add new variants here |
| `src-tauri/src/git/reader.rs` | git2-based reads: branches, diffs, status (graph IPC structs) |
| `src-tauri/src/commands/launch.rs` | "Open in terminal / editor": `terminal_command` / `editor_command` settings (or platform defaults), split by `split_command` and spawned without a shell; `find_program` does PATH lookup |
| `src-tauri/src/git/create.rs` | `git init` (folder created if missing) and `git clone` from any URL with streamed progress; commands `init_repository` / `clone_repository` in `commands/repo.rs` (`register_repo` opens the result as a tab) |
| `src/components/layout/NewRepoDialog.svelte` | Clone-from-URL / initialize dialogs, opened via the `newRepoDialog` store (`src/lib/newRepo.ts`) |
| `src-tauri/src/git/graph.rs` | Commit graph: paginated walk + lane assignment (`LaneState`), full-history search, locate ref/commit row |
| `src-tauri/src/git/writer.rs` | CLI-based writes: checkout, commit, push, pull, stage |
| `src-tauri/src/commands/settings.rs` | `AppSettings` struct, load/save to `settings.json` |
| `src-tauri/src/commands/git_config.rs` | Read/write global `~/.gitconfig` values, detect LFS |
| `src/lib/types/git.ts` | Re-exports the shared IPC types from `src/lib/types/git/<area>.ts` (common, repo, graph, branches, diff, stash, settings, history, workspace); import from `types/git` |
| `src/lib/tauri.ts` | Re-exports the typed `invoke()` wrappers from `src/lib/api/<area>.ts` (repo, graph, branches, diff, stash, settings, github, history, workspace, hosting); import as `import * as tauri from "…/lib/tauri"` |
| `src/lib/stores/graph.ts` | Central Svelte store for graph, branches, diffs, staging; graph pagination (`loadMoreCommits`, `ensureGraphLoaded`) |
| `src/lib/stores/graphSearch.ts` | Commit search state (highlight/filter modes, matches with graph row indices) |
| `src/lib/stores/clock.ts` | Shared minute clock (`now`) for relative dates |
| `src/lib/stores/settings.ts` | Settings store with auto-persist and CSS variable application |
| `src/lib/keybindings.ts` | Global keybinding registry, shortcut parsing, action dispatch |
| `src-tauri/src/commands/stash.rs` | Stash operations: list, push, pop, apply, drop |
| `src/components/staging/StashPanel.svelte` | Stash management UI (collapsible panel in staging area) |
| `src-tauri/src/github.rs` | GitHub REST API client (types, HTTP calls, URL parsing) |
| `src-tauri/src/commands/github.rs` | GitHub Tauri commands (validate, list, clone, create repo/PR) |
| `src/lib/types/github.ts` | GitHub TypeScript interfaces |
| `src/components/shared/Modal.svelte` | Reusable modal dialog component |
| `src/components/github/CloneFromGitHub.svelte` | Clone from GitHub interactive repo list |
| `src/components/github/CreateRepoOnGitHub.svelte` | Create new GitHub repository dialog |
| `src/components/github/CreatePullRequest.svelte` | Create pull request dialog |
| `src/components/settings/GitHubSettings.svelte` | GitHub PAT configuration in settings |
| `src-tauri/src/hosting/` | Hosting integrations: `config` (GitHub/GHE/GitLab/Gitea endpoints from settings), `remote` (provider-aware remote URL parsing), `net_auth` (keyring token for HTTPS fetch/pull/push), `github_api`/`gitlab`/`gitea` (PRs, CI, device flow), `patch` (unified diff → hunks) |
| `src-tauri/src/commands/hosting.rs` | Provider-agnostic `hosting_*` commands (PR list/detail/files/checkout/create, CI status, GitLab/Gitea tokens) + GitHub device flow |
| `src/components/github/PullRequestsPanel.svelte` | Pull/merge request browser (opened via `prPanelOpen` store / `open_pull_requests` action) |
| `src/components/github/CiBadge.svelte` | Drop-in CI badge: `<CiBadge sha={oid} />` (backed by `src/lib/stores/ci.ts` `ciStatusFor(sha)`) |
| `src/lib/markdown.ts` | Safe Markdown renderer (escapes all HTML) for PR descriptions |

## Adding a New Feature

### Adding a new Tauri command

1. If it's a read, add a function to `src-tauri/src/git/reader.rs`
2. If it's a write, add a function to `src-tauri/src/git/writer.rs`
3. Create the `#[tauri::command]` handler in the appropriate `src-tauri/src/commands/*.rs`
4. Register it in `src-tauri/src/lib.rs` `invoke_handler`
5. Add the TypeScript interface to the matching `src/lib/types/git/<area>.ts`
6. Add the typed wrapper to the matching `src/lib/api/<area>.ts` (new area: add a module and an `export *` line in `src/lib/tauri.ts`)
7. Use the wrapper from Svelte components -- never call `invoke()` directly

### Adding a new setting

1. Add the field to `AppSettings` in `src-tauri/src/commands/settings.rs` (with `#[serde(default)]`)
2. Update the `Default` impl
3. Add the field to `AppSettings` in `src/lib/types/git/settings.ts`
4. Update defaults in `src/lib/stores/settings.ts`
5. Add UI control in the appropriate `src/components/settings/*.svelte` section
6. If the setting needs a CSS variable, apply it in `applyVisualSettings()` in `settings.ts`

### Adding a new keybinding

1. Add the action to `ACTIONS` array in `src/lib/keybindings.ts` with `id`, `label`, `category`, `defaultShortcut`
2. Register a handler with `onAction(actionId, handler)` in the component where the action logic lives
3. Handlers registered in `AppShell.svelte` for global actions; component-specific actions register in their own `onMount`

### Backend → UI events

Long-running commands report progress with `app.emit("<event>", payload)` (`tauri::Emitter`), e.g. `clone-progress` from `clone_repository`. Payloads carry an `op_id` chosen by the caller so concurrent operations don't mix. Subscribe through a typed wrapper in `src/lib/api/` (like `onCloneProgress`), never `listen()` directly in components.

### Shared UI primitives

- **Context menus**: `src/components/shared/ContextMenu.svelte` -- pass `x`, `y` (from the `contextmenu` event), `items: MenuItem[]` (`label`, `action`, `shortcut`, `danger`, `disabled`, `separator`), `onclose`. Handles viewport clamping, Esc, arrow keys, click-outside. The browser's default menu is disabled globally in `main.ts`.
- **Notifications**: `toast(kind, message, opts)` / `toastError(title, err)` from `src/lib/stores/toasts.ts` (rendered by `Toaster.svelte` in `AppShell`). Prefer toasts over blocking `message()` dialogs for operation results; keep `ask()`/`Modal` for confirmations.
- **Confirmations** for destructive actions use `ask()` from `@tauri-apps/plugin-dialog` and respect `$settings.confirm_destructive_ops`.
- **Git CLI helpers** in `git/writer.rs` are `pub(crate)`: `run_git`, `run_git_paths` (literal pathspecs after `--`), `safe_ref` (reject leading `-`), `rev_exists`. Always pass `--` before paths and validate refs with `safe_ref`.

### Adding a new component

Components go in `src/components/<feature>/`. Use existing patterns:
- Props via `$props()` (Svelte 5 runes)
- Reactive state via `$state()` and `$derived()`
- Side effects via `$effect()`
- Stores from `src/lib/stores/`
- Tauri calls from `src/lib/tauri.ts`

## Build & Dev Commands

```sh
npm install              # Install frontend deps
npm run tauri dev        # Dev mode (HMR + Rust rebuild on change)
npm run tauri build      # Production build
npm run build            # Frontend only build
npm run check            # TypeScript + Svelte type checking
npm test                 # Frontend unit tests (Vitest, `src/**/*.test.ts`)
npm run tauri build -- --debug --no-bundle && xvfb-run -a -s "-screen 0 1440x900x24" npm run test:e2e
                         # E2E smoke tests (e2e/smoke.mjs; needs tauri-driver + WebKitWebDriver)
```

Rust-only check:

```sh
cd src-tauri && cargo check
```

CI (`.github/workflows/ci.yml`) runs on every push to `main` and every pull request: `npm run check`,
`npm test`, `npm run build`, `cargo clippy --all-targets -- -D warnings` and `cargo test` on ubuntu-22.04. Keep
all of them green locally before pushing. Releases are built separately by `release.yml` on `v*` tags.

## Coding Conventions

### Rust

- Error type: `TwigError` (in `error.rs`), uses `thiserror`
- No `unwrap()`, no `expect()` in library code (main.rs `expect` on Tauri run is the one exception)
- Async all commands, even if the body is sync (Tauri requirement for `State<>` access)
- `serde::Serialize` on all types crossing the IPC boundary
- Command results that are write operations return `CommandResult { success, message }`
- Commit text from git2: use `git::graph::{commit_summary, commit_message, commit_author_name, decode_text}`, never `summary()`/`message()`/`name()` — those return `None` for non-UTF-8 (e.g. Latin-1) commits, which shows up as empty text

### Frontend

- E2E: `e2e/smoke.mjs` drives the debug build through `tauri-driver` with a tiny W3C WebDriver client (`e2e/webdriver.mjs`, no WebdriverIO). It runs in throwaway XDG dirs, git config and repo, and opens the repo via a pre-written `session.json`. CI runs it in the `e2e` job (ubuntu-22.04, `webkit2gtk-driver`, Xvfb; failure screenshot + HTML are uploaded as artifacts). Select elements by existing `aria-label`s/classes; row actions only show on hover/selection, so click the row first. Arch's `webkit2gtk-4.1` ships no `WebKitWebDriver`, so locally it may only run in CI.
- Unit tests: Vitest in the `node` environment, next to the code (`foo.ts` → `foo.test.ts`). Test pure logic and stores; mock `../tauri` with `vi.mock` (see `stores/graph.test.ts`) and stub `window`/`document` where needed (see `keybindings.test.ts`). UI behaviour belongs in the e2e smoke tests.

- Svelte 5 runes (`$state`, `$derived`, `$effect`, `$props`) -- no legacy `let` reactivity
- TailwindCSS v4 with custom `@theme` tokens in `app.css` -- use `var(--color-*)` for colors
- Dark theme only -- all colors defined in the design system
- No nested `<button>` elements (Svelte 5 enforces valid HTML)
- Font: system UI for interface, monospace for code/hashes/diffs

### Design System Colors

```
Background:        #1a1b26 (--color-bg)
Surface:           #1f2335 (--color-surface)
Surface elevated:  #24283b (--color-surface-elevated)
Border:            #292e42 (--color-border)
Accent:            #7aa2f7 (--color-accent)
Accent secondary:  #bb9af7 (--color-accent-secondary)
Text primary:      #c0caf5 (--color-text-primary)
Text muted:        #565f89 (--color-text-muted)
Diff add:          #9ece6a
Diff delete:       #f7768e
Lane colors:       #7aa2f7, #9ece6a, #e0af68, #f7768e, #bb9af7, #2ac3de
```

## Platform Notes

### Linux Wayland + NVIDIA

`main.rs` detects NVIDIA GPUs at runtime via `/proc/driver/nvidia` and configures WebKitGTK accordingly. On NVIDIA, `WEBKIT_DISABLE_DMABUF_RENDERER=1` is set before GTK init. On all Wayland GPUs, `GDK_GL=gles` is set. This is intentional -- do not remove it.

### Auto-updater

`createUpdaterArtifacts` is on, so release builds emit `.sig` files and tauri-action publishes `latest.json` (`linux-x86_64-deb`, `linux-x86_64-rpm`, `darwin-*`, `windows-x86_64`). The `verify-updater-json` CI job fails the release if a platform is missing. `UpdateChecker.svelte` only checks when `updater_supported` (`commands/updater.rs`) says the running binary is owned by the package type it was bundled as -- the AUR `twig-bin` package reuses the `.deb` binary but must update through pacman, and dev builds have no bundle type. The release stays a draft until published manually; `releases/latest/download/latest.json` only resolves after that.

### LFS

LFS pointer files in diffs are detected by checking for `version https://git-lfs.github.com/spec/` in the diff content. They display as "LFS object -- [size]" instead of raw pointer text. All LFS operations go through the system git CLI.

## Settings Architecture

Settings are stored separately from session state:

- **Session** (`session.json`): ephemeral layout state -- open repos, active tab, panel sizes. Managed by `src/lib/stores/repos.ts`.
- **Settings** (`settings.json`): durable user preferences -- UI options, diff preferences, keybinding overrides. Managed by `src/lib/stores/settings.ts`.
- **Git config** (`~/.gitconfig`): identity, pull strategy, signing. Read/written via `git config --global` CLI commands in `src-tauri/src/commands/git_config.rs`.
- **Secrets** (OS keyring): the GitHub token lives in Secret Service / Keychain / Credential Manager via `src-tauri/src/credentials.rs`, never in `settings.json` and never sent to the webview (the UI only calls `github_set_token` / `github_has_token`). A plaintext `github_token` left in an old `settings.json` is migrated on `load_settings`. GitLab/Gitea tokens use keyring accounts `gitlab-token:<host>` / `gitea-token:<host>`. "Sign in with GitHub" (device flow) uses a public OAuth App client ID baked in at build time from the `TWIG_GITHUB_OAUTH_CLIENT_ID` env var (`hosting::github_api::github_oauth_client_id()`); release builds take it from the repository variable of the same name, and without it the sign-in button is hidden. Every command that talks to a remote (fetch, pull, push, ls-remote, submodule update, …) must wrap its git call in `with_network_auth(&app, &repo_path, ...)`; the token is passed as a host-scoped `http.https://<host>/.extraheader` through `GIT_CONFIG_*` env vars only.

Both JSON files live in Tauri's `app_data_dir`. Settings auto-persist with a 300ms debounce on any change.

Visual settings (accent color, font sizes, theme) are applied as CSS custom properties on `document.documentElement` via `applyVisualSettings()` in the settings store subscription. Theme switching sets `data-theme="light"` or `data-theme="dark"` on the root element; light theme colors are defined in `app.css` under the `[data-theme="light"]` selector.

### Settings Screen Sections

| Section | Component | What it controls |
|---------|-----------|------------------|
| General | `GeneralSettings.svelte` | Default repo dir, auto-fetch interval, max commits, confirmations |
| Appearance | `AppearanceSettings.svelte` | Theme (dark/light), accent color, interface/diff font sizes |
| Editor & Diff | `EditorDiffSettings.svelte` | Diff view mode, tab size, context lines, whitespace, word wrap, external tools |
| Git Configuration | `GitConfigSettings.svelte` | user.name/email, pull strategy, fetch.prune, GPG signing, LFS status |
| Keybindings | `KeybindingsSettings.svelte` | View/rebind all keyboard shortcuts, reset to defaults |

## Keybinding System

Global keybindings are managed by `src/lib/keybindings.ts`:

- **Actions**: defined in `ACTIONS` array with id, label, category, default shortcut
- **Handlers**: registered with `onAction(id, fn)`, returning an unsubscribe function
- **Overrides**: stored in `AppSettings.keybinding_overrides` as `Record<actionId, shortcutString>`
- **Shortcuts**: strings like `"Ctrl+Enter"`, `"Ctrl+Shift+P"`. Modifiers: `Ctrl`, `Shift`, `Alt`
- **Input exception**: shortcuts are suppressed when focused on input/textarea/select, except `commit` which works in the commit message textarea

Global actions (open repo, close tab, tab switching, settings, sidebar toggle, fetch/pull/push) are registered in `AppShell.svelte`. Component-specific actions (commit) are registered in their own component's `onMount`.

## V2 TODO

- Authentication / credential manager (GitHub token is in the OS keyring and used for HTTPS fetch/pull/push to the GitHub host via `hosting::net_auth::with_network_auth`; other hosts still rely on the system git credential helper)
- ~~GitHub API integration~~ ✓ (clone from GitHub, create repo, create PR via PAT in Settings > GitHub)
- SSH key management
- ~~Conflict resolution UI~~ ✓ (operation banner + conflict list + 3-way resolver in `src/components/conflicts/`, backend `git/conflicts.rs`)
- Blame view
- ~~Git identity profiles~~ (use local git config directly, no extra abstraction needed)
- ~~Light theme~~ ✓ (dark/light toggle in Appearance settings, `[data-theme="light"]` in `app.css`)
- ~~Stash management UI~~ ✓ (list/push/pop/apply/drop via `StashPanel.svelte`, includes untracked files)
- ~~Interactive rebase UI~~ ✓ (`src/components/rebase/`, backend `git/history.rs`; also rebase onto and force push with lease)
- File history view
- Submodule support
