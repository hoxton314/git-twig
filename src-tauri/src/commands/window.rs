/// Detect whether the current desktop environment is a tiling window manager.
/// On tiling WMs the app skips rendering its own title bar since the WM
/// handles window placement and controls via keyboard shortcuts.
#[tauri::command]
pub async fn is_tiling_wm() -> bool {
    let tiling_wms = [
        "hyprland",
        "sway",
        "i3",
        "bspwm",
        "dwm",
        "awesome",
        "xmonad",
        "qtile",
        "herbstluftwm",
        "river",
        "niri",
    ];

    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();

    let session = std::env::var("DESKTOP_SESSION")
        .unwrap_or_default()
        .to_lowercase();

    tiling_wms
        .iter()
        .any(|wm| desktop.contains(wm) || session.contains(wm))
}
