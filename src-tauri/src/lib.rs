//! Builds the Tauri window, injects page probes, and gates navigation/title messages.

mod adblock;
mod controls;
mod platform;
mod presence;
mod settings;
mod updates;
mod url_policy;
mod windows_media;

use adblock::AdBlockController;
use controls::AppState;
use presence::{PresenceController, PresenceMessage};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::webview::{NewWindowResponse, WebviewWindowBuilder};
use tauri::{Manager, Url, WebviewUrl, WindowEvent};
use tauri_plugin_window_state::StateFlags;
use url_policy::{is_allowed_navigation_url, is_youtube_music_url};

const YOUTUBE_MUSIC_URL: &str = "https://music.youtube.com";
const AD_BLOCK_SCRIPT: &str = include_str!("adblock_probe.js");
const TRACK_PROBE_SCRIPT: &str = include_str!("track_probe.js");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NewWindowAction {
    NavigateInMainWebview,
    OpenExternal,
    Deny,
}

// Hidden diagnostics hook: run with YT_MUSIC_ADBLOCK_SELF_TEST=1 and the window
// title should briefly become ADBLOCK_SELF_TEST:PASS when native blocking works.
const AD_BLOCK_SELF_TEST_SCRIPT: &str = r#"
(() => {
  if (!window.__ytMusicTauriAdBlockSelfTest) {
    window.__ytMusicTauriAdBlockSelfTest = true;
    fetch("https://googleads.g.doubleclick.net/pagead/id", { cache: "no-store" })
      .then((response) => {
        document.title = response.status === 204
          ? "ADBLOCK_SELF_TEST:PASS"
          : `ADBLOCK_SELF_TEST:FAIL:${response.status}`;
      })
      .catch((error) => {
        document.title = `ADBLOCK_SELF_TEST:FAIL:${error && error.name ? error.name : "ERROR"}`;
      });
  }
})();
"#;

const CHROME_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = settings::load();
    settings::update(&settings, |value| {
        value.launch_at_startup = platform::startup_enabled()
    });
    let initial = settings::snapshot(&settings);
    let presence = PresenceController::new(initial.discord_rpc);
    let adblock = AdBlockController::new(initial.ad_block);
    let presence_for_navigation = presence.clone();
    let presence_for_window = presence.clone();
    let adblock_for_webview = adblock.clone();
    let state = AppState {
        settings,
        presence,
        adblock,
        quitting: Arc::new(AtomicBool::new(false)),
    };
    let state_for_events = state.clone();
    let start_in_tray =
        std::env::args().any(|argument| argument == "--tray" || argument == "--start-to-tray");
    let start_minimized = std::env::args().any(|argument| argument == "--minimized")
        || (initial.start_minimized && !start_in_tray);
    let should_start_hidden = start_in_tray || start_minimized;

    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_global_shortcut::Builder::new().build())
            .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                controls::show_main_window(app);
            }));
    }

    builder
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    StateFlags::all() & !StateFlags::VISIBLE & !StateFlags::FULLSCREEN,
                )
                .build(),
        )
        .on_window_event(move |window, event| match event {
            WindowEvent::Focused(focused) if window.label() == "main" => {
                controls::set_local_shortcuts(window.app_handle(), *focused, &state_for_events);
            }
            WindowEvent::CloseRequested { api, .. }
                if settings::snapshot(&state_for_events.settings).close_to_tray
                    && !state_for_events.quitting.load(Ordering::Relaxed) =>
            {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed => {
                state_for_events.presence.clear();
                if let Some(main_window) = window.app_handle().get_webview_window("main") {
                    windows_media::update(&main_window, None);
                }
            }
            _ => {}
        })
        .setup(move |app| {
            let music_url = YOUTUBE_MUSIC_URL
                .parse()
                .expect("static YouTube Music URL must be valid");
            let blank_url = "about:blank"
                .parse()
                .expect("static about:blank URL must be valid");
            let app_for_navigation = app.handle().clone();
            let app_for_new_window = app.handle().clone();
            let settings_for_media = state.settings.clone();
            let menu_components = controls::build_app_menu(app, &initial)?;

            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(blank_url))
                .title("YouTube Music")
                .inner_size(1280.0, 840.0)
                .min_inner_size(900.0, 620.0)
                .center()
                .visible(!should_start_hidden)
                .zoom_hotkeys_enabled(false)
                .user_agent(CHROME_USER_AGENT)
                .menu(menu_components.menu)
                .initialization_script(initialization_script(initial.ad_block))
                .on_navigation(move |url| {
                    if url_policy::is_auth_recovery_url(url) {
                        static LAST_RECOVERY: std::sync::atomic::AtomicU64 =
                            std::sync::atomic::AtomicU64::new(0);
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let last = LAST_RECOVERY.load(std::sync::atomic::Ordering::Relaxed);
                        if now.saturating_sub(last) >= 3 {
                            LAST_RECOVERY.store(now, std::sync::atomic::Ordering::Relaxed);
                            if let Some(window) = app_for_navigation.get_webview_window("main") {
                                if let Ok(target) = Url::parse(YOUTUBE_MUSIC_URL) {
                                    let _ = window.navigate(target);
                                }
                            }
                        }
                        return false;
                    }

                    let allowed = is_allowed_navigation_url(url);

                    if !is_youtube_music_url(url) && !url_policy::is_auth_intermediate_url(url) {
                        presence_for_navigation.clear();
                        if let Some(window) = app_for_navigation.get_webview_window("main") {
                            windows_media::update(&window, None);
                        }
                    }
                    if !allowed && url.scheme() == "https" {
                        platform::open_url(url.as_str());
                    }

                    allowed
                })
                .on_new_window(move |url, _features| {
                    match new_window_action(&url) {
                        NewWindowAction::NavigateInMainWebview => {
                            if let Some(window) = app_for_new_window.get_webview_window("main") {
                                let _ = window.navigate(url);
                            }
                        }
                        NewWindowAction::OpenExternal => {
                            platform::open_url(url.as_str());
                        }
                        NewWindowAction::Deny => {}
                    }

                    NewWindowResponse::Deny
                })
                .on_document_title_changed(move |window, title| {
                    // track_probe.js sends JSON through document.title so the remote
                    // YouTube page never gets direct access to Tauri commands.
                    if presence::is_presence_title_message(&title) {
                        if let Some(message) = presence::parse_presence_title(&title) {
                            if should_accept_presence_message(window.url().ok().as_ref(), &message)
                            {
                                match message {
                                    PresenceMessage::Track(track) => {
                                        let window_title = track.window_title();
                                        let _ = window.set_title(&window_title);
                                        windows_media::update(&window, Some(&track));
                                        presence_for_window.update(track);
                                    }
                                    PresenceMessage::Clear => {
                                        let _ = window.set_title("YouTube Music");
                                        windows_media::update(&window, None);
                                        presence_for_window.clear();
                                    }
                                }
                            }
                        }
                        return;
                    }

                    if !title.trim().is_empty() {
                        let _ = window.set_title(&title);
                    }
                })
                .build()?;
            controls::install(app, state, menu_components.checks)?;
            windows_media::install(&window, settings_for_media);
            let _ = window.set_zoom(initial.zoom.clamp(0.5, 2.0));

            let window_for_webview = window.clone();
            let _ = window.with_webview(move |webview| {
                adblock_for_webview.install(&webview);

                #[cfg(windows)]
                unsafe {
                    if let Ok(core_webview) = webview.controller().CoreWebView2() {
                        use webview2_com::ContainsFullScreenElementChangedEventHandler;
                        let window_fs = window_for_webview.clone();
                        let handler = ContainsFullScreenElementChangedEventHandler::create(Box::new(
                            move |sender, _| {
                                let mut contains = windows::core::BOOL::default();
                                if let Some(sender) = sender {
                                    let _ = sender.ContainsFullScreenElement(&mut contains);
                                }
                                let is_fullscreen = contains.as_bool();
                                let _ = window_fs.set_fullscreen(is_fullscreen);
                                if is_fullscreen {
                                    let _ = window_fs.hide_menu();
                                } else {
                                    let _ = window_fs.show_menu();
                                }
                                Ok(())
                            },
                        ));
                        let mut token = 0i64;
                        let _ = core_webview
                            .add_ContainsFullScreenElementChanged(&handler, &mut token);
                    }
                }
            });

            window.navigate(music_url)?;
            if start_in_tray {
                let _ = window.hide();
            } else if start_minimized {
                #[cfg(windows)]
                if let Ok(hwnd) = window.hwnd() {
                    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWMINNOACTIVE};
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_SHOWMINNOACTIVE);
                    }
                }
                let _ = window.minimize();
            }
            updates::check_in_background(true);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running YouTube Music");
}

fn initialization_script(ad_block_enabled: bool) -> String {
    let enabled = format!("window.__ytMusicTauriAdBlockEnabled = {ad_block_enabled};");

    if std::env::var_os("YT_MUSIC_ADBLOCK_SELF_TEST").is_some() {
        format!("{enabled}\n{AD_BLOCK_SCRIPT}\n{AD_BLOCK_SELF_TEST_SCRIPT}\n{TRACK_PROBE_SCRIPT}")
    } else {
        format!("{enabled}\n{AD_BLOCK_SCRIPT}\n{TRACK_PROBE_SCRIPT}")
    }
}

fn should_accept_presence_message(current_url: Option<&Url>, message: &PresenceMessage) -> bool {
    match message {
        PresenceMessage::Track(_) => current_url.is_some_and(is_youtube_music_url),
        PresenceMessage::Clear => current_url.is_some_and(is_allowed_navigation_url),
    }
}

fn new_window_action(url: &Url) -> NewWindowAction {
    if url.scheme() == "https" && is_allowed_navigation_url(url) {
        NewWindowAction::NavigateInMainWebview
    } else if url.as_str() == "about:blank" {
        NewWindowAction::Deny
    } else {
        NewWindowAction::OpenExternal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_url(value: &str) -> Url {
        Url::parse(value).expect("test URL must parse")
    }

    #[test]
    fn accepts_track_titles_only_from_youtube_music() {
        let title = r#"YTMRPC:{"title":"Song","artist":null,"album":null,"playing":true,"url":null,"cover_url":null,"elapsed_seconds":null,"duration_seconds":null}"#;
        let message = presence::parse_presence_title(title).expect("presence message");
        let music_url = parse_url("https://music.youtube.com/watch?v=x");
        let account_url = parse_url("https://accounts.google.com/signin");

        assert!(should_accept_presence_message(Some(&music_url), &message));
        assert!(!should_accept_presence_message(
            Some(&account_url),
            &message
        ));
        assert!(!should_accept_presence_message(None, &message));
    }

    #[test]
    fn accepts_clear_titles_from_allowed_pages() {
        let message =
            presence::parse_presence_title(r#"YTMRPC:{"type":"clear"}"#).expect("clear message");
        let music_url = parse_url("https://music.youtube.com/watch?v=x");
        let account_url = parse_url("https://accounts.google.com/signin");
        let external_url = parse_url("https://example.com/");

        assert!(should_accept_presence_message(Some(&music_url), &message));
        assert!(should_accept_presence_message(Some(&account_url), &message));
        assert!(!should_accept_presence_message(
            Some(&external_url),
            &message
        ));
        assert!(!should_accept_presence_message(None, &message));
    }

    #[test]
    fn ignores_non_track_titles_for_track_bridge() {
        assert!(presence::parse_presence_title("YouTube Music").is_none());
    }

    #[test]
    fn routes_allowed_new_windows_back_into_main_webview() {
        let music_url = parse_url("https://music.youtube.com/");
        let account_url = parse_url("https://accounts.google.com/signin");
        let external_url = parse_url("https://example.com/");
        let blank_url = parse_url("about:blank");

        assert_eq!(
            new_window_action(&music_url),
            NewWindowAction::NavigateInMainWebview
        );
        assert_eq!(
            new_window_action(&account_url),
            NewWindowAction::NavigateInMainWebview
        );
        assert_eq!(
            new_window_action(&external_url),
            NewWindowAction::OpenExternal
        );
        assert_eq!(new_window_action(&blank_url), NewWindowAction::Deny);
    }
}
