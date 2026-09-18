#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn set_windows_app_identity() {
    use windows::core::w;
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(w!("app.ytmusic.desktop"));
    }
}

#[cfg(not(windows))]
fn set_windows_app_identity() {}

const CHROME_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

fn main() {
    let flags = [
        "--autoplay-policy=no-user-gesture-required",
        "--disable-features=CalculateNativeWinOcclusion,CalculateNativeWinOcclusionTracking,IntensiveWakeUpThrottling",
        "--disable-backgrounding-occluded-windows",
        "--disable-renderer-backgrounding",
        "--disable-background-timer-throttling",
        &format!("--user-agent=\"{CHROME_USER_AGENT}\""),
    ]
    .join(" ");

    let combined = match std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS") {
        Ok(existing) if !existing.trim().is_empty() => format!("{existing} {flags}"),
        _ => flags,
    };
    std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", combined);
    set_windows_app_identity();
    yt_music_tauri_lib::run();
}
