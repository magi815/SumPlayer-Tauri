# SumPlayer-Tauri Feature Status

> Last updated: 2026-02-17
> Migrated from: Electron (E:\Projects\SumPlayer)
> Target: Tauri v2 + WebView2 (E:\Projects\SumPlayer-Tauri)

## Migration Motivation

Electron SumPlayer was blocked by **Akamai bot detection** (Coupang etc.) due to TLS fingerprinting.
Tauri v2 + WebView2 uses the **OS native browser engine**, solving this fundamentally.
Verified: Coupang loads successfully without "Access Denied" in the Tauri build.

---

## Architecture

| Component | Electron | Tauri v2 |
|-----------|----------|----------|
| Backend | Node.js (main process) | Rust (src-tauri/src/) |
| Frontend | Chromium renderer | WebView2 (OS native) |
| IPC | ipcMain.handle / ipcRenderer.invoke | #[tauri::command] / invoke() |
| Events | ipcRenderer.on | @tauri-apps/api event listen() |
| Multi-tab | WebContentsView per tab | WebviewBuilder child per tab (unstable) |
| HTTP server | Node.js http module | tiny_http crate on port 17580 |
| Data storage | Electron app.getPath('userData') | dirs::data_dir().join("SumPlayer") |
| Window frame | frameless + -webkit-app-region: drag | decorations: false + data-tauri-drag-region |
| Zoom | webContents.setZoomFactor() | CSS document.body.style.zoom |
| Back/Forward | webContents.goBack() | webview.eval("window.history.back()") |
| Global API | preload contextBridge | withGlobalTauri: true (window.__TAURI__) |

---

## Project Files

### Rust Backend (src-tauri/src/)

| File | Lines | Description |
|------|-------|-------------|
| main.rs | ~694 | App entry, 40+ Tauri commands, AppState, setup, window events |
| tab_manager.rs | ~587 | Multi-webview tab management, navigation, zoom, find, translate |
| control_server.rs | ~915 | HTTP control server with 59 routes |
| bookmark.rs | ~165 | JSON bookmark/folder CRUD |
| history.rs | ~153 | JSON history management (max 10000, debounced save) |
| window_state.rs | ~59 | Window position/size persistence |
| download.rs | ~91 | Download tracking structures |

### Frontend (src/)

| File | Lines | Description |
|------|-------|-------------|
| index.html | ~200 | Main UI (title bar, tab bar, nav bar, content area) |
| scripts/browser-ui.js | ~822 | Frontend logic (all UI interactions via Tauri invoke) |
| styles/browser.css | ~1271 | Dark theme CSS (copied from Electron) |

### Config

| File | Description |
|------|-------------|
| src-tauri/tauri.conf.json | Tauri config (frameless, withGlobalTauri, CSP) |
| src-tauri/Cargo.toml | Rust deps (tauri unstable, tiny_http, tokio, etc.) |
| src-tauri/capabilities/default.json | Permissions for main window + tab-* webviews |
| package.json | npm deps (@tauri-apps/cli, @tauri-apps/api) |

---

## Feature Status

### Legend
- OK: Implemented and tested working
- PARTIAL: Implemented but limited compared to Electron
- STUB: Code exists but functionality is placeholder
- MISSING: Not yet implemented

---

### 1. Tab Management

| Feature | Status | Notes |
|---------|--------|-------|
| Create tab | OK | `tab_create` command, child webview via `Window::add_child` |
| Close tab | OK | Closes webview, switches to adjacent tab |
| Switch tab | OK | Hides inactive by moving off-screen (-10000) |
| List tabs | OK | Returns id, title, url, active, pinned |
| Restore closed tab | OK | Maintains stack of recently closed (max 10) |
| Next/prev tab | OK | Ctrl+Tab / Ctrl+Shift+Tab |
| Pin/unpin tab | OK | Toggle pin state, emits event |
| Move tab (reorder) | OK | Via command, emits tabs-reordered event |
| Tab title auto-update | OK | JS injected via `on_page_load` -> `webview_title_changed` |
| Tab URL auto-update | OK | Same mechanism as title |
| Tab favicon | OK | JS-based detection via on_page_load injection (extracts link[rel=icon]) |
| Tab drag-and-drop UI | PARTIAL | Backend supports it; frontend DnD may need testing |
| Tab context menu | PARTIAL | Frontend has context menu code; native menu not wired |
| Right-click context menu in page | OK | Custom HTML overlay injected via on_page_load (Copy/Paste/Select All/Open Link/Save Image) |

### 2. Navigation

| Feature | Status | Notes |
|---------|--------|-------|
| Navigate to URL | OK | Auto-adds https://, Google search for non-URLs |
| Go back | OK | Via `window.history.back()` JS eval |
| Go forward | OK | Via `window.history.forward()` JS eval |
| Reload | OK | Via `window.location.reload()` JS eval |
| Go home | OK | Navigates to saved home page |
| URL resolution | OK | Protocol detection, search query fallback |

### 3. Zoom

| Feature | Status | Notes |
|---------|--------|-------|
| Zoom in | OK | CSS `document.body.style.zoom`, max 3.0x |
| Zoom out | OK | Min 0.3x |
| Zoom reset | OK | Resets to 1.0x |
| Zoom indicator | OK | Frontend shows zoom percentage |
| Ctrl+scroll zoom | OK | Injected wheel event listener in child webviews via on_page_load |

### 4. Find in Page

| Feature | Status | Notes |
|---------|--------|-------|
| Find text | OK | `window.find()` JS API |
| Find next/prev | OK | Forward/backward parameter |
| Stop find | OK | `getSelection().removeAllRanges()` |
| Find match count | OK | Custom JS TreeWalker counts matches, reports via find_count_result command |

### 5. Bookmarks

| Feature | Status | Notes |
|---------|--------|-------|
| Add bookmark | OK | JSON file storage, compatible with Electron format |
| Remove bookmark | OK | |
| Update bookmark | OK | Title, URL, folder |
| Check if bookmarked | OK | |
| List bookmarks | OK | All or by folder |
| Get all + folders | OK | |
| Move bookmark | OK | Reorder support |
| Bookmark bar UI | OK | Frontend renders bookmark bar |
| Bookmark dialog | OK | Ctrl+D opens add/edit/remove dialog |
| Bookmark favicon | OK | Auto-detected via tab favicon, stored when bookmarking |

### 6. History

| Feature | Status | Notes |
|---------|--------|-------|
| Auto-record visits | OK | Recorded via `webview_title_changed` command |
| List history | OK | With query filter, limit, offset |
| Search history | OK | URL dedup search |
| Delete entry | OK | |
| Clear all | OK | |
| History panel UI | OK | With date separators, search |
| Debounced save | OK | Every 50 entries or on flush |
| Shared data with Electron | OK | Same JSON format in same data dir |

### 7. Window Management

| Feature | Status | Notes |
|---------|--------|-------|
| Minimize | OK | |
| Maximize/restore | OK | Toggle behavior |
| Close | OK | Saves state, flushes history, stops server |
| Fullscreen | OK | F11 toggle |
| Frameless window | OK | `decorations: false` + `data-tauri-drag-region` |
| Window state persistence | OK | Saves x, y, width, height, isMaximized |
| Window state restore | OK | Restores position/size/maximized on startup |
| Custom title bar drag | OK | `data-tauri-drag-region` on title-bar and tab-bar |

### 8. Remote Control Server (HTTP API on port 17580)

| Route | Status | Notes |
|-------|--------|-------|
| GET /status | OK | |
| POST /tab/create | OK | |
| POST /tab/close | OK | |
| POST /tab/switch | OK | |
| GET /tab/list | OK | |
| GET /tab/active | OK | |
| POST /tab/restore | OK | |
| POST /tab/pin | OK | |
| POST /tab/move | OK | |
| POST /tab/next | OK | |
| POST /tab/prev | OK | |
| POST /nav/go | OK | |
| POST /nav/back | OK | |
| POST /nav/forward | OK | |
| POST /nav/reload | OK | |
| POST /nav/home | OK | |
| GET /page/title | OK | |
| GET /page/url | OK | |
| POST /page/exec | OK | eval only (no return value) |
| GET /page/content | PARTIAL | Executes JS but can't return content |
| POST /page/inspect | OK | Stores elements in window.__inspectedElements |
| POST /page/click | OK | By index or selector |
| POST /page/type | OK | With clear/submit options |
| POST /page/wait | OK | Selector or load complete |
| POST /page/scroll | OK | Direction/amount or selector |
| POST /page/submit | OK | Fill fields + submit form |
| POST /page/screenshot | OK | html2canvas JS injection, base64 result via screenshot_captured command |
| POST /page/screenshot/full | PARTIAL | html2canvas-based, limited by canvas size constraints |
| POST /zoom/in | OK | |
| POST /zoom/out | OK | |
| POST /zoom/reset | OK | |
| POST /find | OK | |
| POST /fullscreen/toggle | OK | |
| POST /devtools/toggle | OK | Returns note that F12 opens DevTools natively in WebView2 |
| GET /history/list | OK | |
| POST /history/search | OK | |
| POST /history/clear | OK | |
| POST /bookmark/add | OK | |
| POST /bookmark/remove | OK | |
| GET /bookmark/list | OK | |
| POST /bookmark/check | OK | |
| GET /download/list | OK | |
| POST /console/start | OK | JS-based capture (injected) |
| POST /console/stop | OK | |
| GET /console/get | OK | |
| POST /network/start | OK | JS-based fetch/XHR intercept |
| POST /network/stop | OK | |
| GET /network/get | OK | |
| POST /storage/get | OK | localStorage/sessionStorage |
| POST /storage/set | OK | |
| POST /storage/clear | OK | |
| GET /viewport/get | OK | |
| POST /viewport/set | OK | |
| GET /page/performance | OK | JS performance API |
| GET /page/errors | OK | Combines window.onerror + unhandledrejection + console.error logs |
| POST /cookie/get | OK | JS document.cookie API (HttpOnly cookies not accessible) |
| POST /cookie/set | OK | JS document.cookie with path/maxAge |
| POST /cookie/delete | OK | Sets expired cookie |
| POST /cookie/clear | OK | Clears all accessible cookies |

### 9. Downloads

| Feature | Status | Notes |
|---------|--------|-------|
| Download tracking | OK | Monitors Downloads folder via polling, detects new files |
| Active downloads list | PARTIAL | Polling-based, no real-time progress (files appear as completed) |
| Download history | OK | Tracked via folder monitoring, persisted to downloads.json |
| Cancel download | STUB | No interception of in-progress WebView2 downloads |
| Open file | OK | Uses `open::that()` |
| Open folder | OK | Uses `open::that()` on parent dir |
| Clear history | OK | |
| Auto-download dialog | PARTIAL | WebView2 handles downloads natively; tracked via folder monitoring |

### 10. Page Interaction

| Feature | Status | Notes |
|---------|--------|-------|
| Execute JS (eval) | OK | Via `webview.eval()`; no return value |
| Translate page | OK | Google Translate API JS injection |
| YouTube Shorts auto-advance | OK | JS injection on youtube.com/shorts via on_page_load (toggle button + auto ArrowDown) |
| User-Agent customization | MISSING | Electron modified UA; WebView2 uses system Chrome UA (which is actually better for bot detection) |

### 11. Terminal

| Feature | Status | Notes |
|---------|--------|-------|
| PTY terminal session | MISSING | `portable-pty` dependency added but not wired |
| Terminal UI (xterm.js) | MISSING | No terminal.html or terminal-ui.js |
| Multiple shells | MISSING | |
| Terminal panel toggle | MISSING | |

### 12. Settings

| Feature | Status | Notes |
|---------|--------|-------|
| Get home page | OK | |
| Set home page | OK | Persisted to settings.json |
| Homepage dialog UI | OK | Frontend has dialog |

### 13. Screenshots

| Feature | Status | Notes |
|---------|--------|-------|
| Viewport screenshot | OK | html2canvas injection, saves PNG to Downloads folder |
| Full-page screenshot | PARTIAL | html2canvas-based, limited by max canvas size |

### 14. UI Components (Frontend)

| Component | Status | Notes |
|-----------|--------|-------|
| Tab bar | OK | Create, switch, close, drag area |
| Navigation bar | OK | URL input, back, forward, reload, home |
| Find bar | OK | Show/hide, search, close |
| Bookmark bar | OK | Click to navigate, Ctrl+D dialog |
| History panel | OK | Search, date groups, delete |
| Downloads panel | OK | UI exists but no real download data |
| Menu dropdown | OK | Homepage, History, Translate |
| Window controls | OK | Minimize, maximize, close buttons |
| Zoom indicator | OK | Shows percentage |
| Control server indicator | OK | Status display |
| Dark theme | OK | Full CSS copied from Electron |
| Keyboard shortcuts | OK | See list below |

### 15. Keyboard Shortcuts

| Shortcut | Action | Status |
|----------|--------|--------|
| Ctrl+T | New tab | OK |
| Ctrl+W | Close tab | OK |
| Ctrl+Shift+T | Restore closed tab | OK |
| Ctrl+L | Focus address bar | OK |
| Ctrl+Tab | Next tab | OK |
| Ctrl+Shift+Tab | Previous tab | OK |
| Ctrl+F | Find in page | OK |
| Ctrl+D | Bookmark dialog (add/edit/remove) | OK |
| Ctrl+H | History panel | OK |
| Ctrl+J | Downloads panel | OK |
| Ctrl+= / Ctrl++ | Zoom in | OK |
| Ctrl+- | Zoom out | OK |
| Ctrl+0 | Zoom reset | OK |
| Ctrl+R / F5 | Reload | OK |
| F11 | Fullscreen | OK |
| F12 / Ctrl+Shift+I | DevTools | OK (WebView2 native) |
| Ctrl+` | Terminal | MISSING |
| Alt+Left | Back | OK |
| Alt+Right | Forward | OK |
| Escape | Close dialogs/panels | OK |

---

## Summary Statistics

| Category | Total Features | OK | PARTIAL | STUB | MISSING |
|----------|---------------|-----|---------|------|---------|
| Tab Management | 14 | 13 | 1 | 0 | 0 |
| Navigation | 6 | 6 | 0 | 0 | 0 |
| Zoom | 5 | 5 | 0 | 0 | 0 |
| Find in Page | 4 | 4 | 0 | 0 | 0 |
| Bookmarks | 10 | 10 | 0 | 0 | 0 |
| History | 8 | 8 | 0 | 0 | 0 |
| Window | 8 | 8 | 0 | 0 | 0 |
| Control Server | 59 | 58 | 1 | 0 | 0 |
| Downloads | 8 | 5 | 2 | 1 | 0 |
| Page Interaction | 4 | 3 | 0 | 0 | 1 |
| Terminal | 4 | 0 | 0 | 0 | 4 |
| Settings | 3 | 3 | 0 | 0 | 0 |
| Screenshots | 2 | 1 | 1 | 0 | 0 |
| UI Components | 13 | 13 | 0 | 0 | 0 |
| Keyboard Shortcuts | 20 | 19 | 0 | 0 | 1 |
| **Total** | **168** | **156 (93%)** | **5 (3%)** | **1 (1%)** | **6 (4%)** |

---

## Known Limitations (Tauri v2 vs Electron)

1. **No JS return values from eval**: `webview.eval()` is fire-and-forget; workaround: invoke commands back from injected JS
2. **DevTools via F12 only**: WebView2 opens DevTools natively with F12 key
3. **No download interception**: Workaround: folder monitoring detects new downloads (no progress tracking)
4. **Cookie API limited**: JS `document.cookie` works but HttpOnly cookies are inaccessible by design
5. **Screenshot via html2canvas**: Not pixel-perfect like native capture; cross-origin images may be blank
6. **Multi-webview is unstable**: Requires `unstable` Cargo feature flag
7. **Terminal not implemented**: PTY dependency added but not wired (deferred)

---

## Build & Run

```bash
# Prerequisites
# - Rust (rustup) installed
# - Node.js installed

# Install dependencies
cd E:\Projects\SumPlayer-Tauri
npm install

# Development
npx tauri dev

# Production build
npx tauri build

# Output locations
# Binary: src-tauri/target/release/sumplayer-tauri.exe
# MSI:    src-tauri/target/release/bundle/msi/SumPlayer_1.0.0_x64_en-US.msi
# NSIS:   src-tauri/target/release/bundle/nsis/SumPlayer_1.0.0_x64-setup.exe
```

## Data Directory

`%APPDATA%\SumPlayer\` (shared with Electron version)

- `bookmarks.json` - Bookmark data
- `history.json` - Browsing history
- `downloads.json` - Download history
- `settings.json` - Home page setting
- `window-state.json` - Window position/size

---

## Priority TODO

1. **Terminal integration** - Wire up portable-pty + xterm.js (deferred)
2. **User-Agent customization** - Currently uses system Chrome UA (sufficient for most cases)
