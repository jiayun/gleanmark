const API_BASE = "http://localhost:21580";

document.addEventListener("DOMContentLoaded", async () => {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab) return;

  document.getElementById("url").textContent = tab.url;
  document.getElementById("title").value = tab.title || "";

  // Extract content from the active tab
  try {
    const [result] = await chrome.scripting.executeScript({
      target: { tabId: tab.id },
      func: extractPageContent,
    });
    if (result?.result) {
      document.getElementById("content").value = result.result;
    }
  } catch (e) {
    console.warn("Could not extract content:", e);
  }
});

document.getElementById("save").addEventListener("click", async () => {
  const btn = document.getElementById("save");
  const status = document.getElementById("status");
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });

  btn.disabled = true;
  btn.textContent = "Saving...";
  status.textContent = "";
  status.className = "status";

  const tagsRaw = document.getElementById("tags").value.trim();
  const tags = tagsRaw ? tagsRaw.split(",").map((t) => t.trim()).filter(Boolean) : [];

  const payload = {
    url: tab.url,
    title: document.getElementById("title").value || tab.title,
    content: document.getElementById("content").value,
    tags: tags.length > 0 ? tags : null,
  };

  try {
    const res = await fetch(`${API_BASE}/api/bookmarks`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || `HTTP ${res.status}`);
    }

    btn.textContent = "Saved!";
    btn.className = "success";
    setTimeout(() => window.close(), 600);
  } catch (e) {
    btn.textContent = "Save Bookmark";
    btn.disabled = false;
    btn.className = "error";
    status.textContent = e.message.includes("fetch")
      ? "Cannot connect to GleanMark server. Is it running?"
      : e.message;
    status.className = "status error";
    setTimeout(() => { btn.className = ""; }, 1500);
  }
});

// This function runs in the content script context
function extractPageContent() {
  const selectors = ["article", "main", '[role="main"]', ".post-content", ".entry-content", ".content"];
  for (const sel of selectors) {
    const el = document.querySelector(sel);
    if (el && el.innerText.trim().length > 100) {
      return el.innerText.trim().substring(0, 5000);
    }
  }
  // Fallback: get meta description + body text
  const meta = document.querySelector('meta[name="description"]')?.content || "";
  const body = document.body.innerText.trim().substring(0, 3000);
  return meta ? `${meta}\n\n${body}` : body;
}
