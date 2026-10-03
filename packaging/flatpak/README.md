# Flatpak

`dev.twig.app.yml` packages Twig for Flatpak on the GNOME 51 runtime (it
provides WebKitGTK 4.1; keep it on a runtime Flathub still supports). It repackages the release `.deb` built from this
repository, plus a git built from source (the runtime has none).

CI builds it on every PR that touches packaging (`.github/workflows/packaging.yml`,
job *flatpak*) and uploads `twig.flatpak` as an artifact.

## Build locally

```sh
# 1. Build the .deb
npx tauri build --bundles deb
cp src-tauri/target/release/bundle/deb/*.deb packaging/flatpak/twig.deb

# 2. Build and install the Flatpak (needs flatpak-builder and the Flathub remote)
flatpak install --user flathub org.gnome.Platform//51 org.gnome.Sdk//51
flatpak-builder --user --install --force-clean build-dir packaging/flatpak/dev.twig.app.yml
flatpak run dev.twig.app
```

## Sandbox permissions

| Permission | Why |
| --- | --- |
| `--filesystem=home`, `/media`, `/run/media` | Repositories can be anywhere; a Git client can't work through the file chooser portal |
| `--share=network` | fetch / push / clone and the GitHub / GitLab / Gitea APIs |
| `--socket=ssh-auth` | SSH remotes through your ssh-agent |
| `--talk-name=org.freedesktop.secrets` | Tokens are stored in the OS keyring |
| `--talk-name=org.freedesktop.Notifications` | New-commit and CI notifications |
| Wayland / X11 / DRI | Display and GPU |

### Inside the sandbox

- **git** is bundled and reads your host configuration (`~/.gitconfig`,
  `~/.config/git/config`, `~/.config/git/ignore`): a small wrapper undoes Flatpak's
  `XDG_CONFIG_HOME` redirect for git only.
- **Credentials:** GitHub, GitLab and Gitea tokens come from Twig's keyring. For other
  HTTPS hosts, `credential.helper=libsecret` works (the helper is bundled); helpers
  installed on the host (`gh`, Git Credential Manager, …) aren't available.
- **Open in terminal / editor** start programs inside the sandbox, which has no host
  terminal or editor. To use host ones, allow `--talk-name=org.freedesktop.Flatpak`
  (e.g. with Flatseal; this grants access to run host commands) and set the commands in
  Settings > Editor & Diff to `flatpak-spawn --host code {path}` /
  `flatpak-spawn --host gnome-terminal --working-directory={path}`.

Commit signing with GPG uses the sandbox's own `gpg`; to sign with your host keys, add
`--filesystem=xdg-run/gnupg:ro` (e.g. with Flatseal). SSH signing works through the agent.

The in-app updater stays off in the Flatpak (Flatpak updates the app).

## Flathub submission (human step)

Flathub builds from source in its own infrastructure, so a submission needs a
manifest that builds Twig from a tagged source tarball (cargo + npm vendored
sources via `flatpak-builder-tools`), not from the `.deb`:

1. Tag a release (`vX.Y.Z`).
2. Generate `cargo-sources.json` and `node-sources.json` with
   `flatpak-builder-tools` for that tag.
3. Fork `flathub/flathub`, add `dev.twig.app.yml` (source build) and open the
   submission PR following <https://docs.flathub.org/docs/for-app-authors/submission>.
4. Verify ownership of the `dev.twig` app id domain (or switch to
   `io.github.hoxton314.Twig`).
