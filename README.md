# YouTube Music Tauri

Low-memory YouTube Music desktop wrapper built with Tauri v2.

## Features

- Loads `https://music.youtube.com` directly in a native Tauri WebView2 window on Windows.
- Persists YouTube login cookies in the app data profile.
- Blocks common YouTube/Google ad and tracking requests through a native WebView2 request filter.
- Injects a YouTube Music ad-control script to skip, mute, and remove ad UI served through first-party player paths.
- Updates Discord Rich Presence from the current YouTube Music track without exposing Tauri IPC to the remote page.
- Keeps a single app instance and restores window size/position.

## Requirements

- Node.js and npm.
- Rust toolchain with Cargo for native Tauri builds.
- Windows WebView2 runtime.
- Discord desktop client running for Rich Presence.

## Setup

```powershell
npm install
```

## Ad Block Verification

The app has a hidden self-test mode for the native blocker:

```powershell
$env:YT_MUSIC_ADBLOCK_SELF_TEST = "1"
Start-Process "$env:LOCALAPPDATA\YouTube Music\yt-music-tauri.exe"
```

When the blocker is wired correctly, the window title briefly becomes `ADBLOCK_SELF_TEST:PASS`.

Discord Rich Presence is configured from the bundled `src-tauri/discord-client-id.txt`.
After the Discord Developer application is created once, the app ID is stored there and
included in future builds. No launch-time environment variable is required.

Advanced override: set `YT_MUSIC_DISCORD_CLIENT_ID` if you intentionally want to test
against a different Discord application without rebuilding.

## Build

```powershell
npm run build
```

If `cargo` is not recognized, install Rust from https://www.rust-lang.org/tools/install, restart PowerShell, then rerun the build.

## Legal Notes

- This project is unofficial and is not affiliated with YouTube, Google, Discord, Microsoft, or Tauri.
- This repository is licensed under the MIT License.
- Third-party dependency acknowledgements are listed in `THIRD_PARTY_NOTICES.md`.

## Contributor Notes

- Track and pause bugs usually belong in `src-tauri/src/track_probe.js`.
- Ad URL rules belong in `src-tauri/src/adblock.rs`; add or update unit tests for each new rule.
- Ad UI skip/cleanup rules belong in `src-tauri/src/adblock_probe.js`; keep skip-button selectors out of the removal list.
- Discord Rich Presence formatting belongs in `src-tauri/src/presence.rs`.
