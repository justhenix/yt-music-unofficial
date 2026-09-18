use crate::{presence::TrackMetadata, settings, settings::SharedSettings};
use tauri::{AppHandle, Manager, WebviewWindow};

#[cfg(windows)]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, OnceLock,
};
#[cfg(windows)]
use windows::{
    core::{w, Result as WindowsResult},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::{
            CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPV5HEADER,
            BI_BITFIELDS, DIB_RGB_COLORS,
        },
        System::Com::{CoCreateInstance, CoInitialize, CLSCTX_INPROC_SERVER},
        UI::{
            Shell::{
                DefSubclassProc, ITaskbarList3, SetWindowSubclass, TaskbarList, THBF_ENABLED,
                THBF_HIDDEN, THBN_CLICKED, THB_FLAGS, THB_ICON, THB_TOOLTIP, THUMBBUTTON,
            },
            WindowsAndMessaging::{
                CreateIconIndirect, GetSystemMetrics, RegisterWindowMessageW, HICON, ICONINFO,
                SM_CXSMICON, SM_CYSMICON, WM_COMMAND,
            },
        },
    },
};

#[cfg(windows)]
const SUBCLASS_ID: usize = 0x5954_4D54;
#[cfg(windows)]
const PREVIOUS_BUTTON: u32 = 0x5001;
#[cfg(windows)]
const PLAY_PAUSE_BUTTON: u32 = 0x5002;
#[cfg(windows)]
const NEXT_BUTTON: u32 = 0x5003;

#[cfg(windows)]
static APP: OnceLock<AppHandle> = OnceLock::new();
#[cfg(windows)]
static SETTINGS: OnceLock<SharedSettings> = OnceLock::new();
#[cfg(windows)]
static LAST_TRACK: Mutex<Option<TrackMetadata>> = Mutex::new(None);
#[cfg(windows)]
static BUTTONS_INITIALIZED: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static WM_TASKBAR_BUTTON_CREATED: OnceLock<u32> = OnceLock::new();

#[cfg(windows)]
struct MediaIcons {
    previous: HICON,
    play: HICON,
    pause: HICON,
    next: HICON,
}

#[cfg(windows)]
unsafe impl Send for MediaIcons {}
#[cfg(windows)]
unsafe impl Sync for MediaIcons {}

#[cfg(windows)]
static ICONS: OnceLock<MediaIcons> = OnceLock::new();

#[cfg(windows)]
pub fn install(window: &WebviewWindow, settings: SharedSettings) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let _ = APP.set(window.app_handle().clone());
    let _ = SETTINGS.set(settings);

    unsafe {
        let msg = RegisterWindowMessageW(w!("TaskbarButtonCreated"));
        let _ = WM_TASKBAR_BUTTON_CREATED.set(msg);
        let _ = SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0);
    }
    update_buttons(hwnd, None);
}

#[cfg(windows)]
pub fn update(window: &WebviewWindow, track: Option<&TrackMetadata>) {
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    if let Ok(mut last) = LAST_TRACK.lock() {
        *last = track.cloned();
    }
    let hwnd_val = hwnd.0 as isize;
    let track_clone = track.cloned();
    let _ = window.run_on_main_thread(move || {
        let hwnd = HWND(hwnd_val as *mut _);
        update_buttons(hwnd, track_clone.as_ref());
    });
}

#[cfg(windows)]
pub fn refresh(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let last = LAST_TRACK.lock().ok().and_then(|t| t.clone());
    let hwnd_val = hwnd.0 as isize;
    let _ = window.run_on_main_thread(move || {
        let hwnd = HWND(hwnd_val as *mut _);
        update_buttons(hwnd, last.as_ref());
    });
}

#[cfg(windows)]
unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    _ref_data: usize,
) -> LRESULT {
    if let Some(&msg) = WM_TASKBAR_BUTTON_CREATED.get() {
        if message == msg && msg != 0 {
            BUTTONS_INITIALIZED.store(false, Ordering::Relaxed);
            let last = LAST_TRACK.lock().ok().and_then(|t| t.clone());
            update_buttons(hwnd, last.as_ref());
            return LRESULT(0);
        }
    }

    if message == WM_COMMAND {
        let notification = ((wparam.0 >> 16) & 0xffff) as u32;
        let button_id = (wparam.0 & 0xffff) as u32;
        if notification == THBN_CLICKED {
            if let Some(app) = APP.get() {
                match button_id {
                    PREVIOUS_BUTTON => media_action(app, "previous"),
                    PLAY_PAUSE_BUTTON => {
                        let toggled = if let Ok(mut last) = LAST_TRACK.lock() {
                            if let Some(track) = last.as_mut() {
                                track.playing = !track.playing;
                                Some(track.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        if let Some(track) = toggled.as_ref() {
                            update_buttons(hwnd, Some(track));
                        }
                        media_action(app, "play_pause");
                    }
                    NEXT_BUTTON => media_action(app, "next"),
                    _ => {}
                }
            }
            return LRESULT(0);
        }
    }

    DefSubclassProc(hwnd, message, wparam, lparam)
}

#[cfg(windows)]
fn media_action(app: &AppHandle, action: &str) {
    let script = match action {
        "previous" => {
            "(() => { const button = document.querySelector('ytmusic-player-bar #previous-button, ytmusic-player-bar #previous-song-button, ytmusic-player-bar .previous-button, ytmusic-player-bar [aria-label^=\"Previous\"]'); if (button) button.click(); else { const player = document.querySelector('#movie_player'); if (player && typeof player.previousVideo === 'function') player.previousVideo(); else { const media = document.querySelector('video, audio'); if (media) media.currentTime = 0; } } })();"
        }
        "play_pause" => {
            "(() => { const button = document.querySelector('ytmusic-player-bar #play-pause-button, ytmusic-player-bar .play-pause-button'); if (button) button.click(); else { const player = document.querySelector('#movie_player'); if (player && typeof player.playVideo === 'function') { player.getPlayerState() === 1 ? player.pauseVideo() : player.playVideo(); } else { const media = document.querySelector('video.video-stream, video.html5-main-video, video, audio'); if (media) media.paused ? media.play() : media.pause(); } } })();"
        }
        "next" => {
            "(() => { const button = document.querySelector('ytmusic-player-bar #next-button, ytmusic-player-bar #next-song-button, ytmusic-player-bar .next-button, ytmusic-player-bar [aria-label^=\"Next\"]'); if (button) button.click(); else { const player = document.querySelector('#movie_player'); if (player && typeof player.nextVideo === 'function') player.nextVideo(); else { const media = document.querySelector('video, audio'); if (media && Number.isFinite(media.duration)) media.currentTime = media.duration; } } })();"
        }
        _ => return,
    };

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval(script);
    }
}

#[cfg(windows)]
fn get_media_icons() -> Option<&'static MediaIcons> {
    ICONS.get_or_init(|| {
        MediaIcons {
            previous: media_icon(IconKind::Previous).unwrap_or_default(),
            play: media_icon(IconKind::Play).unwrap_or_default(),
            pause: media_icon(IconKind::Pause).unwrap_or_default(),
            next: media_icon(IconKind::Next).unwrap_or_default(),
        }
    });
    ICONS.get()
}

#[cfg(windows)]
fn update_buttons(hwnd: HWND, track: Option<&TrackMetadata>) {
    let enabled = SETTINGS
        .get()
        .map(settings::snapshot)
        .is_some_and(|value| value.windows_media_controls);
    let playing = track.is_some_and(|value| value.playing);

    let Ok(taskbar) = taskbar() else {
        return;
    };
    let Some(icons) = get_media_icons() else {
        return;
    };
    let play_pause_icon = if playing {
        icons.pause
    } else {
        icons.play
    };

    let buttons = [
        button(PREVIOUS_BUTTON, icons.previous, "Previous", enabled),
        button(
            PLAY_PAUSE_BUTTON,
            play_pause_icon,
            if playing { "Pause" } else { "Play" },
            enabled,
        ),
        button(NEXT_BUTTON, icons.next, "Next", enabled),
    ];

    unsafe {
        if BUTTONS_INITIALIZED.load(Ordering::Relaxed) {
            if taskbar.ThumbBarUpdateButtons(hwnd, &buttons).is_err()
                && taskbar.ThumbBarAddButtons(hwnd, &buttons).is_ok()
            {
                BUTTONS_INITIALIZED.store(true, Ordering::Relaxed);
            }
        } else if taskbar.ThumbBarAddButtons(hwnd, &buttons).is_ok()
            || taskbar.ThumbBarUpdateButtons(hwnd, &buttons).is_ok()
        {
            BUTTONS_INITIALIZED.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(windows)]
fn taskbar() -> WindowsResult<ITaskbarList3> {
    unsafe {
        let _ = CoInitialize(None);
        let taskbar: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)?;
        let _ = taskbar.HrInit();
        Ok(taskbar)
    }
}

#[cfg(windows)]
fn button(id: u32, icon: HICON, tooltip: &str, enabled: bool) -> THUMBBUTTON {
    let mut button = THUMBBUTTON {
        dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
        iId: id,
        hIcon: icon,
        dwFlags: if enabled { THBF_ENABLED } else { THBF_HIDDEN },
        ..Default::default()
    };
    write_tooltip(&mut button.szTip, tooltip);
    button
}

#[cfg(windows)]
fn write_tooltip(target: &mut [u16; 260], value: &str) {
    for (slot, value) in target.iter_mut().zip(value.encode_utf16().take(259)) {
        *slot = value;
    }
}

#[cfg(windows)]
#[derive(Clone, Copy)]
enum IconKind {
    Previous,
    Play,
    Pause,
    Next,
}

#[cfg(windows)]
fn media_icon(kind: IconKind) -> WindowsResult<HICON> {
    let size = unsafe {
        GetSystemMetrics(SM_CXSMICON)
            .min(GetSystemMetrics(SM_CYSMICON))
            .clamp(16, 64)
    } as usize;
    let mut pixels = vec![0u8; size * size * 4];

    for y in 0..size {
        for x in 0..size {
            let mut inside_count = 0u32;
            for sy in 0..4 {
                for sx in 0..4 {
                    let px = (x as f32 + (sx as f32 + 0.5) / 4.0) / (size as f32);
                    let py = (y as f32 + (sy as f32 + 0.5) / 4.0) / (size as f32);
                    if shape_contains(kind, px, py) {
                        inside_count += 1;
                    }
                }
            }
            if inside_count > 0 {
                let alpha = ((inside_count * 255 + 8) / 16) as u8;
                let offset = (y * size + x) * 4;
                pixels[offset] = alpha;     // Blue (premultiplied: white * alpha)
                pixels[offset + 1] = alpha; // Green
                pixels[offset + 2] = alpha; // Red
                pixels[offset + 3] = alpha; // Alpha
            }
        }
    }

    let header = BITMAPV5HEADER {
        bV5Size: std::mem::size_of::<BITMAPV5HEADER>() as u32,
        bV5Width: size as i32,
        bV5Height: -(size as i32),
        bV5Planes: 1,
        bV5BitCount: 32,
        bV5Compression: BI_BITFIELDS,
        bV5RedMask: 0x00FF_0000,
        bV5GreenMask: 0x0000_FF00,
        bV5BlueMask: 0x0000_00FF,
        bV5AlphaMask: 0xFF00_0000,
        ..Default::default()
    };

    unsafe {
        let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let hbm_color = CreateDIBSection(
            None,
            &header as *const _ as *const BITMAPINFO,
            DIB_RGB_COLORS,
            &mut bits_ptr,
            None,
            0,
        )?;

        if !bits_ptr.is_null() {
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits_ptr as *mut u8, pixels.len());
        }

        let hbm_mask = CreateBitmap(size as i32, size as i32, 1, 1, None);

        let icon_info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };

        let icon = CreateIconIndirect(&icon_info);

        let _ = DeleteObject(hbm_color.into());
        let _ = DeleteObject(hbm_mask.into());

        icon
    }
}

#[cfg(windows)]
fn shape_contains(kind: IconKind, px: f32, py: f32) -> bool {
    match kind {
        IconKind::Play => {
            // Right-pointing triangle with rounded corners
            let v0 = (0.35, 0.26);
            let v1 = (0.35, 0.74);
            let v2 = (0.71, 0.50);
            inside_rounded_triangle(px, py, v0, v1, v2, 0.035)
        }
        IconKind::Pause => {
            // Two vertical rounded capsules (smooth bars like VLC)
            inside_capsule(px, py, 0.38, 0.31, 0.69, 0.055)
                || inside_capsule(px, py, 0.62, 0.31, 0.69, 0.055)
        }
        IconKind::Previous => {
            // Vertical bar on left + two left-pointing triangles
            inside_capsule(px, py, 0.22, 0.30, 0.70, 0.035)
                || inside_rounded_triangle(
                    px,
                    py,
                    (0.28, 0.50), // left tip
                    (0.48, 0.29), // top right
                    (0.48, 0.71), // bottom right
                    0.025,
                )
                || inside_rounded_triangle(
                    px,
                    py,
                    (0.53, 0.50), // left tip
                    (0.73, 0.29), // top right
                    (0.73, 0.71), // bottom right
                    0.025,
                )
        }
        IconKind::Next => {
            // Two right-pointing triangles + vertical bar on right
            inside_rounded_triangle(
                px,
                py,
                (0.27, 0.29), // top left
                (0.27, 0.71), // bottom left
                (0.47, 0.50), // right tip
                0.025,
            ) || inside_rounded_triangle(
                px,
                py,
                (0.52, 0.29), // top left
                (0.52, 0.71), // bottom left
                (0.72, 0.50), // right tip
                0.025,
            ) || inside_capsule(px, py, 0.78, 0.30, 0.70, 0.035)
        }
    }
}

#[cfg(windows)]
fn inside_capsule(px: f32, py: f32, cx: f32, y_top: f32, y_bottom: f32, r: f32) -> bool {
    let cy = py.clamp(y_top, y_bottom);
    let dx = px - cx;
    let dy = py - cy;
    dx * dx + dy * dy <= r * r
}

#[cfg(windows)]
fn inside_rounded_triangle(
    px: f32,
    py: f32,
    v0: (f32, f32),
    v1: (f32, f32),
    v2: (f32, f32),
    r: f32,
) -> bool {
    let p = (px, py);
    if inside_triangle(p, v0, v1, v2) {
        return true;
    }
    let r_sq = r * r;
    dist_sq_to_segment(p, v0, v1) <= r_sq
        || dist_sq_to_segment(p, v1, v2) <= r_sq
        || dist_sq_to_segment(p, v2, v0) <= r_sq
}

#[cfg(windows)]
fn inside_triangle(p: (f32, f32), v0: (f32, f32), v1: (f32, f32), v2: (f32, f32)) -> bool {
    let sign = |p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)| -> f32 {
        (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1)
    };
    let d1 = sign(p, v0, v1);
    let d2 = sign(p, v1, v2);
    let d3 = sign(p, v2, v0);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

#[cfg(windows)]
fn dist_sq_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let ab = (b.0 - a.0, b.1 - a.1);
    let ap = (p.0 - a.0, p.1 - a.1);
    let ab_len_sq = ab.0 * ab.0 + ab.1 * ab.1;
    if ab_len_sq <= f32::EPSILON {
        return ap.0 * ap.0 + ap.1 * ap.1;
    }
    let t = ((ap.0 * ab.0 + ap.1 * ab.1) / ab_len_sq).clamp(0.0, 1.0);
    let proj = (a.0 + t * ab.0, a.1 + t * ab.1);
    let dx = p.0 - proj.0;
    let dy = p.1 - proj.1;
    dx * dx + dy * dy
}

#[cfg(not(windows))]
pub fn install(_window: &WebviewWindow, _settings: SharedSettings) {}

#[cfg(not(windows))]
pub fn update(_window: &WebviewWindow, _track: Option<&TrackMetadata>) {}

#[cfg(not(windows))]
pub fn refresh(_app: &AppHandle) {}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::*;

    #[test]
    #[cfg(windows)]
    fn tooltip_is_null_terminated() {
        let mut buffer = [0u16; 260];
        write_tooltip(&mut buffer, "Play");
        assert_eq!(&buffer[..5], &[80, 108, 97, 121, 0]);
    }

    #[test]
    #[cfg(windows)]
    fn generated_icons_have_pixels() {
        for kind in [
            IconKind::Previous,
            IconKind::Play,
            IconKind::Pause,
            IconKind::Next,
        ] {
            let samples = (0..32)
                .flat_map(|y| {
                    (0..32).map(move |x| {
                        let px = (x as f32 + 0.5) / 32.0;
                        let py = (y as f32 + 0.5) / 32.0;
                        shape_contains(kind, px, py)
                    })
                })
                .filter(|value| *value)
                .count();
            assert!(samples > 30, "kind {:?} had only {} samples", kind as u8, samples);
        }
    }

    #[test]
    #[cfg(windows)]
    fn media_icons_are_cached_statically() {
        let first = get_media_icons().expect("icons should initialize");
        let second = get_media_icons().expect("icons should be cached");

        assert_eq!(first.previous, second.previous);
        assert_eq!(first.play, second.play);
        assert_eq!(first.pause, second.pause);
        assert_eq!(first.next, second.next);
        assert!(!first.play.is_invalid());
    }

    #[test]
    #[cfg(windows)]
    fn last_track_state_persists_for_refresh() {
        let metadata = TrackMetadata {
            title: "Test Song".to_string(),
            artist: Some("Test Artist".to_string()),
            album: None,
            playing: true,
            url: None,
            cover_url: None,
            elapsed_seconds: Some(10),
            duration_seconds: Some(200),
        };

        if let Ok(mut last) = LAST_TRACK.lock() {
            *last = Some(metadata.clone());
        }

        let retrieved = LAST_TRACK.lock().ok().and_then(|t| t.clone());
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test Song");
    }
}
