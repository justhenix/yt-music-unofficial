#[cfg(windows)]
use webview2_com::{
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Environment, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
    },
    WebResourceRequestedEventHandler,
};

#[cfg(windows)]
use windows::core::{HSTRING, PWSTR};

#[cfg(windows)]
const FILTER: &str = "*";

#[cfg(windows)]
pub fn install(webview: tauri::webview::PlatformWebview) {
    unsafe {
        let controller = webview.controller();
        let Ok(core_webview) = controller.CoreWebView2() else {
            return;
        };
        let environment = webview.environment();

        // WebView2 filters are cheap here because should_block_url() keeps the
        // matching rules small and conservative. Add tests for every new pattern.
        let filter = HSTRING::from(FILTER);
        let _ = core_webview
            .AddWebResourceRequestedFilter(&filter, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL);

        let handler = WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else {
                return Ok(());
            };

            let request = args.Request()?;
            let mut uri = PWSTR::null();
            request.Uri(&mut uri)?;
            let url = take_pwstr(uri);

            if should_block_url(&url) {
                let response = blocked_response(&environment)?;
                args.SetResponse(&response)?;
            }

            Ok(())
        }));

        let mut token = 0;
        let _ = core_webview.add_WebResourceRequested(&handler, &mut token);
    }
}

#[cfg(not(windows))]
pub fn install(_webview: tauri::webview::PlatformWebview) {}

#[cfg(windows)]
unsafe fn blocked_response(
    environment: &ICoreWebView2Environment,
) -> windows::core::Result<
    webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2WebResourceResponse,
> {
    let status = HSTRING::from("No Content");
    let headers = HSTRING::from("Access-Control-Allow-Origin: *\r\nCache-Control: no-store\r\n");
    environment.CreateWebResourceResponse(None, 204, &status, &headers)
}

#[cfg(windows)]
fn take_pwstr(value: PWSTR) -> String {
    if value.is_null() {
        String::new()
    } else {
        unsafe { value.to_string().unwrap_or_default() }
    }
}

pub fn should_block_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();

    if lower.starts_with("data:")
        || lower.starts_with("blob:")
        || lower.starts_with("about:")
        || lower.starts_with("chrome-extension:")
        || lower.starts_with("edge-extension:")
    {
        return false;
    }

    let Some(host) = host_from_url(&lower) else {
        return false;
    };

    if is_blocked_ad_host(host) {
        return true;
    }

    if host.ends_with("youtube.com")
        || host.ends_with("youtube-nocookie.com")
        || host.ends_with("googlevideo.com")
        || host.ends_with("ytimg.com")
    {
        return is_blocked_youtube_path(&lower);
    }

    false
}

fn host_from_url(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("://")?;
    let host = rest.split(['/', '?', '#']).next()?.split('@').last()?;
    let host = host.split(':').next().unwrap_or(host);
    (!host.is_empty()).then_some(host)
}

fn is_blocked_ad_host(host: &str) -> bool {
    // Host rules should stay limited to known ad/tracking infrastructure. Blocking
    // broad Google or YouTube hosts will break playback and login.
    const HOSTS: &[&str] = &[
        "2mdn.net",
        "ad.doubleclick.net",
        "adservice.google.com",
        "ads.youtube.com",
        "googleads.g.doubleclick.net",
        "googleadservices.com",
        "googlesyndication.com",
        "pagead2.googlesyndication.com",
        "pagead-googlehosted.l.google.com",
        "partner.googleadservices.com",
        "pubads.g.doubleclick.net",
        "securepubads.g.doubleclick.net",
        "static.doubleclick.net",
        "tpc.googlesyndication.com",
        "www.googleadservices.com",
    ];

    HOSTS
        .iter()
        .any(|blocked| host == *blocked || host.ends_with(&format!(".{blocked}")))
}

fn is_blocked_youtube_path(url: &str) -> bool {
    // Path rules catch first-party YouTube ad endpoints. Prefer specific markers
    // from observed requests and cover each new marker with a unit test.
    const PATH_PARTS: &[&str] = &[
        "/api/stats/ads",
        "/api/stats/qoe?ad",
        "/get_midroll_info",
        "/get_video_ads",
        "/pagead/",
        "/ptracking?",
        "/youtubei/v1/log_event",
        "adformat=",
        "adunit=",
        "afv_ad_tag",
        "ctier=a",
        "oad=",
        "player_ias",
        "videoplayback?oad",
    ];

    PATH_PARTS.iter().any(|part| url.contains(part))
}

#[cfg(test)]
mod tests {
    use super::should_block_url;

    #[test]
    fn blocks_google_ad_hosts() {
        assert!(should_block_url(
            "https://googleads.g.doubleclick.net/pagead/id"
        ));
        assert!(should_block_url(
            "https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js"
        ));
    }

    #[test]
    fn blocks_youtube_ad_endpoints() {
        assert!(should_block_url(
            "https://music.youtube.com/api/stats/ads?x=1"
        ));
        assert!(should_block_url(
            "https://rr1---sn.googlevideo.com/videoplayback?oad=1&mime=video/mp4"
        ));
    }

    #[test]
    fn allows_youtube_music_and_normal_media() {
        assert!(!should_block_url("https://music.youtube.com/"));
        assert!(!should_block_url(
            "https://rr1---sn.googlevideo.com/videoplayback?expire=1&mime=audio/webm"
        ));
        assert!(!should_block_url("https://i.ytimg.com/vi/abc/default.jpg"));
    }
}
