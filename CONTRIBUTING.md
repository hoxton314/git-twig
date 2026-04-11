# Contributing to Twig

Thanks for your interest in contributing to Twig!

## Getting Started

1. Fork the repository
2. Clone your fork and install dependencies:
   ```bash
   npm install
   ```
3. Start the development server:
   ```bash
   npm run tauri dev
   ```

## Development

- **Frontend**: Svelte 5 with runes, TypeScript, TailwindCSS v4
- **Backend**: Rust with Tauri v2, git2 for reads, system git CLI for writes
- See `AGENTS.md` for full architecture rules and coding conventions

## Pull Requests

- Keep PRs focused on a single change
- Run `npm run check` before submitting to catch type errors
- Test on Linux Wayland if possible (primary target platform)

## Reporting Issues

Open an issue on GitHub with:
- Steps to reproduce
- Expected vs actual behavior
- OS and desktop environment (X11/Wayland, compositor)

## License

By contributing, you agree that your contributions will be licensed under the [AGPL-3.0](LICENSE).
