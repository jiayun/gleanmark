# Changelog

All notable changes to GleanMark will be documented in this file.

## [0.1.7] - 2026-07-14

### Added

- Cloud usage display now shows plan limits: "This month: 3 / 30 · Total: 29 / 500". Unlimited plans show plain counts.

### Changed

- Hitting a cloud plan limit now shows a clear message (e.g. "Monthly bookmark limit reached (30/30). Upgrade your plan to save more.") in the extension popup, web UI, and CLI, instead of a raw gateway error string.

## [0.1.6] - 2026-07-02

### Added

- Automatic update check on launch: a few seconds after startup the app silently checks for a newer version and prompts to install only when one is available. Previously updates were checked only via the "Check for Updates…" menu item, so fixes didn't reach users unless they checked manually.

## [0.1.5] - 2026-07-02

### Fixed

- Fix export/import failing when the path starts with `~` (including the default `~/gleanmark-export.json`): a leading `~` is now expanded to the home directory. Applies to the desktop app, CLI, and server.

## [0.1.4] - 2026-07-01

### Fixed

- Fix broken Settings → Backend layout: the Local/Cloud radio buttons were stretched full-width, pushing their labels off to the side. Radio inputs are now excluded from the full-width field styling.

## [0.1.3] - 2026-07-01

### Added

- **Optional Cloud backend (invite-only)** — sync bookmarks across devices via a hosted gateway. Embeddings are still computed on-device; only the pre-computed vectors + content are sent to your private, per-account collection. Turn it on in **Settings → Backend → Cloud** and sign in — no URLs or keys to configure. Local remains the default and is unchanged.
- **Cloud interest waitlist** — a **Settings → "Interested in GleanMark Cloud?"** form to request access (and say whether you'd pay), since cloud is invite-only for now.

### Changed

- README repositioned as local-first with an optional invite-only Cloud backend.

## [0.1.2] - 2026-04-04

### Fixed

- Fix app crash when launched from Finder/Dock: app bundle was missing `_CodeSignature` directory because bundle-level code signing was never performed. Now uses ad-hoc signing (`signingIdentity: "-"`) so macOS Gatekeeper accepts the bundle without an Apple Developer certificate.
- Fix app crash when global shortcut registration fails (e.g. Accessibility permission reset after macOS update). The shortcut failure is now a warning instead of a fatal error.
- Fix app crash due to Qdrant client/server version mismatch: bundled Qdrant server was v1.14.0 but qdrant-client requires v1.16+. Updated bundled Qdrant to v1.17.0.

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
