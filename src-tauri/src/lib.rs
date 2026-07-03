//! lib.rs
//! Builds the Tauri window, injects page probes, and gates navigation/title messages.

mod adblock;
mod presence;
mod url_policy;

use presence::PresenceController;
use tauri::webview::WebviewWindowBuilder;
use tauri::{Manager, Url, WebviewUrl};
use url_policy::{is_allowed_navigation_url, is_youtube_music_url};

const YOUTUBE_MUSIC_URL: &str = "https://music.youtube.com";
const AD_BLOCK_SCRIPT: &str = include_str!("adblock_probe.js");
const TRACK_PROBE_SCRIPT: &str = include_str!("track_probe.js");
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let presence = PresenceController::new();
    let presence_for_window = presence.clone();

    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }));
    }

    builder
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(move |app| {
            let music_url = YOUTUBE_MUSIC_URL
                .parse()
                .expect("static YouTube Music URL must be valid");
            let blank_url = "about:blank"
                .parse()
                .expect("static about:blank URL must be valid");

            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(blank_url))
                .title("YouTube Music")
                .inner_size(1280.0, 840.0)
                .min_inner_size(900.0, 620.0)
                .center()
                .initialization_script(&initialization_script())
                .on_navigation(is_allowed_navigation_url)
                .on_document_title_changed(move |window, title| {
                    // track_probe.js sends JSON through document.title so the remote
                    // YouTube page never gets direct access to Tauri commands.
                    if presence::is_track_title_message(&title) {
                        if should_accept_track_title(window.url().ok().as_ref(), &title) {
                            if let Some(track) = presence::parse_track_title(&title) {
                                let window_title = track.window_title();
                                let _ = window.set_title(&window_title);
                                presence_for_window.update(track);
                            }
                        }
                        return;
                    }

                    if !title.trim().is_empty() {
                        let _ = window.set_title(&title);
                    }
                })
                .build()?;
            let _ = window.with_webview(adblock::install);
            window.navigate(music_url)?;

            Ok(())
        })
        .manage(presence)
        .run(tauri::generate_context!())
        .expect("error while running YouTube Music");
}

fn initialization_script() -> String {
    if std::env::var_os("YT_MUSIC_ADBLOCK_SELF_TEST").is_some() {
        format!("{AD_BLOCK_SCRIPT}\n{AD_BLOCK_SELF_TEST_SCRIPT}\n{TRACK_PROBE_SCRIPT}")
    } else {
        format!("{AD_BLOCK_SCRIPT}\n{TRACK_PROBE_SCRIPT}")
    }
}

fn should_accept_track_title(current_url: Option<&Url>, title: &str) -> bool {
    presence::is_track_title_message(title) && current_url.is_some_and(is_youtube_music_url)
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
        let music_url = parse_url("https://music.youtube.com/watch?v=x");
        let account_url = parse_url("https://accounts.google.com/signin");

        assert!(should_accept_track_title(Some(&music_url), title));
        assert!(!should_accept_track_title(Some(&account_url), title));
        assert!(!should_accept_track_title(None, title));
    }

    #[test]
    fn ignores_non_track_titles_for_track_bridge() {
        let music_url = parse_url("https://music.youtube.com/");

        assert!(!should_accept_track_title(
            Some(&music_url),
            "YouTube Music"
        ));
    }
}
