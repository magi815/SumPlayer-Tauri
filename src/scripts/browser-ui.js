// SumPlayer Tauri Frontend - browser-ui.js
// Uses @tauri-apps/api invoke() instead of Electron IPC

// Tauri v2 API - loaded globally via the Tauri runtime
let invoke, listen;

function tryLoadTauriApi() {
  // Strategy 1: @tauri-apps/api global with core.invoke and event.listen
  try {
    if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === 'function') {
      invoke = window.__TAURI__.core.invoke;
    }
  } catch (_) {}

  // Strategy 2: __TAURI_INTERNALS__.invoke fallback
  if (!invoke) {
    try {
      if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
        invoke = window.__TAURI_INTERNALS__.invoke;
      }
    } catch (_) {}
  }

  // For listen: try event module first
  if (!listen) {
    try {
      if (window.__TAURI__ && window.__TAURI__.event && typeof window.__TAURI__.event.listen === 'function') {
        listen = window.__TAURI__.event.listen;
      }
    } catch (_) {}
  }

  // For listen: build from core.transformCallback + invoke if available
  if (!listen && invoke) {
    try {
      var tc = (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.transformCallback)
        || (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.transformCallback);
      if (typeof tc === 'function') {
        listen = function(event, handler) {
          var cbId = tc(function(e) { handler(e); }, true);
          return invoke('plugin:event|listen', { event: event, target: { kind: 'Any' }, handler: cbId }).then(function(id) {
            return function() { invoke('plugin:event|unlisten', { event: event, eventId: id }); };
          });
        };
      }
    } catch (_) {}
  }

  return !!(invoke && listen);
}

tryLoadTauriApi();

// Helper: show overlay and emit event with retry to handle race condition
// The overlay webview may not have its listeners ready yet, so we retry
function showOverlayAndEmit(eventName, payloadFn) {
  api.showMenuOverlay().then(() => {
    const tauriEvent = window.__TAURI__ && window.__TAURI__.event;
    if (!tauriEvent) return;
    const payload = payloadFn();
    let done = false;
    // Listen for overlay-ready (fires after overlay sets up listeners)
    tauriEvent.listen('overlay-ready', () => {
      if (!done) { done = true; tauriEvent.emit(eventName, payload); }
    }).then(unlisten => { setTimeout(() => unlisten(), 3000); });
    // Retry emit every 100ms to handle already-initialized overlay
    let attempts = 0;
    const retry = setInterval(() => {
      tauriEvent.emit(eventName, payload);
      attempts++;
      if (attempts >= 10 || done) clearInterval(retry);
    }, 100);
  });
}

// ─── API wrapper (mirrors the Electron preload API) ───

const api = {
  // Tab operations
  createTab: (url) => invoke('tab_create', { url: url || null }),
  closeTab: (id) => invoke('tab_close', { id }),
  switchTab: (id) => invoke('tab_switch', { id }),
  restoreTab: () => invoke('tab_restore'),
  nextTab: () => invoke('tab_next'),
  prevTab: () => invoke('tab_prev'),
  pinTab: (id) => invoke('tab_pin', { id }),
  moveTab: (tabId, beforeTabId) => invoke('tab_move', { tabId, beforeTabId: beforeTabId || null }),

  // Navigation
  navigate: (url) => invoke('nav_go', { url }),
  goBack: () => invoke('nav_back'),
  goForward: () => invoke('nav_forward'),
  reload: () => invoke('nav_reload'),
  goHome: () => invoke('nav_home'),
  getHomePage: () => invoke('settings_get_home_page'),
  setHomePage: (url) => invoke('settings_set_home_page', { url }),
  translatePage: (targetLang) => invoke('page_translate', { targetLang }),

  // Zoom
  zoomIn: () => invoke('zoom_in'),
  zoomOut: () => invoke('zoom_out'),
  zoomReset: () => invoke('zoom_reset'),

  // Find in page
  findInPage: (text, forward) => invoke('find_start', { text, forward }),
  stopFind: () => invoke('find_stop'),

  // Screenshot - full page capture using bundled html2canvas (injected via eval to bypass CSP)
  captureScreenshot: () => invoke('capture_full_screenshot'),

  // Window controls
  minimizeWindow: () => invoke('window_minimize'),
  maximizeWindow: () => invoke('window_maximize'),
  closeWindow: () => invoke('window_close'),
  toggleFullScreen: () => invoke('window_fullscreen'),
  toggleDevTools: () => invoke('open_devtools'),

  // Control server
  toggleControlServer: () => invoke('control_server_toggle'),
  getControlServerStatus: () => invoke('control_server_status'),

  // History
  historyList: (query, limit, offset) => invoke('history_list', { query: query || null, limit: limit || null, offset: offset || null }),
  historyDelete: (id) => invoke('history_delete', { id }),
  historyClear: () => invoke('history_clear'),
  historySearch: (query, limit) => invoke('history_search', { query, limit: limit || null }),

  // Bookmarks
  bookmarkAdd: (url, title, folderId, favicon) => invoke('bookmark_add', { url, title, folderId: folderId || null, favicon: favicon || null }),
  bookmarkRemove: (id) => invoke('bookmark_remove', { id }),
  bookmarkUpdate: (id, updates) => invoke('bookmark_update', { id, title: updates.title || null, url: updates.url || null, folderId: updates.folderId || null }),
  bookmarkCheck: (url) => invoke('bookmark_check', { url }),
  bookmarkList: (folderId) => invoke('bookmark_list', { folderId: folderId || null }),
  bookmarkAll: () => invoke('bookmark_all'),
  bookmarkMove: (bookmarkId, beforeBookmarkId) => invoke('bookmark_move', { bookmarkId, beforeBookmarkId: beforeBookmarkId || null }),

  // Downloads
  downloadList: () => invoke('download_list'),
  downloadHistory: () => invoke('download_history'),
  downloadCancel: (id) => invoke('download_cancel', { id }),
  downloadOpen: (path) => invoke('download_open', { path }),
  downloadOpenFolder: (path) => invoke('download_open_folder', { path }),
  downloadClearHistory: () => invoke('download_clear_history'),

  // Chrome height
  setChromeHeight: (height) => invoke('set_chrome_height', { height }),
  resizeTabs: () => invoke('resize_tabs'),
  setTabOffset: (extraTop) => invoke('set_tab_offset', { extraTop }),
  showMenuOverlay: () => invoke('show_menu_overlay'),
  hideMenuOverlay: () => invoke('hide_menu_overlay'),

  // Chrome expanded (not needed in Tauri since UI is part of webview)
  setChromeExpanded: () => Promise.resolve(),
};

// ─── Tauri event listeners ───

const eventCallbacks = {};

function onEvent(eventName, callback) {
  if (!eventCallbacks[eventName]) {
    eventCallbacks[eventName] = [];
    listen(eventName, (event) => {
      for (const cb of eventCallbacks[eventName]) {
        cb(event.payload);
      }
    });
  }
  eventCallbacks[eventName].push(callback);
}

// ─── DOM elements ───

const tabsContainer = document.getElementById('tabs-container');
const newTabBtn = document.getElementById('new-tab-btn');
const urlInput = document.getElementById('url-input');
const btnBack = document.getElementById('btn-back');
const btnForward = document.getElementById('btn-forward');
const btnReload = document.getElementById('btn-reload');
const btnHome = document.getElementById('btn-home');
const btnMinimize = document.getElementById('btn-minimize');
const btnMaximize = document.getElementById('btn-maximize');
const btnClose = document.getElementById('btn-close');
const zoomIndicator = document.getElementById('zoom-indicator');
const findBar = document.getElementById('find-bar');
const findInput = document.getElementById('find-input');
const findMatches = document.getElementById('find-matches');
const findPrev = document.getElementById('find-prev');
const findNext = document.getElementById('find-next');
const findClose = document.getElementById('find-close');
const bookmarkBar = document.getElementById('bookmark-bar');
const bookmarkItems = document.getElementById('bookmark-items');
const panelOverlay = document.getElementById('panel-overlay');
const panelTitle = document.getElementById('panel-title');
const panelClose = document.getElementById('panel-close');
const panelSearch = document.getElementById('panel-search');
const panelClear = document.getElementById('panel-clear');
const panelContent = document.getElementById('panel-content');
const btnBookmark = document.getElementById('btn-bookmark');
const btnScreenshot = document.getElementById('btn-screenshot');
const btnMenu = document.getElementById('btn-menu');
const menuDropdown = document.getElementById('menu-dropdown');
const menuHome = document.getElementById('menu-home');
const menuHistory = document.getElementById('menu-history');
const translateSubmenu = document.getElementById('translate-submenu');
const homepageDialog = document.getElementById('homepage-dialog');
const homepageUrlInput = document.getElementById('homepage-url-input');
const homepageSaveBtn = document.getElementById('homepage-save-btn');
const homepageCancelBtn = document.getElementById('homepage-cancel-btn');
const homepageCurrentBtn = document.getElementById('homepage-current-btn');

let activeTabId = null;
let currentUrl = '';
let currentTitle = '';
let currentFavicon = '';
let currentPanel = null;

// ─── Bookmark star ───

async function updateBookmarkStar() {
  if (!currentUrl) {
    btnBookmark.classList.remove('bookmarked');
    return;
  }
  const existing = await api.bookmarkCheck(currentUrl);
  if (existing) {
    btnBookmark.classList.add('bookmarked');
  } else {
    btnBookmark.classList.remove('bookmarked');
  }
}

// ─── Tab management ───

function createTabElement(id, title) {
  const tab = document.createElement('div');
  tab.className = 'tab';
  tab.dataset.tabId = String(id);
  tab.draggable = true;

  const pinIcon = document.createElement('span');
  pinIcon.className = 'tab-pin-icon';
  pinIcon.textContent = '\uD83D\uDCCC';

  const favicon = document.createElement('img');
  favicon.className = 'tab-favicon';
  favicon.width = 16;
  favicon.height = 16;
  favicon.style.display = 'none';
  favicon.addEventListener('error', () => { favicon.style.display = 'none'; });

  const titleSpan = document.createElement('span');
  titleSpan.className = 'tab-title';
  titleSpan.textContent = title || 'New Tab';

  const closeBtn = document.createElement('button');
  closeBtn.className = 'tab-close';
  closeBtn.textContent = '\u2715';
  closeBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    api.closeTab(id);
  });

  tab.appendChild(pinIcon);
  tab.appendChild(favicon);
  tab.appendChild(titleSpan);
  tab.appendChild(closeBtn);
  tab.addEventListener('click', () => api.switchTab(id));

  // Tab drag
  tab.addEventListener('dragstart', (e) => {
    e.dataTransfer.setData('text/plain', String(id));
    tab.classList.add('dragging');
  });
  tab.addEventListener('dragend', () => {
    tab.classList.remove('dragging');
    document.querySelectorAll('.tab.drag-over').forEach(el => el.classList.remove('drag-over'));
  });
  tab.addEventListener('dragover', (e) => {
    e.preventDefault();
    tab.classList.add('drag-over');
  });
  tab.addEventListener('dragleave', () => {
    tab.classList.remove('drag-over');
  });
  tab.addEventListener('drop', (e) => {
    e.preventDefault();
    tab.classList.remove('drag-over');
    const draggedId = Number(e.dataTransfer.getData('text/plain'));
    const targetId = Number(tab.dataset.tabId);
    if (draggedId !== targetId) {
      api.moveTab(draggedId, targetId);
      const draggedEl = tabsContainer.querySelector(`[data-tab-id="${draggedId}"]`);
      if (draggedEl) {
        tabsContainer.insertBefore(draggedEl, tab);
      }
    }
  });

  return tab;
}

function setActiveTab(id) {
  activeTabId = id;
  document.querySelectorAll('.tab').forEach((tab) => {
    tab.classList.toggle('active', tab.dataset.tabId === String(id));
  });
}

// ─── Find bar ───

function showFindBar() {
  findBar.classList.remove('hidden');
  findInput.focus();
  findInput.select();
  findCurrentIndex = 0;
  findTotalCount = 0;
  // Notify backend that find bar changed chrome height
  reportChromeHeight();
}

function hideFindBar() {
  findBar.classList.add('hidden');
  findMatches.textContent = '0/0';
  findInput.value = '';
  findCurrentIndex = 0;
  findTotalCount = 0;
  api.stopFind();
  // Notify backend that find bar changed chrome height
  reportChromeHeight();
}

let findCurrentIndex = 0;
let findTotalCount = 0;

function doFind(forward) {
  const text = findInput.value.trim();
  if (text) {
    api.findInPage(text, forward);
    if (forward) findCurrentIndex++; else findCurrentIndex--;
    // Request match count from the active tab via JS injection
    var findCode = '(function(){' +
      'var text = ' + JSON.stringify(text) + ';' +
      'if(!text) return;' +
      'var count = 0;' +
      'var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null, false);' +
      'var re = new RegExp(text.replace(/[.*+?^${}()|[\\]\\\\]/g, "\\\\$&"), "gi");' +
      'while(walker.nextNode()){' +
      '  var matches = walker.currentNode.textContent.match(re);' +
      '  if(matches) count += matches.length;' +
      '}' +
      'if(window.__TAURI__) window.__TAURI__.core.invoke("find_count_result", {count: count});' +
      '})()';
    invoke('page_exec', { code: findCode }).catch(() => {});
  }
}

// ─── Panel (History / Downloads) ───

function formatTime(ts) {
  const d = new Date(ts);
  const now = new Date();
  const isToday = d.toDateString() === now.toDateString();
  const time = d.toLocaleTimeString('ko-KR', { hour: '2-digit', minute: '2-digit' });
  if (isToday) return time;
  return d.toLocaleDateString('ko-KR', { month: 'short', day: 'numeric' }) + ' ' + time;
}

function formatDate(ts) {
  const d = new Date(ts);
  const now = new Date();
  if (d.toDateString() === now.toDateString()) return '오늘';
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  if (d.toDateString() === yesterday.toDateString()) return '어제';
  return d.toLocaleDateString('ko-KR', { year: 'numeric', month: 'long', day: 'numeric' });
}

function formatBytes(bytes) {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function escapeHtml(s) {
  const div = document.createElement('div');
  div.textContent = s;
  return div.innerHTML;
}

async function showPanel(panel) {
  currentPanel = panel;
  showOverlayAndEmit('overlay-show-panel', () => ({ type: panel }));
}

function hidePanel() {
  currentPanel = null;
  api.hideMenuOverlay();
}

async function loadHistory(query) {
  const entries = await api.historyList(query, 200);
  panelContent.innerHTML = '';

  let lastDate = '';
  for (const entry of entries) {
    const dateStr = formatDate(entry.visitedAt);
    if (dateStr !== lastDate) {
      lastDate = dateStr;
      const sep = document.createElement('div');
      sep.className = 'panel-date-separator';
      sep.textContent = dateStr;
      panelContent.appendChild(sep);
    }

    const row = document.createElement('div');
    row.className = 'panel-entry';
    row.innerHTML = `
      <div class="panel-entry-info">
        <div class="panel-entry-title">${escapeHtml(entry.title)}</div>
        <div class="panel-entry-url">${escapeHtml(entry.url)}</div>
      </div>
      <span class="panel-entry-time">${formatTime(entry.visitedAt)}</span>
      <button class="panel-entry-delete" title="삭제">&#x2715;</button>
    `;

    row.querySelector('.panel-entry-info').addEventListener('click', () => {
      api.navigate(entry.url);
      hidePanel();
    });

    row.querySelector('.panel-entry-delete').addEventListener('click', async (e) => {
      e.stopPropagation();
      await api.historyDelete(entry.id);
      row.remove();
    });

    panelContent.appendChild(row);
  }

  if (entries.length === 0) {
    panelContent.innerHTML = '<div style="padding:20px;text-align:center;color:#9aa0a6">방문 기록이 없습니다</div>';
  }
}

async function loadDownloads() {
  const result = await api.downloadHistory();
  panelContent.innerHTML = '';

  if (result.length === 0) {
    panelContent.innerHTML = '<div style="padding:20px;text-align:center;color:#9aa0a6">다운로드 기록이 없습니다</div>';
    return;
  }

  for (const dl of result) {
    const row = document.createElement('div');
    row.className = 'panel-entry';
    row.style.flexDirection = 'column';
    row.style.alignItems = 'stretch';

    let statusText = '';
    if (dl.state === 'completed') statusText = formatBytes(dl.totalBytes);
    else if (dl.state === 'cancelled') statusText = '취소됨';
    else if (dl.state === 'interrupted') statusText = '중단됨';
    else statusText = `${formatBytes(dl.receivedBytes)} / ${formatBytes(dl.totalBytes)}`;

    let actionsHtml = '';
    if (dl.state === 'completed') {
      actionsHtml = `<button class="dl-open">열기</button><button class="dl-folder">폴더</button>`;
    }

    row.innerHTML = `
      <div style="display:flex;align-items:center;gap:8px">
        <div class="panel-entry-info">
          <div class="panel-entry-title">${escapeHtml(dl.filename)}</div>
          <div class="panel-entry-url">${statusText}</div>
        </div>
        <div class="download-actions">${actionsHtml}</div>
      </div>
    `;

    row.querySelector('.dl-open')?.addEventListener('click', () => api.downloadOpen(dl.savePath));
    row.querySelector('.dl-folder')?.addEventListener('click', () => api.downloadOpenFolder(dl.savePath));

    panelContent.appendChild(row);
  }
}

// ─── Bookmark bar ───

async function refreshBookmarkBar() {
  const bookmarks = await api.bookmarkList('');
  bookmarkItems.innerHTML = '';

  if (bookmarks.length > 0) {
    for (const bm of bookmarks) {
      const item = document.createElement('button');
      item.className = 'bookmark-item';
      item.title = bm.url;

      if (bm.favicon) {
        const ico = document.createElement('img');
        ico.className = 'bookmark-favicon';
        ico.src = bm.favicon;
        ico.width = 16;
        ico.height = 16;
        ico.addEventListener('error', () => { ico.style.display = 'none'; });
        item.appendChild(ico);
      }

      const label = document.createElement('span');
      label.textContent = bm.title || bm.url;
      item.appendChild(label);
      item.addEventListener('click', () => api.createTab(bm.url));

      // Right-click context menu for bookmark delete/edit
      item.addEventListener('contextmenu', (e) => {
        e.preventDefault();
        e.stopPropagation();
        const tauriEmit = window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.emit;
        if (!tauriEmit) return;
        const bmCtxItems = [
          { label: '새 탭에서 열기', action: 'bm-open-new-tab', payload: bm.url },
          { label: '삭제', action: 'bm-delete', payload: bm.id },
        ];
        showOverlayAndEmit('overlay-show-menu', () => ({
          items: bmCtxItems,
          x: Math.min(e.clientX, window.innerWidth - 160),
          y: e.clientY,
        }));
      });

      item.dataset.bookmarkId = bm.id;
      item.draggable = true;

      item.addEventListener('dragstart', (e) => {
        e.dataTransfer.setData('text/bookmark-id', bm.id);
        item.classList.add('dragging');
      });
      item.addEventListener('dragend', () => {
        item.classList.remove('dragging');
        bookmarkItems.querySelectorAll('.bookmark-item.drag-over').forEach(el => el.classList.remove('drag-over'));
      });
      item.addEventListener('dragover', (e) => {
        e.preventDefault();
        item.classList.add('drag-over');
      });
      item.addEventListener('dragleave', () => {
        item.classList.remove('drag-over');
      });
      item.addEventListener('drop', async (e) => {
        e.preventDefault();
        item.classList.remove('drag-over');
        const draggedId = e.dataTransfer.getData('text/bookmark-id');
        if (draggedId && draggedId !== bm.id) {
          await api.bookmarkMove(draggedId, bm.id);
          refreshBookmarkBar();
        }
      });

      bookmarkItems.appendChild(item);
    }
  } else {
    const empty = document.createElement('span');
    empty.id = 'bookmark-bar-empty';
    empty.textContent = 'Ctrl+D로 즐겨찾기 추가';
    bookmarkItems.appendChild(empty);
  }
}

// ─── Bookmark dialog ───

// ─── Event listeners ───

newTabBtn.addEventListener('click', () => api.createTab());

urlInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    const url = urlInput.value.trim();
    if (url) api.navigate(url);
  }
});
urlInput.addEventListener('focus', () => urlInput.select());

btnBack.addEventListener('click', () => api.goBack());
btnForward.addEventListener('click', () => api.goForward());
btnReload.addEventListener('click', () => api.reload());
btnHome.addEventListener('click', () => api.goHome());
btnBookmark.addEventListener('click', async () => {
  if (!currentUrl) return;
  const existing = await api.bookmarkCheck(currentUrl);
  if (existing) {
    await api.bookmarkRemove(existing.id);
  } else {
    await api.bookmarkAdd(currentUrl, currentTitle || currentUrl, '', currentFavicon);
  }
  refreshBookmarkBar();
  updateBookmarkStar();
});
btnScreenshot.addEventListener('click', async () => {
  btnScreenshot.classList.add('capturing');
  try {
    await api.captureScreenshot();
  } catch (e) {
    console.error('[SumPlayer] Screenshot error:', e);
  }
  setTimeout(() => btnScreenshot.classList.remove('capturing'), 2000);
});

btnMinimize.addEventListener('click', () => api.minimizeWindow());
btnMaximize.addEventListener('click', () => api.maximizeWindow());
btnClose.addEventListener('click', () => api.closeWindow());

// Three-dot menu - uses overlay webview to render on top of tab content
const menuItems = [
  { label: '홈페이지', action: 'menu-home', icon: '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M3 8.5l5-5 5 5"/><path d="M4 8v4.5a1 1 0 0 0 1 1h2v-3h2v3h2a1 1 0 0 0 1-1V8"/></svg>' },
  { label: '방문 기록', action: 'menu-history', icon: '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><circle cx="8" cy="8" r="6"/><path d="M8 4.5V8l2.5 1.5"/></svg>' },
  { label: '다운로드', action: 'menu-downloads', icon: '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M8 2v8M5 7l3 3 3-3"/><path d="M3 11v2h10v-2"/></svg>' },
  { label: '페이지에서 찾기', action: 'menu-find', icon: '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><circle cx="7" cy="7" r="4"/><path d="M10 10l3.5 3.5"/></svg>' },
  { type: 'divider' },
  { label: '페이지 번역', action: 'menu-translate', icon: '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M2 3h7M5.5 3v-1M3.5 3c.5 2 2 4 4 5.5M7 3c-.5 2-2 4-4 5.5"/><path d="M9 9l2 5 2-5M9.5 12.5h3"/></svg>',
    submenu: [
      { label: '한국어', action: 'translate', payload: 'ko' },
      { label: 'English', action: 'translate', payload: 'en' },
      { label: '日本語', action: 'translate', payload: 'ja' },
      { label: '中文(简体)', action: 'translate', payload: 'zh-CN' },
      { label: 'Español', action: 'translate', payload: 'es' },
      { label: 'Français', action: 'translate', payload: 'fr' },
      { label: 'Deutsch', action: 'translate', payload: 'de' },
    ]
  },
  { type: 'divider' },
  { label: '줌 인', action: 'menu-zoom-in', icon: '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><circle cx="7" cy="7" r="4"/><path d="M10 10l3.5 3.5"/><path d="M5 7h4M7 5v4"/></svg>' },
  { label: '줌 아웃', action: 'menu-zoom-out', icon: '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><circle cx="7" cy="7" r="4"/><path d="M10 10l3.5 3.5"/><path d="M5 7h4"/></svg>' },
  { label: '줌 초기화', action: 'menu-zoom-reset', icon: '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><circle cx="7" cy="7" r="4"/><path d="M10 10l3.5 3.5"/><path d="M6 6.5h2v2"/></svg>' },
  { type: 'divider' },
  { label: '스크린샷', action: 'menu-screenshot', icon: '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="12" height="10" rx="1.5"/><circle cx="8" cy="8.5" r="2.5"/><path d="M5.5 3L6.5 1.5h3L10.5 3"/></svg>' },
  { label: '개발자 도구', action: 'menu-devtools', icon: '<svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M5 4L1.5 8 5 12M11 4l3.5 4L11 12M9.5 2l-3 12"/></svg>' },
  { type: 'divider' },
  { type: 'version', label: 'SumPlayer v1.0.7' },
];

function openMenu() {
  showOverlayAndEmit('overlay-show-menu', () => {
    const btnRect = btnMenu.getBoundingClientRect();
    return {
      items: menuItems,
      x: Math.max(0, Math.round(btnRect.right) - 200),
      y: Math.round(btnRect.bottom),
    };
  });
}
function closeMenu() {
  api.hideMenuOverlay();
}

btnMenu.addEventListener('click', (e) => {
  e.stopPropagation();
  openMenu();
});

menuHome.addEventListener('click', async () => {
  closeMenu();
});
menuHistory.addEventListener('click', () => {
  closeMenu();
  showPanel('history');
});

// Homepage dialog
function hideHomepageDialog() {
  homepageDialog.classList.add('hidden');
  api.hideMenuOverlay();
}
homepageSaveBtn.addEventListener('click', async () => {
  const url = homepageUrlInput.value.trim();
  if (url) {
    await api.setHomePage(url);
  }
  hideHomepageDialog();
});
homepageCancelBtn.addEventListener('click', () => hideHomepageDialog());
homepageCurrentBtn.addEventListener('click', () => {
  if (currentUrl) {
    homepageUrlInput.value = currentUrl;
  }
});
homepageDialog.addEventListener('click', (e) => {
  if (e.target === homepageDialog) hideHomepageDialog();
});

// Translation submenu
translateSubmenu.addEventListener('click', (e) => {
  const target = e.target.closest('[data-lang]');
  if (target) {
    const lang = target.getAttribute('data-lang');
    closeMenu();
    api.translatePage(lang);
  }
});

// Panel events
panelClose.addEventListener('click', () => hidePanel());
panelOverlay.addEventListener('click', (e) => {
  if (e.target === panelOverlay) hidePanel();
});

panelSearch.addEventListener('input', async () => {
  if (currentPanel === 'history') {
    await loadHistory(panelSearch.value.trim() || undefined);
  }
});

panelClear.addEventListener('click', async () => {
  if (currentPanel === 'history') {
    await api.historyClear();
    await loadHistory();
  } else if (currentPanel === 'downloads') {
    await api.downloadClearHistory();
    await loadDownloads();
  }
});

// Find bar events
findInput.addEventListener('input', () => doFind(true));
findInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    doFind(!e.shiftKey);
  } else if (e.key === 'Escape') {
    hideFindBar();
  }
});
findNext.addEventListener('click', () => doFind(true));
findPrev.addEventListener('click', () => doFind(false));
findClose.addEventListener('click', () => hideFindBar());

// ─── Keyboard shortcuts ───

document.addEventListener('keydown', (e) => {
  const ctrl = e.ctrlKey || e.metaKey;

  if (ctrl && e.key === 't') { e.preventDefault(); api.createTab(); return; }
  if (ctrl && e.key === 'w') { e.preventDefault(); if (activeTabId) api.closeTab(activeTabId); return; }
  if (ctrl && e.shiftKey && e.key === 'T') { e.preventDefault(); api.restoreTab(); return; }
  if (ctrl && e.key === 'l') { e.preventDefault(); urlInput.focus(); urlInput.select(); return; }
  if (ctrl && e.key === 'Tab' && !e.shiftKey) { e.preventDefault(); api.nextTab(); return; }
  if (ctrl && e.shiftKey && e.key === 'Tab') { e.preventDefault(); api.prevTab(); return; }
  if (ctrl && e.key === 'f') { e.preventDefault(); showFindBar(); return; }
  if (ctrl && e.key === 'h') { e.preventDefault(); showPanel('history'); return; }
  if (ctrl && e.key === 'j') { e.preventDefault(); showPanel('downloads'); return; }
  if (ctrl && (e.key === '=' || e.key === '+')) { e.preventDefault(); api.zoomIn(); return; }
  if (ctrl && e.key === '-') { e.preventDefault(); api.zoomOut(); return; }
  if (ctrl && e.key === '0') { e.preventDefault(); api.zoomReset(); return; }
  if ((ctrl && e.shiftKey && e.key === 'I') || e.key === 'F12') { e.preventDefault(); api.toggleDevTools(); return; }
  if (e.key === 'F11') { e.preventDefault(); api.toggleFullScreen(); return; }
  if (e.key === 'F5' || (ctrl && e.key === 'r')) { e.preventDefault(); api.reload(); return; }
  if (e.altKey && e.key === 'ArrowLeft') { e.preventDefault(); api.goBack(); return; }
  if (e.altKey && e.key === 'ArrowRight') { e.preventDefault(); api.goForward(); return; }
  if (e.key === 'Escape') {
    if (!panelOverlay.classList.contains('hidden')) { hidePanel(); return; }
    if (!findBar.classList.contains('hidden')) { hideFindBar(); return; }
  }
});

// ─── Tauri event handlers ───
// Registered inside registerEventHandlers() after listen is confirmed available

const controlIndicator = document.getElementById('control-indicator');
let eventsRegistered = false;

function registerEventHandlers() {
  if (eventsRegistered) return;
  eventsRegistered = true;

  onEvent('tab-created', (data) => {
    // Avoid duplicate if tab element already exists (from init fallback)
    if (tabsContainer.querySelector(`[data-tab-id="${data.id}"]`)) {
      setActiveTab(data.id);
      return;
    }
    const tabEl = createTabElement(data.id, data.title);
    tabsContainer.appendChild(tabEl);
    setActiveTab(data.id);
  });

  onEvent('tab-closed', (data) => {
    const tabEl = tabsContainer.querySelector(`[data-tab-id="${data.id}"]`);
    if (tabEl) tabEl.remove();
  });

  // After delayed webview close, WebView2 may relayout - fix sizes
  onEvent('_internal_resize', () => {
    api.resizeTabs();
  });

  onEvent('tab-switched', (data) => {
    setActiveTab(data.id);
    urlInput.value = data.url || '';
    currentUrl = data.url || '';
    currentTitle = data.title || '';
    currentFavicon = data.favicon || '';
    updateBookmarkStar();
  });

  onEvent('tab-url-updated', (data) => {
    if (data.id === activeTabId) {
      urlInput.value = data.url;
      currentUrl = data.url;
      updateBookmarkStar();
    }
  });

  onEvent('tab-title-updated', (data) => {
    const tabEl = tabsContainer.querySelector(`[data-tab-id="${data.id}"]`);
    if (tabEl) {
      const titleSpan = tabEl.querySelector('.tab-title');
      if (titleSpan) titleSpan.textContent = data.title;
    }
    if (data.id === activeTabId) {
      currentTitle = data.title;
    }
  });

  onEvent('tab-favicon-updated', (data) => {
    const tabEl = tabsContainer.querySelector(`[data-tab-id="${data.id}"]`);
    if (tabEl) {
      const img = tabEl.querySelector('.tab-favicon');
      if (img && data.favicon) {
        img.src = data.favicon;
        img.style.display = '';
      }
    }
    if (data.id === activeTabId) {
      currentFavicon = data.favicon;
    }
  });

  onEvent('find-count-updated', (data) => {
    findTotalCount = data.count || 0;
    if (findTotalCount > 0) {
      findCurrentIndex = Math.max(1, Math.min(findCurrentIndex, findTotalCount));
      findMatches.textContent = `${findCurrentIndex}/${findTotalCount}`;
    } else {
      findCurrentIndex = 0;
      findMatches.textContent = '0/0';
    }
  });

  onEvent('zoom-changed', (data) => {
    if (data.zoom === 100) {
      zoomIndicator.classList.remove('visible');
      zoomIndicator.textContent = '';
    } else {
      zoomIndicator.classList.add('visible');
      zoomIndicator.textContent = `${data.zoom}%`;
    }
  });

  onEvent('tab-pinned', (data) => {
    const tabEl = tabsContainer.querySelector(`[data-tab-id="${data.id}"]`);
    if (!tabEl) return;
    if (data.pinned) {
      tabEl.classList.add('pinned');
      const closeBtn = tabEl.querySelector('.tab-close');
      if (closeBtn) closeBtn.classList.add('hidden');
      const firstUnpinned = tabsContainer.querySelector('.tab:not(.pinned)');
      if (firstUnpinned) {
        tabsContainer.insertBefore(tabEl, firstUnpinned);
      }
    } else {
      tabEl.classList.remove('pinned');
      const closeBtn = tabEl.querySelector('.tab-close');
      if (closeBtn) closeBtn.classList.remove('hidden');
    }
  });

  onEvent('tabs-reordered', (data) => {
    const { order } = data;
    for (const id of order) {
      const el = tabsContainer.querySelector(`[data-tab-id="${id}"]`);
      if (el) tabsContainer.appendChild(el);
    }
  });

  onEvent('ui-action', (data) => {
    const action = data.action;
    if (action === 'focus-url') { urlInput.focus(); urlInput.select(); }
    else if (action === 'find') { showFindBar(); }
    else if (action === 'history') { showPanel('history'); }
    else if (action === 'downloads') { showPanel('downloads'); }
    else if (action === 'toggle-menu') { openMenu(); }
  });

  // Menu overlay actions (from overlay webview)
  onEvent('menu-overlay-action', (data) => {
    const { action, payload } = data;
    if (action === 'menu-home') {
      // Show homepage dialog in overlay
      api.getHomePage().then(homeUrl => {
        showOverlayAndEmit('overlay-show-dialog', () => ({ type: 'homepage', homeUrl, currentUrl }));
      });
    } else if (action === 'menu-history') {
      showPanel('history');
    } else if (action === 'translate') {
      api.translatePage(payload);
    } else if (action === 'bm-open-new-tab') {
      api.createTab(payload);
    } else if (action === 'bm-delete') {
      api.bookmarkRemove(payload).then(() => {
        refreshBookmarkBar();
        updateBookmarkStar();
      });
    } else if (action === 'menu-downloads') {
      showPanel('downloads');
    } else if (action === 'menu-find') {
      showFindBar();
    } else if (action === 'menu-zoom-in') {
      api.zoomIn();
    } else if (action === 'menu-zoom-out') {
      api.zoomOut();
    } else if (action === 'menu-zoom-reset') {
      api.zoomReset();
    } else if (action === 'menu-screenshot') {
      api.captureScreenshot();
    } else if (action === 'menu-devtools') {
      api.toggleDevTools();
    } else if (action === 'save-homepage') {
      api.setHomePage(payload);
    } else if (action === 'navigate') {
      api.navigate(payload);
    }
  });

  // Auto-update: show confirm dialog when new version is available
  onEvent('update-available', (data) => {
    var existing = document.getElementById('update-dialog');
    if (existing) return;
    var overlay = document.createElement('div');
    overlay.id = 'update-dialog';
    overlay.style.cssText = 'position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.5);z-index:999999;display:flex;align-items:flex-start;justify-content:center;padding-top:80px;';
    var box = document.createElement('div');
    box.style.cssText = 'background:#2c2d32;border:1px solid rgba(255,255,255,0.1);border-radius:12px;padding:24px;width:360px;box-shadow:0 8px 32px rgba(0,0,0,0.4);color:#e4e5e9;font-size:14px;';
    box.innerHTML = '<div style="font-size:16px;font-weight:600;margin-bottom:12px;">새로운 버전 발견 (v' + data.version + ')</div>'
      + '<div style="color:#8b8d93;margin-bottom:20px;">프로그램을 종료하고 업데이트 후 재시작합니다.</div>'
      + '<div style="display:flex;justify-content:flex-end;gap:8px;">'
      + '<button id="update-cancel" style="height:34px;padding:0 16px;border-radius:8px;border:1px solid rgba(255,255,255,0.1);background:transparent;color:#e4e5e9;font-size:13px;cursor:pointer;">나중에</button>'
      + '<button id="update-ok" style="height:34px;padding:0 16px;border-radius:8px;border:none;background:#7c5cfc;color:#fff;font-size:13px;cursor:pointer;">확인</button>'
      + '</div>';
    overlay.appendChild(box);
    document.body.appendChild(overlay);
    document.getElementById('update-cancel').addEventListener('click', function() { overlay.remove(); });
    document.getElementById('update-ok').addEventListener('click', function() {
      box.innerHTML = '<div style="text-align:center;padding:10px;color:#8b8d93;">업데이트 설치 중...</div>';
      invoke('update_confirm').catch(function() { overlay.remove(); });
    });
  });

  onEvent('control-server-status', (data) => {
    if (data.active) {
      controlIndicator.classList.add('active');
      controlIndicator.title = '원격 제어 활성 (클릭하여 비활성화)';
    } else {
      controlIndicator.classList.remove('active');
      controlIndicator.title = '원격 제어 비활성 (클릭하여 활성화 시도)';
    }
  });
}
controlIndicator.addEventListener('click', () => {
  api.toggleControlServer();
});
// Check initial control server status
api.getControlServerStatus().then(active => {
  if (active) {
    controlIndicator.classList.add('active');
    controlIndicator.title = '원격 제어 활성 (클릭하여 비활성화)';
  }
}).catch(() => {});

// Bookmark bar drop on empty area
bookmarkItems.addEventListener('dragover', (e) => { e.preventDefault(); });
bookmarkItems.addEventListener('drop', async (e) => {
  const draggedId = e.dataTransfer.getData('text/bookmark-id');
  if (draggedId && e.target === bookmarkItems) {
    e.preventDefault();
    await api.bookmarkMove(draggedId, null);
    refreshBookmarkBar();
  }
});

// ─── Window resize → sync child webview size ───
window.addEventListener('resize', () => {
  api.resizeTabs();
});

// ─── Disable default context menu on main webview ───
document.addEventListener('contextmenu', (e) => {
  // Allow custom contextmenu handlers on bookmark items (they call e.stopPropagation)
  // Prevent the default WebView2 context menu everywhere else
  e.preventDefault();
});

// ─── Drag to resize title bar region ───
// Make title bar draggable for window moving
document.getElementById('title-bar').addEventListener('mousedown', (e) => {
  // Only drag on the drag region (not buttons)
  if (e.target.closest('[data-tauri-drag-region]') || e.target.id === 'title-bar' || e.target.id === 'tab-bar') {
    // Tauri handles this via data-tauri-drag-region attribute
  }
});

// ─── Report chrome height to backend ───

function reportChromeHeight() {
  const chrome = document.getElementById('browser-chrome');
  if (chrome && invoke) {
    const dpr = window.devicePixelRatio || 1;
    const height = Math.round(chrome.offsetHeight * dpr);
    api.setChromeHeight(height).catch(() => {});
  }
}

// ─── Initialize ───

async function init() {
  try {
    registerEventHandlers();
    await refreshBookmarkBar();
    setTimeout(reportChromeHeight, 100);
    await new Promise(r => setTimeout(r, 100));
    const tabId = await api.createTab();
    // Ensure the tab element exists (fallback if event was missed)
    if (tabId && !tabsContainer.querySelector(`[data-tab-id="${tabId}"]`)) {
      const tabEl = createTabElement(tabId, 'New Tab');
      tabsContainer.appendChild(tabEl);
      setActiveTab(tabId);
    }
  } catch (e) {
    console.error('[SumPlayer] Init error:', e);
  }
}

// Wait for Tauri API to be ready
if (invoke && listen) {
  init();
} else {
  let retries = 0;
  const waitForTauri = setInterval(() => {
    retries++;
    tryLoadTauriApi();
    if (invoke && listen) {
      clearInterval(waitForTauri);
      init();
    } else if (retries > 50) {
      clearInterval(waitForTauri);
      console.error('[SumPlayer] Tauri API not available after 5s');
    }
  }, 100);
}
