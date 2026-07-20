#[cfg(windows)]
use webview2_com::{
    ClearBrowsingDataCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Profile2, ICoreWebView2_13,
        COREWEBVIEW2_BROWSING_DATA_KINDS_CACHE_STORAGE,
        COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE,
    },
};

#[cfg(windows)]
use windows::core::Interface;

use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

#[derive(Clone)]
pub struct AdBlockController {
    enabled: Arc<AtomicBool>,
    blocked_requests: Arc<AtomicU64>,
}

impl AdBlockController {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(enabled)),
            blocked_requests: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn blocked_requests(&self) -> u64 {
        self.blocked_requests.load(Ordering::Relaxed)
    }

    pub fn install(&self, _webview: tauri::webview::PlatformWebview) {
        // Do not register WebResourceRequestedFilter on * because intercepting
        // streaming media requests breaks WebView2 chunked media playback.
    }
}

#[cfg(windows)]
pub fn clear_cache(
    webview: tauri::webview::PlatformWebview,
    on_complete: impl FnOnce() + Send + 'static,
) {
    unsafe {
        let Ok(core_webview) = webview.controller().CoreWebView2() else {
            on_complete();
            return;
        };
        let Ok(profile) = core_webview
            .cast::<ICoreWebView2_13>()
            .and_then(|webview| webview.Profile())
            .and_then(|profile| profile.cast::<ICoreWebView2Profile2>())
        else {
            on_complete();
            return;
        };
        let kinds = COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE
            | COREWEBVIEW2_BROWSING_DATA_KINDS_CACHE_STORAGE;
        let mut on_complete = Some(on_complete);
        let _ = profile.ClearBrowsingData(
            kinds,
            &ClearBrowsingDataCompletedHandler::create(Box::new(move |_| {
                if let Some(on_complete) = on_complete.take() {
                    on_complete();
                }
                Ok(())
            })),
        );
    }
}

#[cfg(not(windows))]
pub fn clear_cache(
    _webview: tauri::webview::PlatformWebview,
    on_complete: impl FnOnce() + Send + 'static,
) {
    on_complete();
}

pub fn should_block_url(_url: &str) -> bool {
    // Network-level blocking triggers YouTube anti-adblock locks on media playback.
    // UI ad-skipping and media acceleration are handled in adblock_probe.js.
    false
}

#[cfg(test)]
mod tests {
    use super::should_block_url;

    #[test]
    fn network_level_blocking_disabled_to_prevent_playback_anti_adblock_locks() {
        assert!(!should_block_url(
            "https://googleads.g.doubleclick.net/pagead/id"
        ));
        assert!(!should_block_url("https://music.youtube.com/"));
        assert!(!should_block_url(
            "https://rr1---sn.googlevideo.com/videoplayback?expire=1&mime=audio/webm"
        ));
    }
}
