// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    setup_linux_display();

    twig_lib::run();
}

/// Configure WebKitGTK rendering for the detected GPU + display server.
///
/// Must run before any GTK/GDK initialization (i.e. before `tauri::Builder`).
///
/// - AMD / Intel on Wayland: DMA-BUF works natively, force GLES for best
///   format compatibility.
/// - NVIDIA on Wayland: the proprietary driver's DMA-BUF implementation
///   advertises buffer format+modifier pairs that WebKitGTK's renderer
///   requests but that wlroots-based compositors (Hyprland, Sway) reject,
///   causing a fatal `wl_display` protocol error (EPROTO 71).
///   Redirect WebKitGTK to its SHM fallback renderer which still
///   composites through the Wayland surface protocol — the window, input,
///   and display pipeline remain fully Wayland-native.
/// - X11 / XWayland: no intervention needed.
#[cfg(target_os = "linux")]
fn setup_linux_display() {
    use std::env;
    use std::path::Path;

    let on_wayland = env::var("WAYLAND_DISPLAY").is_ok()
        || env::var("XDG_SESSION_TYPE").map_or(false, |v| v == "wayland");

    if !on_wayland {
        return;
    }

    let has_nvidia = Path::new("/proc/driver/nvidia").exists();

    if has_nvidia {
        // NVIDIA proprietary: DMA-BUF modifier negotiation is broken with
        // WebKitGTK on wlroots compositors.  Fall back to SHM rendering
        // inside WebKit; the Wayland surface path is unaffected.
        if env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            unsafe { env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
        }
    }

    // Prefer GLES/EGL over desktop-GL/GLX on all GPUs for Wayland.
    if env::var("GDK_GL").is_err() {
        unsafe { env::set_var("GDK_GL", "gles") };
    }
}
