/// Whether this install can update itself through the Tauri updater.
///
/// On Linux the updater replaces the package the binary was bundled in
/// (AppImage file, or `dpkg -i` / `rpm -U` for deb/rpm). That is only
/// correct when that package manager actually owns the running binary:
/// the AUR `twig-bin` package ships the binary extracted from the `.deb`,
/// so it reports a deb bundle but must be updated through pacman instead.
/// Dev builds report no bundle type at all.
#[tauri::command]
pub async fn updater_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        use tauri::utils::{config::BundleType, platform::bundle_type};

        // Flatpak updates the app itself (the binary inside is the deb build).
        if std::env::var_os("FLATPAK_ID").is_some() {
            return false;
        }

        match bundle_type() {
            Some(BundleType::AppImage) => std::env::var_os("APPIMAGE").is_some(),
            Some(BundleType::Deb) => exe_owned_by("dpkg", "-S").await,
            Some(BundleType::Rpm) => exe_owned_by("rpm", "-qf").await,
            _ => false,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// Ask a package manager whether it owns the running executable.
/// Returns false if the tool is missing (e.g. no dpkg on Arch).
#[cfg(target_os = "linux")]
async fn exe_owned_by(tool: &str, query_flag: &str) -> bool {
    let Ok(exe) = std::env::current_exe().and_then(|p| p.canonicalize()) else {
        return false;
    };
    tokio::process::Command::new(tool)
        .arg(query_flag)
        .arg(exe)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}
