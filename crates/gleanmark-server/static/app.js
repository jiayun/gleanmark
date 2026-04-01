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

  async listBookmarks(limit = 50) {
    const res = await fetch(`/api/bookmarks?limit=${limit}`);
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
}

// Nav click handlers
document.querySelectorAll('nav a').forEach(a => {
  a.addEventListener('click', e => {
    e.preventDefault();
    const view = a.dataset.view;
    window.location.hash = view;
    switchView(view);
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
let bookmarkLimit = 50;

async function loadBookmarks() {
  const list = document.getElementById('bookmark-list');
  const count = document.getElementById('bookmark-count');
  const loadMore = document.getElementById('load-more');

  list.innerHTML = '<p class="loading">Loading...</p>';

  try {
    const bookmarks = await API.listBookmarks(bookmarkLimit);
    count.textContent = `${bookmarks.length} bookmark${bookmarks.length !== 1 ? 's' : ''}`;

    if (bookmarks.length === 0) {
      list.innerHTML = '<p class="empty-state">No bookmarks yet.</p>';
      loadMore.classList.add('hidden');
    } else {
      list.innerHTML = bookmarks
        .map(b => renderBookmarkCard(b, { showDelete: true }))
        .join('');

      if (bookmarks.length >= bookmarkLimit) {
        loadMore.classList.remove('hidden');
      } else {
        loadMore.classList.add('hidden');
      }
    }
  } catch (err) {
    list.innerHTML = `<p class="empty-state" style="color:#c5221f">Error: ${escapeHtml(err.message)}</p>`;
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
  bookmarkLimit += 50;
  loadBookmarks();
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

// Init
handleHash();
