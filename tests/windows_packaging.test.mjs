import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const packageJson = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);
const tauriConfig = JSON.parse(
  readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
);

test("default release build goes through signed Windows build guard", () => {
  assert.match(packageJson.scripts.build, /build-windows\.ps1/);
  assert.equal(packageJson.scripts["build:unsigned"], "tauri build");
});

test("first install embeds the WebView2 bootstrapper", () => {
  assert.equal(
    tauriConfig.bundle.windows.webviewInstallMode.type,
    "embedBootstrapper",
  );
});

test("MSI license source remains plain text for Tauri's RTF conversion", () => {
  assert.doesNotMatch(tauriConfig.bundle.licenseFile, /\.rtf$/i);

  const licenseSource = readFileSync(
    new URL(`../src-tauri/${tauriConfig.bundle.licenseFile}`, import.meta.url),
    "utf8",
  );
  assert.doesNotMatch(licenseSource, /^\{\\rtf1\\ansi/);
  assert.match(licenseSource, /^MIT License/);
});
