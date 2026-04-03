# Changelog

All notable changes to GleanMark will be documented in this file.

## [0.1.2] - 2026-04-04

### Fixed

- Fix app crash when launched from Finder/Dock: app bundle was missing `_CodeSignature` directory because bundle-level code signing was never performed. Now uses ad-hoc signing (`signingIdentity: "-"`) so macOS Gatekeeper accepts the bundle without an Apple Developer certificate.
- Fix app crash when global shortcut registration fails (e.g. Accessibility permission reset after macOS update). The shortcut failure is now a warning instead of a fatal error.

### Improved

- Auto-updater: show download progress bar in the app window instead of only logging to console
- Auto-updater: native confirmation dialog before downloading updates
- Auto-updater: prompt to restart after installation with one-click restart (via `app.restart()`)
- Auto-updater: proper error dialogs instead of JavaScript `alert()`

## [0.1.1] - 2026-04-04

### Fixed

- Fix app crash on macOS after system update: Qdrant sidecar binary blocked by Gatekeeper due to `com.apple.quarantine` attribute. The app now removes quarantine via `removexattr` syscall before launching Qdrant.
- Fix `prepare_sidecar()` failing to create symlink because it looked for the wrong binary name (with target triple suffix) in the app bundle. Now tries both naming conventions.
- Fix broken symlink not being detected: if the Qdrant symlink target no longer exists (e.g. after app update), it is now automatically recreated.

## [0.1.0] - 2026-03-26

### Added

- Initial release
- Hybrid search: semantic (MultilingualE5) + keyword (SPLADE/BM25) with Reciprocal Rank Fusion
- Multilingual support with jieba Chinese word segmentation
- Desktop app (Tauri v2) with system tray, global hotkey (Cmd+Shift+G), and auto-updater
- Web UI at `http://localhost:21580`
- Chrome extension (Manifest V3) for one-click saving
- CLI tool for terminal workflows
- MCP server for Claude Code / Claude Desktop integration
- Local embeddings via fastembed-rs, no API keys needed
- JSON import/export for data portability
