const DEFAULT_URL = 'http://localhost:21580';

document.addEventListener('DOMContentLoaded', () => {
  chrome.storage.sync.get({ serverUrl: DEFAULT_URL }, (items) => {
    document.getElementById('server-url').value = items.serverUrl;
  });
});

document.getElementById('save').addEventListener('click', () => {
  const url = document.getElementById('server-url').value.trim().replace(/\/+$/, '') || DEFAULT_URL;
  chrome.storage.sync.set({ serverUrl: url }, () => {
    document.getElementById('status').textContent = 'Saved!';
    setTimeout(() => { document.getElementById('status').textContent = ''; }, 1500);
  });
});
