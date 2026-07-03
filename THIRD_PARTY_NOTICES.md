# Third-Party Notices

This project uses open-source dependencies and is licensed under the MIT License.

This project is unofficial and is not affiliated with, endorsed by, or sponsored
by YouTube, Google, Discord, Microsoft, or Tauri.

## Direct Runtime Dependencies

| Component | License | Purpose |
| --- | --- | --- |
| Tauri | Apache-2.0 OR MIT | Desktop app framework |
| Tauri single-instance plugin | Apache-2.0 OR MIT | Reuses the existing app window |
| Tauri window-state plugin | Apache-2.0 OR MIT | Persists window size and position |
| discord-rich-presence | MIT | Discord Rich Presence IPC |
| serde | MIT OR Apache-2.0 | Data serialization |
| serde_json | MIT OR Apache-2.0 | JSON parsing |
| webview2-com | MIT | Windows WebView2 COM bindings |
| windows | MIT OR Apache-2.0 | Windows API bindings |

## Build Tooling

| Component | License | Purpose |
| --- | --- | --- |
| @tauri-apps/cli | Apache-2.0 OR MIT | Tauri build and bundle CLI |
| tauri-build | Apache-2.0 OR MIT | Tauri build script support |

## Platform Components

| Component | Provider | Notes |
| --- | --- | --- |
| Microsoft Edge WebView2 Runtime | Microsoft | Required runtime used by Tauri on Windows |
| Discord desktop client | Discord | Required only for Rich Presence display |
| YouTube Music web service | Google/YouTube | Loaded as a remote web page; no affiliation claimed |

## Transitive Dependencies

The Windows release build also includes transitive Rust dependencies recorded in
`src-tauri/Cargo.lock`. The Windows-target dependency graph can be inspected with:

```powershell
cargo metadata --format-version 1 --filter-platform x86_64-pc-windows-msvc
```

Most transitive dependencies are licensed under permissive licenses such as MIT,
Apache-2.0, BSD-3-Clause, Zlib, Unlicense, Unicode-3.0, CC0-1.0, or MPL-2.0.
