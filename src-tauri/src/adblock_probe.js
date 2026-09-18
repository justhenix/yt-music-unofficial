(() => {
  if (location.hostname !== "music.youtube.com") {
    return;
  }
  if (window.__ytMusicTauriAdBlockInstalled) {
    return;
  }
  window.__ytMusicTauriAdBlockInstalled = true;

  const savedMediaState = new WeakMap();
  let hasSavedMedia = false;
  const style = document.createElement("style");
  style.textContent = `
    ytd-ad-slot-renderer,
    ytd-display-ad-renderer,
    ytd-in-feed-ad-layout-renderer,
    ytd-promoted-sparkles-web-renderer,
    .html5-video-player.ad-showing .ytp-ad-module,
    .html5-video-player.ad-interrupting .ytp-ad-module {
      display: none !important;
    }
    ytmusic-mealbar-promo-renderer {
      opacity: 0 !important;
      pointer-events: none !important;
    }
  `;

  const injectStyle = () => {
    if (window.__ytMusicTauriAdBlockEnabled === false) {
      return;
    }
    const target = document.head || document.documentElement;
    if (target) {
      target.appendChild(style);
    } else {
      window.setTimeout(injectStyle, 50);
    }
  };
  injectStyle();

  const isMusicHost = () => location.hostname === "music.youtube.com";

  const isAdShowing = () => {
    const player = document.querySelector(".html5-video-player");
    return Boolean(
      player &&
        player.classList &&
        (player.classList.contains("ad-showing") ||
          player.classList.contains("ad-interrupting")),
    );
  };

  let popupObserver = null;
  const dismissMealbar = () => {
    const mealbar = document.querySelector("ytmusic-mealbar-promo-renderer");
    if (!mealbar) {
      return;
    }
    if (typeof mealbar.dismiss === "function") {
      mealbar.dismiss();
      return;
    }
    const dismissBtn = mealbar.querySelector(
      "#dismiss-button, [aria-label*='Dismiss' i], yt-button-renderer:last-child button"
    );
    if (dismissBtn) {
      dismissBtn.click();
    }
  };

  const attachPopupObserver = () => {
    if (popupObserver) {
      return;
    }
    const container = document.querySelector("ytmusic-popup-container");
    if (!container) {
      return;
    }
    popupObserver = new MutationObserver(dismissMealbar);
    popupObserver.observe(container, { childList: true, subtree: true });
  };

  const clearOrphanedInert = () => {
    if (typeof document?.querySelector !== "function") {
      return;
    }
    const dialog = document.querySelector(
      "tp-yt-paper-dialog:not([style*='display: none']), ytmusic-dialog:not([style*='display: none'])"
    );
    if (dialog && dialog.offsetParent !== null) {
      return;
    }
    const layout = document.querySelector("ytmusic-app-layout[inert]");
    if (layout) {
      layout.removeAttribute("inert");
    }
    if (typeof document.querySelectorAll === "function") {
      for (const backdrop of document.querySelectorAll(
        "tp-yt-iron-overlay-backdrop.opened, tp-yt-iron-overlay-backdrop[opened]"
      )) {
        backdrop.classList?.toggle?.("opened", false);
        backdrop.removeAttribute?.("opened");
        if (backdrop.style) {
          backdrop.style.pointerEvents = "none";
        }
      }
    }
  };

  const restoreMedia = () => {
    if (!hasSavedMedia) {
      return;
    }
    for (const media of document.querySelectorAll("video, audio")) {
      const saved = savedMediaState.get(media);
      if (!saved) {
        continue;
      }
      media.muted = saved.muted;
      media.playbackRate = saved.playbackRate;
      savedMediaState.delete(media);
    }
    hasSavedMedia = false;
  };

  const skipAd = () => {
    const skipButton = document.querySelector(
      ".ytp-ad-skip-button-modern, .ytp-ad-skip-button, .ytp-ad-skip-button-container button, .ytp-ad-skip-button-slot button",
    );
    if (skipButton && !skipButton.disabled) {
      skipButton.click();
    }

    for (const media of document.querySelectorAll("video, audio")) {
      if (!savedMediaState.has(media)) {
        savedMediaState.set(media, {
          muted: media.muted,
          playbackRate: media.playbackRate,
        });
        hasSavedMedia = true;
      }

      media.muted = true;
      media.playbackRate = 16;
      if (
        Number.isFinite(media.duration) &&
        media.duration > 0 &&
        media.currentTime < media.duration - 0.25
      ) {
        try {
          media.currentTime = Math.max(media.currentTime, media.duration - 0.1);
        } catch {
          // Some ad streams reject seeking; accelerated muted playback remains.
        }
      }
    }
  };

  const run = () => {
    attachPopupObserver();
    dismissMealbar();
    clearOrphanedInert();

    if (
      window.__ytMusicTauriAdBlockEnabled === false ||
      !isMusicHost() ||
      !isAdShowing()
    ) {
      restoreMedia();
      return;
    }

    skipAd();
  };

  const install = () => {
    if (!document.documentElement) {
      window.setTimeout(install, 50);
      return;
    }

    const observer = new MutationObserver(run);
    observer.observe(document.documentElement, {
      childList: true,
      subtree: true,
    });

    document.addEventListener("yt-navigate-finish", run, true);
    window.addEventListener("pageshow", run, true);
    window.addEventListener("pointerdown", clearOrphanedInert, true);
    window.setInterval(run, 250);
    window.setInterval(clearOrphanedInert, 2000);
    run();
  };

  install();
})();
