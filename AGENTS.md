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
6. **TypeScript types** must mirror Rust structs exactly (see `src/lib/types/git.ts`)
7. **Every Tauri command** gets a typed wrapper in `src/lib/tauri.ts` -- no direct `invoke()` elsewhere

## Key Files to Understand First

| File | Purpose |
|------|---------|
| `src-tauri/src/lib.rs` | Tauri builder -- all commands registered here |
| `src-tauri/src/state.rs` | `AppState` with `Mutex<HashMap<String, OpenRepo>>` |
| `src-tauri/src/error.rs` | `TwigError` enum -- add new variants here |
| `src-tauri/src/git/reader.rs` | git2-based reads: graph, branches, diffs, status |
| `src-tauri/src/git/writer.rs` | CLI-based writes: checkout, commit, push, pull, stage |
| `src-tauri/src/commands/settings.rs` | `AppSettings` struct, load/save to `settings.json` |
| `src-tauri/src/commands/git_config.rs` | Read/write global `~/.gitconfig` values, detect LFS |
| `src/lib/types/git.ts` | All shared TypeScript interfaces |
| `src/lib/tauri.ts` | Typed `invoke()` wrappers for every command |
| `src/lib/stores/graph.ts` | Central Svelte store for graph, branches, diffs, staging |
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

## Adding a New Feature

### Adding a new Tauri command

1. If it's a read, add a function to `src-tauri/src/git/reader.rs`
2. If it's a write, add a function to `src-tauri/src/git/writer.rs`
3. Create the `#[tauri::command]` handler in the appropriate `src-tauri/src/commands/*.rs`
4. Register it in `src-tauri/src/lib.rs` `invoke_handler`
5. Add the TypeScript interface to `src/lib/types/git.ts`
6. Add the typed wrapper to `src/lib/tauri.ts`
7. Use the wrapper from Svelte components -- never call `invoke()` directly

### Adding a new setting

1. Add the field to `AppSettings` in `src-tauri/src/commands/settings.rs` (with `#[serde(default)]`)
2. Update the `Default` impl
3. Add the field to `AppSettings` in `src/lib/types/git.ts`
4. Update defaults in `src/lib/stores/settings.ts`
5. Add UI control in the appropriate `src/components/settings/*.svelte` section
6. If the setting needs a CSS variable, apply it in `applyVisualSettings()` in `settings.ts`

### Adding a new keybinding

1. Add the action to `ACTIONS` array in `src/lib/keybindings.ts` with `id`, `label`, `category`, `defaultShortcut`
2. Register a handler with `onAction(actionId, handler)` in the component where the action logic lives
3. Handlers registered in `AppShell.svelte` for global actions; component-specific actions register in their own `onMount`

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
```

Rust-only check:

```sh
cd src-tauri && cargo check
```

## Coding Conventions

### Rust

- Error type: `TwigError` (in `error.rs`), uses `thiserror`
- No `unwrap()`, no `expect()` in library code (main.rs `expect` on Tauri run is the one exception)
- Async all commands, even if the body is sync (Tauri requirement for `State<>` access)
- `serde::Serialize` on all types crossing the IPC boundary
- Command results that are write operations return `CommandResult { success, message }`

### Frontend

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

### LFS

LFS pointer files in diffs are detected by checking for `version https://git-lfs.github.com/spec/` in the diff content. They display as "LFS object -- [size]" instead of raw pointer text. All LFS operations go through the system git CLI.

## Settings Architecture

Settings are stored separately from session state:

- **Session** (`session.json`): ephemeral layout state -- open repos, active tab, panel sizes. Managed by `src/lib/stores/repos.ts`.
- **Settings** (`settings.json`): durable user preferences -- UI options, diff preferences, keybinding overrides. Managed by `src/lib/stores/settings.ts`.
- **Git config** (`~/.gitconfig`): identity, pull strategy, signing. Read/written via `git config --global` CLI commands in `src-tauri/src/commands/git_config.rs`.

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

- Authentication / credential manager
- ~~GitHub API integration~~ ✓ (clone from GitHub, create repo, create PR via PAT in Settings > GitHub)
- SSH key management
- Conflict resolution UI
- Blame view
- ~~Git identity profiles~~ (use local git config directly, no extra abstraction needed)
- ~~Light theme~~ ✓ (dark/light toggle in Appearance settings, `[data-theme="light"]` in `app.css`)
- ~~Stash management UI~~ ✓ (list/push/pop/apply/drop via `StashPanel.svelte`, includes untracked files)
- Interactive rebase UI
- File history view
- Submodule support
