# Privacy Policy

**Last updated:** 2026-04-03

## Overview

GleanMark is a local-first bookmark search tool. Your data stays on your machine.

## Data Collection

GleanMark and its Chrome extension **do not collect, transmit, or store any data on external servers**.

### What the Chrome extension accesses

- **Current page URL and title** — only when you click the extension icon to save a bookmark
- **Page text content** — extracted from the current page for semantic search indexing
- **Extension settings** — your configured server URL, stored locally via Chrome's storage API

### Where data is stored

All bookmark data is stored exclusively on your local machine:

- Bookmark content and embeddings are stored in a local Qdrant database
- Extension settings are stored in Chrome's local storage
- No data is sent to any cloud service, analytics platform, or third party

### Network connections

The extension connects only to your local GleanMark server (default: `localhost:21580`). No other network connections are made.

## Data Sharing

We do not sell, trade, or transfer your data to any third party.

## Auto-Updater

The GleanMark desktop application checks for updates by contacting GitHub Releases (`github.com`). This is a standard HTTPS request that does not transmit any personal data or bookmark content.

## Open Source

GleanMark is open source. You can inspect the complete source code at [github.com/jiayun/gleanmark](https://github.com/jiayun/gleanmark) to verify these privacy practices.

## Contact

For privacy concerns, please open an issue at [github.com/jiayun/gleanmark/issues](https://github.com/jiayun/gleanmark/issues).
