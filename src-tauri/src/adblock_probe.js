(() => {
  if (window.__ytMusicTauriAdBlockInstalled) {
    return;
  }
  window.__ytMusicTauriAdBlockInstalled = true;

  const style = document.createElement("style");
  style.textContent = `
    ytd-ad-slot-renderer,
    ytd-display-ad-renderer,
    ytd-in-feed-ad-layout-renderer,
    ytd-promoted-sparkles-web-renderer,
    ytmusic-mealbar-promo-renderer {
      display: none !important;
    }
  `;

  const injectStyle = () => {
    const target = document.head || document.documentElement;
    if (target) {
      target.appendChild(style);
    } else {
      window.setTimeout(injectStyle, 50);
    }
  };
  injectStyle();

  window.setInterval(() => {
    if (window.__ytMusicTauriAdBlockEnabled === false) {
      return;
    }
    const skipBtn = document.querySelector(
      ".ytp-ad-skip-button-modern, .ytp-ad-skip-button, .ytp-ad-skip-button-slot button"
    );
    if (skipBtn && !skipBtn.disabled) {
      skipBtn.click();
    }
  }, 1000);
})();
