//! presence.rs
//! Converts trusted YouTube Music track metadata into Discord Rich Presence updates.

use crate::url_policy::{valid_artwork_url, valid_track_url};
use discord_rich_presence::{
    activity::{Activity, ActivityType, Assets, Button, StatusDisplayType, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use serde::Deserialize;
use std::{
    fs,
    path::PathBuf,
    sync::mpsc::{self, SyncSender},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

const TITLE_PREFIX: &str = "YTMRPC:";
const CLIENT_ID_ENV: &str = "YT_MUSIC_DISCORD_CLIENT_ID";
const BUNDLED_CLIENT_ID: &str = include_str!("../discord-client-id.txt");

#[derive(Clone)]
pub struct PresenceController {
    tx: SyncSender<TrackMetadata>,
}

#[derive(Default)]
struct PresenceState {
    client_id: Option<String>,
    client: Option<DiscordIpcClient>,
    last_track: Option<TrackMetadata>,
}

impl PresenceController {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel::<TrackMetadata>(1);

        thread::spawn(move || {
            let mut state = PresenceState {
                client_id: read_client_id(),
                client: None,
                last_track: None,
            };

            for track in rx {
                update_presence_state(&mut state, track);
            }
        });

        Self { tx }
    }

    pub fn update(&self, track: TrackMetadata) {
        if track.title.trim().is_empty() {
            return;
        }

        let _ = self.tx.try_send(track);
    }
}

fn update_presence_state(state: &mut PresenceState, track: TrackMetadata) {
    if state.last_track.as_ref() == Some(&track) {
        return;
    }

    let Some(client_id) = state.client_id.clone() else {
        state.last_track = Some(track);
        return;
    };

    if state.client.is_none() {
        let mut client = DiscordIpcClient::new(client_id);
        if client.connect().is_err() {
            state.last_track = Some(track);
            return;
        }
        state.client = Some(client);
    }

    let mut activity = Activity::new()
        .activity_type(ActivityType::Listening)
        .name("YouTube Music")
        .details(track.title.clone());

    if let Some(presence_state) = track.presence_state() {
        activity = activity
            .state(presence_state)
            .status_display_type(StatusDisplayType::State);
    }

    let activity = apply_activity_urls(activity, &track);
    let activity = apply_activity_assets(activity, &track);
    let activity = apply_activity_timestamps(activity, &track);
    let activity = apply_activity_buttons(activity, &track);

    let result = state
        .client
        .as_mut()
        .map(|client| client.set_activity(activity));

    if matches!(result, Some(Err(_))) {
        state.client = None;
    } else {
        state.last_track = Some(track);
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct TrackMetadata {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub playing: bool,
    pub url: Option<String>,
    pub cover_url: Option<String>,
    pub elapsed_seconds: Option<u64>,
    pub duration_seconds: Option<u64>,
}

impl TrackMetadata {
    pub fn window_title(&self) -> String {
        match self.artist.as_deref().filter(|artist| !artist.is_empty()) {
            Some(artist) => format!("{} - {} - YouTube Music", self.title, artist),
            None => format!("{} - YouTube Music", self.title),
        }
    }

    fn presence_state(&self) -> Option<String> {
        let mut parts = Vec::new();

        if let Some(artist) = clean_presence_part(self.artist.as_deref()) {
            parts.push(artist.to_string());
        }

        if !self.playing {
            parts.push("Paused".to_string());
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" - "))
        }
    }
}

pub fn parse_track_title(title: &str) -> Option<TrackMetadata> {
    // Only consume titles emitted by track_probe.js; normal YouTube titles still
    // pass through to the window unchanged.
    let payload = title.strip_prefix(TITLE_PREFIX)?;
    serde_json::from_str::<TrackMetadata>(payload).ok()
}

pub fn is_track_title_message(title: &str) -> bool {
    title.starts_with(TITLE_PREFIX)
}

fn read_client_id() -> Option<String> {
    std::env::var(CLIENT_ID_ENV)
        .ok()
        .map(normalize_client_id)
        .filter(|id| !id.is_empty())
        .or_else(read_client_id_file)
        .or_else(read_bundled_client_id)
}

fn read_client_id_file() -> Option<String> {
    let path = client_id_file_path()?;
    let value = fs::read_to_string(path).ok()?;
    let id = normalize_client_id(value);
    (!id.is_empty()).then_some(id)
}

fn read_bundled_client_id() -> Option<String> {
    let id = normalize_client_id(BUNDLED_CLIENT_ID);
    (!id.is_empty()).then_some(id)
}

fn client_id_file_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(PathBuf::from).map(|dir| {
            dir.join("app.ytmusic.desktop")
                .join("discord-client-id.txt")
        })
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var_os("HOME").map(PathBuf::from).map(|dir| {
            dir.join(".config")
                .join("app.ytmusic.desktop")
                .join("discord-client-id.txt")
        })
    }
}

fn normalize_client_id(value: impl AsRef<str>) -> String {
    value
        .as_ref()
        .trim()
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect()
}

fn apply_activity_urls<'a>(activity: Activity<'a>, track: &'a TrackMetadata) -> Activity<'a> {
    let Some(url) = valid_track_url(track.url.as_deref()) else {
        return activity;
    };

    activity.details_url(url).state_url(url)
}

fn apply_activity_assets<'a>(activity: Activity<'a>, track: &'a TrackMetadata) -> Activity<'a> {
    let Some(cover_url) = valid_artwork_url(track.cover_url.as_deref()) else {
        return activity;
    };

    let mut assets = Assets::new()
        .large_image(cover_url)
        .large_text(track.asset_text());

    if let Some(track_url) = valid_track_url(track.url.as_deref()) {
        assets = assets.large_url(track_url);
    }

    activity.assets(assets)
}

fn apply_activity_timestamps<'a>(activity: Activity<'a>, track: &TrackMetadata) -> Activity<'a> {
    // Discord keeps counting from start/end timestamps, so the JS side only needs
    // to refresh progress occasionally instead of every second.
    if !track.playing {
        return activity;
    }

    let (Some(elapsed), Some(duration)) = (track.elapsed_seconds, track.duration_seconds) else {
        return activity;
    };

    if duration == 0 || elapsed >= duration {
        return activity;
    }

    let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return activity;
    };

    let now_ms = now.as_millis() as i64;
    let start = now_ms.saturating_sub((elapsed as i64) * 1000);
    let end = start.saturating_add((duration as i64) * 1000);

    activity.timestamps(Timestamps::new().start(start).end(end))
}

fn apply_activity_buttons<'a>(activity: Activity<'a>, track: &'a TrackMetadata) -> Activity<'a> {
    let Some(url) = valid_track_url(track.url.as_deref()) else {
        return activity;
    };

    activity.buttons(vec![Button::new("Listen on YouTube Music", url)])
}

fn clean_presence_part(value: Option<&str>) -> Option<&str> {
    let value = value?.trim();
    if value.is_empty() || is_metric_label(value) || is_generic_source_label(value) {
        None
    } else {
        Some(value)
    }
}

fn is_metric_label(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "view",
        "views",
        "play",
        "plays",
        "subscriber",
        "subscribers",
        "like",
        "likes",
    ]
    .iter()
    .any(|word| lower.split_whitespace().any(|part| part == *word))
}

fn is_generic_source_label(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "youtube music" | "music.youtube.com" | "youtube"
    )
}

impl TrackMetadata {
    fn asset_text(&self) -> String {
        self.album
            .as_deref()
            .and_then(|album| clean_presence_part(Some(album)))
            .unwrap_or("YouTube Music")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_probe_title() {
        let title = r#"YTMRPC:{"title":"Song","artist":"Artist","album":"Album","playing":true,"url":"https://music.youtube.com/watch?v=x","cover_url":"https://lh3.googleusercontent.com/image","elapsed_seconds":17,"duration_seconds":134}"#;
        let parsed = parse_track_title(title).expect("track metadata");

        assert_eq!(parsed.title, "Song");
        assert_eq!(parsed.artist.as_deref(), Some("Artist"));
        assert!(parsed.playing);
        assert_eq!(parsed.elapsed_seconds, Some(17));
        assert_eq!(parsed.duration_seconds, Some(134));
    }

    #[test]
    fn ignores_normal_titles() {
        assert!(parse_track_title("YouTube Music").is_none());
    }

    #[test]
    fn drops_view_count_from_presence() {
        let track = TrackMetadata {
            title: "Song".to_string(),
            artist: Some("Artist".to_string()),
            album: Some("1.7m views".to_string()),
            playing: true,
            url: None,
            cover_url: None,
            elapsed_seconds: None,
            duration_seconds: None,
        };

        assert_eq!(track.presence_state().as_deref(), Some("Artist"));
        assert_eq!(track.asset_text(), "YouTube Music");
    }

    #[test]
    fn presence_state_uses_artist_only() {
        let track = TrackMetadata {
            title: "Song".to_string(),
            artist: Some("Artist".to_string()),
            album: Some("Album".to_string()),
            playing: true,
            url: None,
            cover_url: None,
            elapsed_seconds: None,
            duration_seconds: None,
        };

        assert_eq!(track.presence_state().as_deref(), Some("Artist"));
    }

    #[test]
    fn omits_redundant_youtube_music_state() {
        let track = TrackMetadata {
            title: "Song".to_string(),
            artist: Some("YouTube Music".to_string()),
            album: None,
            playing: true,
            url: None,
            cover_url: None,
            elapsed_seconds: None,
            duration_seconds: None,
        };

        assert_eq!(track.presence_state(), None);
    }
}
