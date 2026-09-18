import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const probeScript = readFileSync(
  new URL("../src-tauri/src/adblock_probe.js", import.meta.url),
  "utf8",
);

function createHarness({
  adShowing = false,
  enabled = true,
  hostname = "music.youtube.com",
  layoutInert = false,
  backdropOpened = false,
} = {}) {
  const intervals = [];
  const styles = [];
  const playerClasses = new Set(adShowing ? ["ad-showing"] : []);
  let skipClicks = 0;
  let inertRemoved = false;
  let backdropOpenedCleared = false;

  const media = {
    currentTime: 2,
    duration: 30,
    muted: false,
    playbackRate: 1,
  };
  const player = {
    classList: {
      contains: (name) => playerClasses.has(name),
    },
  };
  const skipButton = {
    disabled: false,
    click() {
      skipClicks += 1;
    },
  };
  const layout = {
    removeAttribute(name) {
      if (name === "inert") {
        inertRemoved = true;
      }
    },
  };
  const backdrop = {
    style: { pointerEvents: backdropOpened ? "auto" : "none" },
    classList: {
      toggle(cls, val) {
        if (cls === "opened" && !val) {
          backdropOpenedCleared = true;
        }
      },
    },
    removeAttribute(name) {
      if (name === "opened") {
        backdropOpenedCleared = true;
      }
    },
  };
  const location = { hostname };
  const document = {
    head: {
      appendChild(node) {
        styles.push(node);
      },
    },
    documentElement: {},
    createElement() {
      return { textContent: "" };
    },
    addEventListener() {},
    querySelector(selector) {
      if (selector === ".html5-video-player") {
        return player;
      }
      if (selector === "ytmusic-app-layout[inert]") {
        return layoutInert ? layout : null;
      }
      if (selector.includes("ytp-ad-skip")) {
        return playerClasses.has("ad-showing") || playerClasses.has("ad-interrupting")
          ? skipButton
          : null;
      }
      return null;
    },
    querySelectorAll(selector) {
      if (selector === "video, audio") {
        return [media];
      }
      if (selector.includes("tp-yt-iron-overlay-backdrop")) {
        return backdropOpened ? [backdrop] : [];
      }
      return [];
    },
  };
  class MutationObserver {
    constructor(callback) {
      this.callback = callback;
    }

    observe() {}
  }
  const window = {
    __ytMusicTauriAdBlockEnabled: enabled,
    addEventListener() {},
    clearTimeout() {},
    setInterval(callback) {
      intervals.push(callback);
      return intervals.length;
    },
    setTimeout(callback) {
      callback();
      return 1;
    },
  };
  window.window = window;

  vm.runInNewContext(probeScript, {
    document,
    location,
    MutationObserver,
    window,
  });

  return {
    backdrop,
    isInertRemoved: () => inertRemoved,
    isBackdropCleared: () => backdropOpenedCleared,
    location,
    media,
    playerClasses,
    run() {
      for (const callback of intervals) {
        callback();
      }
    },
    skipClicks: () => skipClicks,
    styles,
  };
}

test("leaves normal playback state untouched", () => {
  const harness = createHarness();

  harness.run();

  assert.equal(harness.skipClicks(), 0);
  assert.equal(harness.media.currentTime, 2);
  assert.equal(harness.media.muted, false);
  assert.equal(harness.media.playbackRate, 1);
});

test("skips an explicitly detected player ad and restores media state", () => {
  const harness = createHarness({ adShowing: true });

  harness.run();

  assert.ok(harness.skipClicks() > 0);
  assert.equal(harness.media.muted, true);
  assert.ok(harness.media.currentTime >= 29);

  harness.playerClasses.delete("ad-showing");
  harness.run();

  assert.equal(harness.media.muted, false);
  assert.equal(harness.media.playbackRate, 1);
});

test("does not act on Google account pages during logout", () => {
  const harness = createHarness({
    adShowing: true,
    hostname: "accounts.google.com",
  });

  harness.run();

  assert.equal(harness.skipClicks(), 0);
  assert.equal(harness.media.currentTime, 2);
  assert.equal(harness.media.muted, false);
});

test("never hides or removes generic YouTube Music dialogs", () => {
  const harness = createHarness();
  const injectedCss = harness.styles.map((style) => style.textContent).join("\n");

  assert.doesNotMatch(injectedCss, /ytmusic-popup-container|tp-yt-paper-dialog/);
  assert.doesNotMatch(probeScript, /\.remove\s*\(/);
});

test("removes inert attribute from ytmusic-app-layout when promo leaves it behind", () => {
  const harness = createHarness({ layoutInert: true });

  harness.run();

  assert.equal(harness.isInertRemoved(), true);
});

test("clears orphaned iron-overlay-backdrops even when layout has no inert attribute", () => {
  const harness = createHarness({ layoutInert: false, backdropOpened: true });

  harness.run();

  assert.equal(harness.isBackdropCleared(), true);
  assert.equal(harness.backdrop.style.pointerEvents, "none");
});
