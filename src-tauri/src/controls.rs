use crate::{
    adblock::{self, AdBlockController},
    platform,
    presence::PresenceController,
    settings::{self, SharedSettings},
    updates,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{
    menu::{
        CheckMenuItem, CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem,
        SubmenuBuilder,
    },
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const PREVIOUS_ID: &str = "previous";
const PLAY_PAUSE_ID: &str = "play_pause";
const NEXT_ID: &str = "next";
const RELOAD_ID: &str = "reload";
const ZOOM_IN_ID: &str = "zoom_in";
const ZOOM_OUT_ID: &str = "zoom_out";
const ZOOM_RESET_ID: &str = "zoom_reset";
const DISCORD_RPC_ID: &str = "discord_rpc";
const TRAY_DISCORD_RPC_ID: &str = "tray_discord_rpc";
const DISCORD_STATUS_ID: &str = "discord_status";
const AD_BLOCK_ID: &str = "ad_block";
const AD_BLOCK_STATUS_ID: &str = "ad_block_status";
const CLOSE_TO_TRAY_ID: &str = "close_to_tray";
const STARTUP_ID: &str = "startup";
const START_MINIMIZED_ID: &str = "start_minimized";
const CLEAR_CACHE_ID: &str = "clear_cache";
const RESET_SESSION_ID: &str = "reset_session";
const CHECK_UPDATES_ID: &str = "check_updates";
const TRAY_SHOW_ID: &str = "tray_show";
const TRAY_QUIT_ID: &str = "tray_quit";
const LOCAL_SHORTCUTS: [(&str, &str); 5] = [
    ("Ctrl+R", RELOAD_ID),
    ("Ctrl+=", ZOOM_IN_ID),
    ("Ctrl+-", ZOOM_OUT_ID),
    ("Ctrl+0", ZOOM_RESET_ID),
    ("Ctrl+Shift+Delete", RESET_SESSION_ID),
];

#[derive(Clone)]
pub struct AppState {
    pub settings: SharedSettings,
    pub presence: PresenceController,
    pub adblock: AdBlockController,
    pub quitting: Arc<AtomicBool>,
}

#[derive(Clone)]
struct CheckItems {
    discord: CheckMenuItem<tauri::Wry>,
    tray_discord: CheckMenuItem<tauri::Wry>,
    ad_block: CheckMenuItem<tauri::Wry>,
    close_to_tray: CheckMenuItem<tauri::Wry>,
    startup: CheckMenuItem<tauri::Wry>,
    start_minimized: CheckMenuItem<tauri::Wry>,
}

pub fn install(app: &mut App, state: AppState) -> tauri::Result<()> {
    let initial = settings::snapshot(&state.settings);
    let checks = build_app_menu(app, &initial)?;
    build_tray(app, &checks, &initial)?;
    set_local_shortcuts(app.handle(), true, &state);
    for (shortcut, action) in [
        ("Ctrl+Alt+A", PREVIOUS_ID),
        ("Ctrl+Alt+S", PLAY_PAUSE_ID),
        ("Ctrl+Alt+D", NEXT_ID),
    ] {
        if let Err(error) =
            app.global_shortcut()
                .on_shortcut(shortcut, move |app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        media_action(app, action);
                    }
                })
        {
            platform::error(
                "Global Shortcut",
                &format!("{shortcut} could not be registered: {error}"),
            );
        }
    }

    let state_for_menu = state.clone();
    app.on_menu_event(move |app, event| {
        handle_menu_event(app, event.id().0.as_str(), &state_for_menu, &checks);
    });

    app.on_tray_icon_event(|tray, event| {
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            }
        ) {
            toggle_main_window(tray.app_handle());
        }
    });

    Ok(())
}

pub fn set_local_shortcuts(app: &AppHandle, focused: bool, state: &AppState) {
    for (shortcut, action) in LOCAL_SHORTCUTS {
        if focused {
            if app.global_shortcut().is_registered(shortcut) {
                continue;
            }
            let state = state.clone();
            if let Err(error) =
                app.global_shortcut()
                    .on_shortcut(shortcut, move |app, _shortcut, event| {
                        if event.state == ShortcutState::Pressed {
                            handle_local_shortcut(app, action, &state);
                        }
                    })
            {
                platform::error(
                    "Shortcut",
                    &format!("{shortcut} could not be registered: {error}"),
                );
            }
        } else if app.global_shortcut().is_registered(shortcut) {
            let _ = app.global_shortcut().unregister(shortcut);
        }
    }
}

fn build_app_menu(app: &App, initial: &settings::Settings) -> tauri::Result<CheckItems> {
    let previous = MenuItemBuilder::with_id(PREVIOUS_ID, "Previous")
        .accelerator("Ctrl+Alt+A")
        .build(app)?;
    let play_pause = MenuItemBuilder::with_id(PLAY_PAUSE_ID, "Play/Pause")
        .accelerator("Ctrl+Alt+S")
        .build(app)?;
    let next = MenuItemBuilder::with_id(NEXT_ID, "Next")
        .accelerator("Ctrl+Alt+D")
        .build(app)?;
    let playback = SubmenuBuilder::new(app, "Playback")
        .item(&previous)
        .item(&play_pause)
        .item(&next)
        .build()?;

    let reload = MenuItemBuilder::with_id(RELOAD_ID, "Reload")
        .accelerator("Ctrl+R")
        .build(app)?;
    let zoom_in = MenuItemBuilder::with_id(ZOOM_IN_ID, "Zoom In")
        .accelerator("Ctrl+=")
        .build(app)?;
    let zoom_out = MenuItemBuilder::with_id(ZOOM_OUT_ID, "Zoom Out")
        .accelerator("Ctrl+-")
        .build(app)?;
    let zoom_reset = MenuItemBuilder::with_id(ZOOM_RESET_ID, "Actual Size")
        .accelerator("Ctrl+0")
        .build(app)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let view = SubmenuBuilder::new(app, "View")
        .item(&reload)
        .item(&separator)
        .item(&zoom_in)
        .item(&zoom_out)
        .item(&zoom_reset)
        .build()?;

    let discord = CheckMenuItemBuilder::with_id(DISCORD_RPC_ID, "Discord RPC")
        .checked(initial.discord_rpc)
        .build(app)?;
    let discord_status =
        MenuItemBuilder::with_id(DISCORD_STATUS_ID, "Discord RPC Status").build(app)?;
    let ad_block = CheckMenuItemBuilder::with_id(AD_BLOCK_ID, "Ad Blocking")
        .checked(initial.ad_block)
        .build(app)?;
    let ad_block_status =
        MenuItemBuilder::with_id(AD_BLOCK_STATUS_ID, "Ad-block Status").build(app)?;
    let close_to_tray = CheckMenuItemBuilder::with_id(CLOSE_TO_TRAY_ID, "Close to Tray")
        .checked(initial.close_to_tray)
        .build(app)?;
    let startup = CheckMenuItemBuilder::with_id(STARTUP_ID, "Launch at Startup")
        .checked(initial.launch_at_startup)
        .build(app)?;
    let start_minimized = CheckMenuItemBuilder::with_id(START_MINIMIZED_ID, "Start Minimized")
        .checked(initial.start_minimized)
        .enabled(initial.launch_at_startup)
        .build(app)?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;
    let settings_menu = SubmenuBuilder::new(app, "Settings")
        .item(&discord)
        .item(&discord_status)
        .item(&separator_one)
        .item(&ad_block)
        .item(&ad_block_status)
        .item(&separator_two)
        .item(&close_to_tray)
        .item(&startup)
        .item(&start_minimized)
        .build()?;

    let clear_cache =
        MenuItemBuilder::with_id(CLEAR_CACHE_ID, "Clear Cache and Reload").build(app)?;
    let reset_session = MenuItemBuilder::with_id(RESET_SESSION_ID, "Reset Session")
        .accelerator("Ctrl+Shift+Delete")
        .build(app)?;
    let check_updates =
        MenuItemBuilder::with_id(CHECK_UPDATES_ID, "Check for Updates").build(app)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let tools = SubmenuBuilder::new(app, "Tools")
        .item(&clear_cache)
        .item(&reset_session)
        .item(&separator)
        .item(&check_updates)
        .build()?;

    let menu = MenuBuilder::new(app)
        .item(&playback)
        .item(&view)
        .item(&settings_menu)
        .item(&tools)
        .build()?;
    if let Some(window) = app.get_webview_window("main") {
        window.set_menu(menu)?;
    }

    let tray_discord = CheckMenuItemBuilder::with_id(TRAY_DISCORD_RPC_ID, "Discord RPC")
        .checked(initial.discord_rpc)
        .build(app)?;

    Ok(CheckItems {
        discord,
        tray_discord,
        ad_block,
        close_to_tray,
        startup,
        start_minimized,
    })
}

fn build_tray(app: &App, checks: &CheckItems, initial: &settings::Settings) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id(TRAY_SHOW_ID, "Show/Hide").build(app)?;
    let previous = MenuItemBuilder::with_id(PREVIOUS_ID, "Previous").build(app)?;
    let play_pause = MenuItemBuilder::with_id(PLAY_PAUSE_ID, "Play/Pause").build(app)?;
    let next = MenuItemBuilder::with_id(NEXT_ID, "Next").build(app)?;
    checks.tray_discord.set_checked(initial.discord_rpc)?;
    let quit = MenuItemBuilder::with_id(TRAY_QUIT_ID, "Quit").build(app)?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;
    let menu = MenuBuilder::new(app)
        .item(&show)
        .item(&separator_one)
        .item(&previous)
        .item(&play_pause)
        .item(&next)
        .item(&checks.tray_discord)
        .item(&separator_two)
        .item(&quit)
        .build()?;

    let mut tray = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("YouTube Music");
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    let _ = tray.build(app)?;

    Ok(())
}

fn handle_menu_event(app: &AppHandle, id: &str, state: &AppState, checks: &CheckItems) {
    match id {
        PREVIOUS_ID | PLAY_PAUSE_ID | NEXT_ID => media_action(app, id),
        RELOAD_ID => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.reload();
            }
        }
        ZOOM_IN_ID => set_zoom(app, state, 0.1),
        ZOOM_OUT_ID => set_zoom(app, state, -0.1),
        ZOOM_RESET_ID => reset_zoom(app, state),
        DISCORD_RPC_ID => {
            let enabled = checks.discord.is_checked().unwrap_or(true);
            set_discord_rpc(state, checks, enabled);
        }
        TRAY_DISCORD_RPC_ID => {
            let enabled = checks.tray_discord.is_checked().unwrap_or(true);
            set_discord_rpc(state, checks, enabled);
        }
        DISCORD_STATUS_ID => platform::info("Discord RPC", &state.presence.status()),
        AD_BLOCK_ID => {
            let enabled = checks.ad_block.is_checked().unwrap_or(true);
            state.adblock.set_enabled(enabled);
            settings::update(&state.settings, |value| value.ad_block = enabled);
            eval_main(
                app,
                &format!("window.__ytMusicTauriAdBlockEnabled = {enabled}; location.reload();"),
            );
        }
        AD_BLOCK_STATUS_ID => platform::info(
            "Ad Blocking",
            &format!(
                "{}\nBlocked requests this session: {}",
                if settings::snapshot(&state.settings).ad_block {
                    "Enabled"
                } else {
                    "Disabled"
                },
                state.adblock.blocked_requests()
            ),
        ),
        CLOSE_TO_TRAY_ID => {
            let enabled = checks.close_to_tray.is_checked().unwrap_or(false);
            settings::update(&state.settings, |value| value.close_to_tray = enabled);
        }
        STARTUP_ID => set_startup(state, checks),
        START_MINIMIZED_ID => set_start_minimized(state, checks),
        CLEAR_CACHE_ID => clear_cache(app),
        RESET_SESSION_ID => reset_session(app, state),
        CHECK_UPDATES_ID => updates::check_in_background(false),
        TRAY_SHOW_ID => toggle_main_window(app),
        TRAY_QUIT_ID => {
            state.quitting.store(true, Ordering::Relaxed);
            state.presence.clear();
            app.exit(0);
        }
        _ => {}
    }
}

fn handle_local_shortcut(app: &AppHandle, action: &str, state: &AppState) {
    match action {
        RELOAD_ID => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.reload();
            }
        }
        ZOOM_IN_ID => set_zoom(app, state, 0.1),
        ZOOM_OUT_ID => set_zoom(app, state, -0.1),
        ZOOM_RESET_ID => reset_zoom(app, state),
        RESET_SESSION_ID => reset_session(app, state),
        _ => {}
    }
}

fn set_discord_rpc(state: &AppState, checks: &CheckItems, enabled: bool) {
    let _ = checks.discord.set_checked(enabled);
    let _ = checks.tray_discord.set_checked(enabled);
    settings::update(&state.settings, |value| value.discord_rpc = enabled);
    state.presence.set_enabled(enabled);
}

fn set_startup(state: &AppState, checks: &CheckItems) {
    let enabled = checks.startup.is_checked().unwrap_or(false);
    let minimized = checks.start_minimized.is_checked().unwrap_or(false);

    match platform::set_startup_enabled(enabled, minimized) {
        Ok(()) => {
            let _ = checks.start_minimized.set_enabled(enabled);
            settings::update(&state.settings, |value| {
                value.launch_at_startup = enabled;
                value.start_minimized = minimized;
            });
        }
        Err(error) => {
            let _ = checks.startup.set_checked(!enabled);
            platform::error("Launch at Startup", &error.to_string());
        }
    }
}

fn set_start_minimized(state: &AppState, checks: &CheckItems) {
    let minimized = checks.start_minimized.is_checked().unwrap_or(false);
    let enabled = checks.startup.is_checked().unwrap_or(false);

    if let Err(error) = platform::set_startup_enabled(enabled, minimized) {
        let _ = checks.start_minimized.set_checked(!minimized);
        platform::error("Launch at Startup", &error.to_string());
        return;
    }

    settings::update(&state.settings, |value| value.start_minimized = minimized);
}

fn set_zoom(app: &AppHandle, state: &AppState, delta: f64) {
    let current = settings::snapshot(&state.settings).zoom;
    apply_zoom(app, state, (current + delta).clamp(0.5, 2.0));
}

fn reset_zoom(app: &AppHandle, state: &AppState) {
    apply_zoom(app, state, 1.0);
}

fn apply_zoom(app: &AppHandle, state: &AppState, zoom: f64) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_zoom(zoom);
        settings::update(&state.settings, |value| value.zoom = zoom);
    }
}

fn clear_cache(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let reload = window.clone();
        let _ = window.with_webview(move |webview| {
            adblock::clear_cache(webview, move || {
                let _ = reload.reload();
            });
        });
    }
}

fn reset_session(app: &AppHandle, state: &AppState) {
    if !platform::confirm(
        "Reset YouTube Music Session",
        "This signs out of YouTube Music and clears all site data. Continue?",
    ) {
        return;
    }

    state.presence.clear();
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.clear_all_browsing_data();
        let _ = window.navigate(
            "https://music.youtube.com"
                .parse()
                .expect("static YouTube Music URL"),
        );
    }
}

fn eval_main(app: &AppHandle, script: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval(script);
    }
}

fn media_action(app: &AppHandle, action: &str) {
    let script = match action {
        PREVIOUS_ID => {
            "(() => { const button = document.querySelector('ytmusic-player-bar #previous-button, ytmusic-player-bar #previous-song-button, ytmusic-player-bar .previous-button, ytmusic-player-bar [aria-label^=\"Previous\"]'); if (button) button.click(); else { const media = document.querySelector('video, audio'); if (media) media.currentTime = 0; } })();"
        }
        PLAY_PAUSE_ID => {
            "(() => { const media = document.querySelector('video, audio'); if (media) media.paused ? media.play() : media.pause(); else document.querySelector('ytmusic-player-bar #play-pause-button, ytmusic-player-bar .play-pause-button')?.click(); })();"
        }
        NEXT_ID => {
            "(() => { const button = document.querySelector('ytmusic-player-bar #next-button, ytmusic-player-bar #next-song-button, ytmusic-player-bar .next-button, ytmusic-player-bar [aria-label^=\"Next\"]'); if (button) button.click(); else { const media = document.querySelector('video, audio'); if (media && Number.isFinite(media.duration)) media.currentTime = media.duration; } })();"
        }
        _ => return,
    };
    eval_main(app, script);
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn toggle_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            show_main_window(app);
        }
    }
}
