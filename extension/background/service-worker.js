// GleanMark background service worker
// Currently minimal — popup handles API calls directly via fetch.
// This service worker is reserved for future features like:
// - Context menu "Save to GleanMark"
// - Keyboard shortcut handling
// - Offline queue for when server is down

chrome.runtime.onInstalled.addListener(() => {
  console.log("GleanMark extension installed");
});
