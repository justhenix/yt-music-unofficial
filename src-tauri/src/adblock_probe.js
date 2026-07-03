(() => {
  if (window.__ytMusicTauriAdBlockInstalled) {
    return;
  }

  window.__ytMusicTauriAdBlockInstalled = true;

  const AD_SELECTORS = [
    // UI cleanup only. Network-level blocking lives in adblock.rs.
    // Do not add the skip-button container here; click it first in SKIP_SELECTORS.
    "ytd-ad-slot-renderer",
    "ytd-display-ad-renderer",
    "ytd-in-feed-ad-layout-renderer",
    "ytd-promoted-sparkles-web-renderer",
    "ytmusic-mealbar-promo-renderer",
    "ytmusic-popup-container tp-yt-paper-dialog",
    ".ytp-ad-module",
    ".ytp-ad-overlay-container",
    ".ytp-ad-player-overlay",
    ".ytp-ad-text-overlay",
    ".ytp-ad-survey",
  ];

  const SKIP_SELECTORS = [
    // Add localized skip labels here when YouTube serves a new locale.
    ".ytp-ad-skip-button-modern",
    ".ytp-ad-skip-button",
    ".ytp-ad-skip-button-container button",
    "button[aria-label*='Skip']",
    "button[aria-label*='skip']",
    "button[aria-label*='Lewati']",
    "button[aria-label*='lewati']",
  ];

  const closeDialogs = () => {
    const buttons = document.querySelectorAll(
      "ytmusic-mealbar-promo-renderer button, ytmusic-popup-container button, tp-yt-paper-dialog button"
    );

    for (const button of buttons) {
      const label = `${button.textContent || ""} ${button.getAttribute("aria-label") || ""}`.toLowerCase();
      if (
        label.includes("no thanks") ||
        label.includes("not now") ||
        label.includes("dismiss") ||
        label.includes("skip") ||
        label.includes("nanti") ||
        label.includes("lewati")
      ) {
        button.click();
      }
    }
  };

  const removeAdNodes = () => {
    for (const selector of AD_SELECTORS) {
      for (const node of document.querySelectorAll(selector)) {
        if (node.closest && node.closest("ytmusic-player-bar")) {
          continue;
        }
        node.remove();
      }
    }
  };

  const skipVisibleAds = () => {
    for (const selector of SKIP_SELECTORS) {
      for (const button of document.querySelectorAll(selector)) {
        if (!button.disabled) {
          button.click();
        }
      }
    }
  };

  const accelerateAdMedia = () => {
    // Fallback for ads that arrive through first-party media paths and cannot be
    // cleanly blocked without risking normal song playback.
    const adShowing = Boolean(
      document.querySelector(
        ".ad-showing, .ytp-ad-player-overlay, .ytp-ad-preview-container, .ytp-ad-skip-button-container"
      )
    );

    if (!adShowing) {
      for (const media of document.querySelectorAll("video, audio")) {
        if (media.playbackRate > 2) {
          media.playbackRate = 1;
        }
      }
      return;
    }

    for (const media of document.querySelectorAll("video, audio")) {
      media.muted = true;
      media.playbackRate = 16;

      if (Number.isFinite(media.duration) && media.duration > 0) {
        media.currentTime = Math.max(media.currentTime, media.duration - 0.2);
      }
    }
  };

  const run = () => {
    if (location.hostname !== "music.youtube.com") {
      return;
    }

    // Order matters: skip before removing ad UI, otherwise the useful button can disappear.
    skipVisibleAds();
    closeDialogs();
    removeAdNodes();
    accelerateAdMedia();
  };

  const observer = new MutationObserver(run);

  const install = () => {
    if (!document.documentElement) {
      window.setTimeout(install, 50);
      return;
    }

    observer.observe(document.documentElement, {
      attributes: true,
      childList: true,
      subtree: true,
    });

    window.setInterval(run, 500);
    run();
  };

  install();
})();
