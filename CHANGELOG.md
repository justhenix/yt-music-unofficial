# Changelog

## Unreleased

### Fixes

- Restored ad blocking with host-scoped WebView2 filters that do not intercept media streams.
- Preserved Share, inactivity, and other YouTube Music dialogs while skipping detected player ads.
- Reapplied ad blocking after sign-out navigation without changing Google account pages.
- Embedded the WebView2 bootstrapper for more reliable first-time Windows installs.
- Added MSI license/bootstrapper verification and blocked accidental unsigned release builds.

## 0.1.6 - 2026-07-20

### Fixes

- Fixed Google sign-in navigation being opened in an external browser instead of WebView.
- Fixed HTML5 audio/video media playback stalling at 0:00 when ad-block is enabled.
- Fixed WebView2 media autoplay policy requiring manual user gesture.

## 0.1.5 - 2026-07-12

### QoL

- Added persistent Discord RPC, ad-block, close-to-tray, startup, start-minimized, and zoom settings.
- Added a system tray with show/hide, previous, play/pause, next, Discord RPC, and quit controls.
- Added reload, zoom, cache clear, session reset, RPC status, and ad-block status actions.
- Added automatic and manual GitHub release checks.
- Open external HTTPS links in the default browser.
- Added left-hand global playback shortcuts: `Ctrl+Alt+A` previous, `Ctrl+Alt+S` play/pause, and `Ctrl+Alt+D` next.
- Avoided GitHub API rate limits in update checks.

## 0.1.4 - 2026-07-04

### Fixes

- Clear Discord Rich Presence when YouTube Music has no valid track, shows an ad, leaves the music host, or the app closes.

### Documentation

- Refined README positioning for the desktop app.
