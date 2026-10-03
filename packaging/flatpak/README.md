# Flatpak

`dev.twig.app.yml` packages Twig for Flatpak on the GNOME 48 runtime (it
provides WebKitGTK 4.1). It repackages the release `.deb` built from this
repository, plus a git built from source (the runtime has none).

CI builds it on every PR that touches packaging (`.github/workflows/packaging.yml`,
job *flatpak*) and uploads `twig.flatpak` as an artifact.

## Build locally

```sh
# 1. Build the .deb
npx tauri build --bundles deb
cp src-tauri/target/release/bundle/deb/*.deb packaging/flatpak/twig.deb

# 2. Build and install the Flatpak (needs flatpak-builder and the Flathub remote)
flatpak install --user flathub org.gnome.Platform//48 org.gnome.Sdk//48
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
