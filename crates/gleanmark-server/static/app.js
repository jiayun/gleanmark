// Theme: light / dark / system
const THEME_ICONS = { light: '\u2600', dark: '\u{1F319}', system: '\u{1F5A5}' }; // sun, moon, monitor
const THEME_CYCLE = ['light', 'dark', 'system'];

function getSystemTheme() {
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function applyTheme(theme) {
  const actual = theme === 'system' ? getSystemTheme() : theme;
  document.documentElement.classList.toggle('dark', actual === 'dark');
}

function initTheme() {
  const stored = localStorage.getItem('theme') || 'system';
  applyTheme(stored);

  const btn = document.getElementById('theme-toggle');
  btn.textContent = THEME_ICONS[stored];
  btn.addEventListener('click', () => {
    const current = localStorage.getItem('theme') || 'system';
    const next = THEME_CYCLE[(THEME_CYCLE.indexOf(current) + 1) % THEME_CYCLE.length];
    localStorage.setItem('theme', next);
    applyTheme(next);
    btn.textContent = THEME_ICONS[next];
  });

  // Listen for system theme changes
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    if ((localStorage.getItem('theme') || 'system') === 'system') {
      applyTheme('system');
    }
  });
}

initTheme();

// API Layer
const API = {
  async search(query, limit = 10, tags = null) {
    const body = { query, limit };
    if (tags) body.tags = tags;
    const res = await fetch('/api/search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error((await res.json()).error || `HTTP ${res.status}`);
    return res.json();
  },

  async listBookmarks(limit = 50, offset = null) {
    let url = `/api/bookmarks?limit=${limit}`;
    if (offset) url += `&offset=${encodeURIComponent(offset)}`;
    const res = await fetch(url);
    if (!res.ok) throw new Error((await res.json()).error || `HTTP ${res.status}`);
    return res.json();
  },

  async deleteBookmark(id) {
    const res = await fetch(`/api/bookmarks/${id}`, { method: 'DELETE' });
    if (!res.ok) throw new Error((await res.json()).error || `HTTP ${res.status}`);
  },

  async exportBookmarks(path) {
    const res = await fetch('/api/export', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ path }),
    });
    if (!res.ok) throw new Error((await res.json()).error || `HTTP ${res.status}`);
    return res.json();
  },

  async importBookmarks(path) {
    const res = await fetch('/api/import', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ path }),
    });
    if (!res.ok) throw new Error((await res.json()).error || `HTTP ${res.status}`);
    return res.json();
  },

  async getConfig() {
    const res = await fetch('/api/config');
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.json();
  },

  async saveConfig(payload) {
    const res = await fetch('/api/config', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    if (!res.ok) throw new Error((await res.json().catch(() => ({}))).error || `HTTP ${res.status}`);
    return res.json();
  },

  async getAuthStatus() {
    const res = await fetch('/api/auth/status');
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.json();
  },

  async login(email, password) {
    const res = await fetch('/api/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email, password }),
    });
    if (!res.ok) throw new Error((await res.json().catch(() => ({}))).error || `HTTP ${res.status}`);
    return res.json();
  },

  async logout() {
    const res = await fetch('/api/auth/logout', { method: 'POST' });
    if (!res.ok) throw new Error((await res.json().catch(() => ({}))).error || `HTTP ${res.status}`);
    return res.json();
  },
};

// View Switcher
function switchView(name) {
  document.querySelectorAll('section[data-view]').forEach(s => s.classList.add('hidden'));
  const target = document.querySelector(`section[data-view="${name}"]`);
  if (target) target.classList.remove('hidden');

  document.querySelectorAll('nav a').forEach(a =>
    a.classList.toggle('active', a.dataset.view === name)
  );

  if (name === 'bookmarks') loadBookmarks();
  if (name === 'settings') loadSettings();
}

// Nav click handlers — only set hash, let hashchange handle the switch
document.querySelectorAll('nav a').forEach(a => {
  a.addEventListener('click', e => {
    e.preventDefault();
    window.location.hash = a.dataset.view;
  });
});

// Hash routing
function handleHash() {
  const hash = window.location.hash.slice(1) || 'search';
  switchView(hash);
}
window.addEventListener('hashchange', handleHash);

// Render helpers
function renderTags(tags) {
  if (!tags || tags.length === 0) return '';
  return tags.map(t => `<span class="tag">${escapeHtml(t)}</span>`).join('');
}

function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}

function renderBookmarkCard(bookmark, { showScore = false, score = 0, showDelete = false } = {}) {
  const tags = renderTags(bookmark.tags);
  const scoreHtml = showScore ? `<span class="score">${score.toFixed(3)}</span>` : '';
  const deleteBtn = showDelete
    ? `<button class="btn-delete" data-id="${escapeHtml(bookmark.id)}" title="Delete">&times;</button>`
    : '';

  return `
    <div class="card">
      <div class="card-header">
        <a href="${escapeHtml(bookmark.url)}" target="_blank" rel="noopener" class="card-title">${escapeHtml(bookmark.title)}</a>
        ${deleteBtn}
      </div>
      <div class="card-url">${escapeHtml(bookmark.url)}</div>
      <div class="card-meta">
        ${tags}
        ${scoreHtml}
      </div>
    </div>
  `;
}

// Search View
const searchForm = document.getElementById('search-form');
const searchInput = document.getElementById('search-input');
const searchResults = document.getElementById('search-results');

searchForm.addEventListener('submit', async e => {
  e.preventDefault();
  const query = searchInput.value.trim();
  if (!query) return;

  searchResults.innerHTML = '<p class="loading">Searching...</p>';

  try {
    const results = await API.search(query);
    if (results.length === 0) {
      searchResults.innerHTML = '<p class="empty-state">No results found.</p>';
    } else {
      searchResults.innerHTML = results
        .map(r => renderBookmarkCard(r.bookmark, { showScore: true, score: r.score }))
        .join('');
    }
  } catch (err) {
    searchResults.innerHTML = `<p class="empty-state" style="color:#c5221f">Error: ${escapeHtml(err.message)}</p>`;
  }
});

// Bookmarks View
const PAGE_SIZE = 50;
let nextOffset = null;
let totalLoaded = 0;

async function loadBookmarks(append = false) {
  const list = document.getElementById('bookmark-list');
  const count = document.getElementById('bookmark-count');
  const loadMore = document.getElementById('load-more');

  if (!append) {
    list.innerHTML = '<p class="loading">Loading...</p>';
    nextOffset = null;
    totalLoaded = 0;
  }

  try {
    const bookmarks = await API.listBookmarks(PAGE_SIZE, append ? nextOffset : null);
    totalLoaded += bookmarks.length;
    nextOffset = bookmarks.length >= PAGE_SIZE ? bookmarks[bookmarks.length - 1].id : null;
    count.textContent = `${totalLoaded} bookmark${totalLoaded !== 1 ? 's' : ''}`;

    if (totalLoaded === 0) {
      list.innerHTML = '<p class="empty-state">No bookmarks yet.</p>';
      loadMore.classList.add('hidden');
    } else {
      const html = bookmarks
        .map(b => renderBookmarkCard(b, { showDelete: true }))
        .join('');

      if (append) {
        list.insertAdjacentHTML('beforeend', html);
      } else {
        list.innerHTML = html;
      }

      if (nextOffset) {
        loadMore.classList.remove('hidden');
      } else {
        loadMore.classList.add('hidden');
      }
    }
  } catch (err) {
    if (!append) {
      list.innerHTML = `<p class="empty-state" style="color:#c5221f">Error: ${escapeHtml(err.message)}</p>`;
    }
  }
}

// Delete bookmark (event delegation)
document.getElementById('bookmark-list').addEventListener('click', async e => {
  const btn = e.target.closest('.btn-delete');
  if (!btn) return;

  const id = btn.dataset.id;
  if (!confirm('Delete this bookmark?')) return;

  try {
    await API.deleteBookmark(id);
    loadBookmarks();
  } catch (err) {
    alert('Failed to delete: ' + err.message);
  }
});

// Load more
document.getElementById('load-more').addEventListener('click', () => {
  loadBookmarks(true);
});

// Export
document.getElementById('export-form').addEventListener('submit', async e => {
  e.preventDefault();
  const path = document.getElementById('export-path').value.trim();
  const status = document.getElementById('export-status');
  if (!path) return;

  status.className = 'status';
  status.textContent = 'Exporting...';

  try {
    const result = await API.exportBookmarks(path);
    status.className = 'status success';
    status.textContent = `Exported ${result.exported} bookmarks to ${path}`;
  } catch (err) {
    status.className = 'status error';
    status.textContent = 'Error: ' + err.message;
  }
});

// Import
document.getElementById('import-form').addEventListener('submit', async e => {
  e.preventDefault();
  const path = document.getElementById('import-path').value.trim();
  const status = document.getElementById('import-status');
  if (!path) return;

  status.className = 'status';
  status.textContent = 'Importing...';

  try {
    const result = await API.importBookmarks(path);
    status.className = 'status success';
    status.textContent = `Imported ${result.imported} bookmarks from ${path}`;
  } catch (err) {
    status.className = 'status error';
    status.textContent = 'Error: ' + err.message;
  }
});

// Open external links in system browser
document.addEventListener('click', e => {
  const a = e.target.closest('a[target="_blank"]');
  if (!a) return;
  e.preventDefault();
  // Use server-side open endpoint (works in both browser and Tauri WebView)
  fetch('/api/open', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ url: a.href }),
  }).catch(() => {
    // Fallback: try window.open
    window.open(a.href, '_blank');
  });
});

// Settings View
function updateCloudFieldsVisibility() {
  const checked = document.querySelector('input[name="mode"]:checked');
  const cloudFields = document.getElementById('cloud-fields');
  cloudFields.style.display = checked && checked.value === 'cloud' ? 'flex' : 'none';
}

async function loadSettings() {
  const current = document.getElementById('settings-current');
  const status = document.getElementById('settings-status');
  status.className = 'status';
  status.textContent = '';

  try {
    const c = await API.getConfig();
    document.querySelectorAll('input[name="mode"]').forEach(r => {
      r.checked = r.value === c.mode;
    });
    document.getElementById('gateway-url').value = c.gateway_url || '';
    document.getElementById('supabase-url').value = c.supabase_url || '';
    document.getElementById('supabase-anon-key').value = c.supabase_anon_key || '';
    updateCloudFieldsVisibility();

    current.className = 'status';
    current.textContent = c.mode === 'cloud'
      ? `Current: Cloud — ${c.gateway_url || '(no URL set)'}`
      : 'Current: Local (bundled Qdrant)';
  } catch (err) {
    current.className = 'status error';
    current.textContent = 'Failed to load config: ' + err.message;
  }

  loadAuthStatus();
}

// Reflect sign-in state: show the Account section only in cloud mode, and
// toggle between the login form and a "signed in as ..." + Sign out button.
async function loadAuthStatus() {
  const section = document.getElementById('account-section');
  const authStatus = document.getElementById('auth-status');
  const loginForm = document.getElementById('login-form');
  const logoutBtn = document.getElementById('logout-btn');
  const loginStatus = document.getElementById('login-status');
  loginStatus.className = 'status';
  loginStatus.textContent = '';

  try {
    const s = await API.getAuthStatus();
    if (!s.cloud) {
      section.classList.add('hidden');
      return;
    }
    section.classList.remove('hidden');
    if (s.signed_in) {
      authStatus.className = 'status success';
      authStatus.textContent = `Signed in${s.email ? ' as ' + s.email : ''}.`;
      loginForm.classList.add('hidden');
      logoutBtn.classList.remove('hidden');
    } else {
      authStatus.className = 'status';
      authStatus.textContent = 'Not signed in.';
      loginForm.classList.remove('hidden');
      logoutBtn.classList.add('hidden');
    }
  } catch (err) {
    section.classList.remove('hidden');
    authStatus.className = 'status error';
    authStatus.textContent = 'Failed to load account: ' + err.message;
  }
}

document.querySelectorAll('input[name="mode"]').forEach(r =>
  r.addEventListener('change', updateCloudFieldsVisibility)
);

document.getElementById('settings-form').addEventListener('submit', async e => {
  e.preventDefault();
  const status = document.getElementById('settings-status');
  const checked = document.querySelector('input[name="mode"]:checked');
  if (!checked) return;

  const payload = { mode: checked.value };
  if (checked.value === 'cloud') {
    payload.gateway_url = document.getElementById('gateway-url').value.trim();
    payload.supabase_url = document.getElementById('supabase-url').value.trim();
    payload.supabase_anon_key = document.getElementById('supabase-anon-key').value.trim();
    if (!payload.gateway_url || !payload.supabase_url || !payload.supabase_anon_key) {
      status.className = 'status error';
      status.textContent = 'Gateway URL, Supabase URL and anon key are all required for cloud mode.';
      return;
    }
  }

  status.className = 'status';
  status.textContent = 'Saving...';
  try {
    const res = await API.saveConfig(payload);
    status.className = 'status success';
    status.textContent = res.restart_required
      ? 'Saved. Restart GleanMark for the change to take effect.'
      : 'Saved.';
    loadSettings();
  } catch (err) {
    status.className = 'status error';
    status.textContent = 'Error: ' + err.message;
  }
});

document.getElementById('login-form').addEventListener('submit', async e => {
  e.preventDefault();
  const status = document.getElementById('login-status');
  const email = document.getElementById('login-email').value.trim();
  const password = document.getElementById('login-password').value;
  if (!email || !password) return;

  status.className = 'status';
  status.textContent = 'Signing in...';
  try {
    await API.login(email, password);
    document.getElementById('login-password').value = '';
    status.className = 'status';
    status.textContent = '';
    loadAuthStatus();
  } catch (err) {
    status.className = 'status error';
    status.textContent = 'Sign-in failed: ' + err.message;
  }
});

document.getElementById('logout-btn').addEventListener('click', async () => {
  const status = document.getElementById('login-status');
  try {
    await API.logout();
    loadAuthStatus();
  } catch (err) {
    status.className = 'status error';
    status.textContent = 'Sign-out failed: ' + err.message;
  }
});

// Init
handleHash();
