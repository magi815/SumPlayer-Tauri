// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bookmark;
mod control_server;
mod download;
mod history;
mod tab_manager;
mod window_state;

use tauri_plugin_updater::UpdaterExt;

use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

use bookmark::BookmarkManager;
use control_server::ControlServer;
use download::DownloadManager;
use history::HistoryManager;
use tab_manager::TabManager;
use window_state::WindowStateManager;

pub struct AppState {
    pub tab_manager: Arc<Mutex<TabManager>>,
    pub history_manager: Arc<Mutex<HistoryManager>>,
    pub bookmark_manager: Arc<Mutex<BookmarkManager>>,
    pub download_manager: Arc<Mutex<DownloadManager>>,
    pub window_state_manager: Arc<Mutex<WindowStateManager>>,
    pub control_server: Arc<Mutex<ControlServer>>,
}

// ─── Tab Commands ───

#[tauri::command]
async fn tab_create(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    url: Option<String>,
) -> Result<u32, String> {
    let mut tm = state.tab_manager.lock().await;
    let id = tm.create_tab(&app, url).await.map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
async fn tab_close(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: u32,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    let no_tabs = tm.close_tab(&app, id);
    // If all tabs closed, create a new one so the window isn't empty
    if no_tabs {
        let _ = tm.create_tab(&app, None).await;
    }
    Ok(())
}

#[tauri::command]
async fn tab_switch(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: u32,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.switch_tab(&app, id);
    Ok(())
}

#[tauri::command]
async fn tab_list(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let tm = state.tab_manager.lock().await;
    Ok(tm.list_tabs())
}

#[tauri::command]
async fn tab_restore(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Option<u32>, String> {
    let mut tm = state.tab_manager.lock().await;
    let id = tm.restore_closed_tab(&app).await.map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
async fn tab_next(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.switch_to_next_tab(&app);
    Ok(())
}

#[tauri::command]
async fn tab_prev(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.switch_to_prev_tab(&app);
    Ok(())
}

#[tauri::command]
async fn tab_pin(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: u32,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.toggle_pin_tab(&app, id);
    Ok(())
}

#[tauri::command]
async fn tab_move(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    tab_id: u32,
    before_tab_id: Option<u32>,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.move_tab(&app, tab_id, before_tab_id);
    Ok(())
}

// ─── Navigation Commands ───

#[tauri::command]
async fn nav_go(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.navigate_to(&app, &url).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn nav_back(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let tm = state.tab_manager.lock().await;
    tm.go_back(&app);
    Ok(())
}

#[tauri::command]
async fn nav_forward(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let tm = state.tab_manager.lock().await;
    tm.go_forward(&app);
    Ok(())
}

#[tauri::command]
async fn nav_reload(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let tm = state.tab_manager.lock().await;
    tm.reload(&app);
    Ok(())
}

#[tauri::command]
async fn nav_home(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.go_home(&app).await.map_err(|e| e.to_string())
}

// ─── Zoom Commands ───

#[tauri::command]
async fn zoom_in(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.zoom_in(&app);
    Ok(())
}

#[tauri::command]
async fn zoom_out(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.zoom_out(&app);
    Ok(())
}

#[tauri::command]
async fn zoom_reset(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.zoom_reset(&app);
    Ok(())
}

// ─── Find Commands ───

#[tauri::command]
async fn find_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    text: String,
    forward: bool,
) -> Result<(), String> {
    let tm = state.tab_manager.lock().await;
    tm.find_in_page(&app, &text, forward);
    Ok(())
}

#[tauri::command]
async fn find_stop(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let tm = state.tab_manager.lock().await;
    tm.stop_find(&app);
    Ok(())
}

// ─── Bookmark Commands ───

#[tauri::command]
async fn bookmark_add(
    state: tauri::State<'_, AppState>,
    url: String,
    title: String,
    folder_id: Option<String>,
    favicon: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut bm = state.bookmark_manager.lock().await;
    let bookmark = bm.add_bookmark(&url, &title, folder_id.as_deref(), favicon.as_deref());
    Ok(serde_json::to_value(bookmark).map_err(|e| e.to_string())?)
}

#[tauri::command]
async fn bookmark_remove(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let mut bm = state.bookmark_manager.lock().await;
    bm.remove_bookmark(&id);
    Ok(())
}

#[tauri::command]
async fn bookmark_update(
    state: tauri::State<'_, AppState>,
    id: String,
    title: Option<String>,
    url: Option<String>,
    folder_id: Option<String>,
) -> Result<(), String> {
    let mut bm = state.bookmark_manager.lock().await;
    bm.update_bookmark(&id, title.as_deref(), url.as_deref(), folder_id.as_deref());
    Ok(())
}

#[tauri::command]
async fn bookmark_check(
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<Option<serde_json::Value>, String> {
    let bm = state.bookmark_manager.lock().await;
    match bm.is_bookmarked(&url) {
        Some(b) => Ok(Some(serde_json::to_value(b).map_err(|e| e.to_string())?)),
        None => Ok(None),
    }
}

#[tauri::command]
async fn bookmark_list(
    state: tauri::State<'_, AppState>,
    folder_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let bm = state.bookmark_manager.lock().await;
    let list = bm.get_bookmarks(folder_id.as_deref().unwrap_or(""));
    Ok(list
        .into_iter()
        .map(|b| serde_json::to_value(b).unwrap())
        .collect())
}

#[tauri::command]
async fn bookmark_all(
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let bm = state.bookmark_manager.lock().await;
    let bookmarks = bm.get_all_bookmarks();
    let folders = bm.get_all_folders();
    Ok(serde_json::json!({
        "bookmarks": bookmarks,
        "folders": folders,
    }))
}

#[tauri::command]
async fn bookmark_move(
    state: tauri::State<'_, AppState>,
    bookmark_id: String,
    before_bookmark_id: Option<String>,
) -> Result<(), String> {
    let mut bm = state.bookmark_manager.lock().await;
    bm.move_bookmark(&bookmark_id, before_bookmark_id.as_deref());
    Ok(())
}

// ─── History Commands ───

#[tauri::command]
async fn history_list(
    state: tauri::State<'_, AppState>,
    query: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<serde_json::Value>, String> {
    let hm = state.history_manager.lock().await;
    let entries = hm.get_entries(query.as_deref(), limit.unwrap_or(200), offset.unwrap_or(0));
    Ok(entries
        .into_iter()
        .map(|e| serde_json::to_value(e).unwrap())
        .collect())
}

#[tauri::command]
async fn history_delete(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let mut hm = state.history_manager.lock().await;
    hm.delete_entry(&id);
    Ok(())
}

#[tauri::command]
async fn history_clear(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut hm = state.history_manager.lock().await;
    hm.clear_all();
    Ok(())
}

#[tauri::command]
async fn history_search(
    state: tauri::State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<serde_json::Value>, String> {
    let hm = state.history_manager.lock().await;
    let entries = hm.search(&query, limit.unwrap_or(8));
    Ok(entries
        .into_iter()
        .map(|e| serde_json::to_value(e).unwrap())
        .collect())
}

// ─── Settings Commands ───

#[tauri::command]
async fn settings_get_home_page(
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let tm = state.tab_manager.lock().await;
    Ok(tm.get_home_page())
}

#[tauri::command]
async fn settings_set_home_page(
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.set_home_page(&url);
    Ok(())
}

// ─── Window Commands ───

#[tauri::command]
async fn window_minimize(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.minimize().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn window_maximize(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_maximized().unwrap_or(false) {
            win.unmaximize().map_err(|e| e.to_string())?;
        } else {
            win.maximize().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
async fn window_close(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    // Save state before closing
    if let Some(win) = app.get_webview_window("main") {
        let wsm = state.window_state_manager.lock().await;
        if let Ok(pos) = win.outer_position() {
            if let Ok(size) = win.outer_size() {
                let is_maximized = win.is_maximized().unwrap_or(false);
                wsm.save(pos.x, pos.y, size.width, size.height, is_maximized);
            }
        }
        drop(wsm);
        let hm = state.history_manager.lock().await;
        hm.flush_save();
        drop(hm);
        let mut cs = state.control_server.lock().await;
        cs.stop();
        drop(cs);
        win.close().map_err(|e| e.to_string())?;
    }
    // Ensure the process exits
    app.exit(0);
    Ok(())
}

#[tauri::command]
async fn window_fullscreen(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        let is_fullscreen = win.is_fullscreen().unwrap_or(false);
        win.set_fullscreen(!is_fullscreen)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ─── Control Server Commands ───

#[tauri::command]
async fn control_server_toggle(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    use tauri::Emitter;
    let mut cs = state.control_server.lock().await;
    let active = if cs.is_running() {
        cs.stop();
        false
    } else {
        let tab_mgr = state.tab_manager.clone();
        let history_mgr = state.history_manager.clone();
        let bookmark_mgr = state.bookmark_manager.clone();
        let download_mgr = state.download_manager.clone();
        cs.start(app.clone(), tab_mgr, history_mgr, bookmark_mgr, download_mgr)
    };
    let _ = app.emit("control-server-status", serde_json::json!({ "active": active }));
    Ok(active)
}

#[tauri::command]
async fn control_server_status(
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let cs = state.control_server.lock().await;
    Ok(cs.is_running())
}

// ─── Download Commands ───

#[tauri::command]
async fn download_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let dm = state.download_manager.lock().await;
    Ok(dm
        .get_active_downloads()
        .into_iter()
        .map(|d| serde_json::to_value(d).unwrap())
        .collect())
}

#[tauri::command]
async fn download_history(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let dm = state.download_manager.lock().await;
    Ok(dm
        .get_history()
        .into_iter()
        .map(|d| serde_json::to_value(d).unwrap())
        .collect())
}

#[tauri::command]
async fn download_cancel(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let mut dm = state.download_manager.lock().await;
    dm.cancel_download(&id);
    Ok(())
}

#[tauri::command]
async fn download_open(path: String) -> Result<(), String> {
    open::that(&path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn download_open_folder(path: String) -> Result<(), String> {
    let parent = std::path::Path::new(&path)
        .parent()
        .unwrap_or(std::path::Path::new("."));
    open::that(parent).map_err(|e| e.to_string())
}

#[tauri::command]
async fn download_clear_history(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut dm = state.download_manager.lock().await;
    dm.clear_history();
    Ok(())
}

// ─── Webview Event Commands (called from injected JS) ───

#[tauri::command]
async fn webview_title_changed(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    tab_id: u32,
    title: String,
    url: String,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    let old_url = tm.get_active_tab().map(|t| t.url.clone()).unwrap_or_default();
    tm.update_tab_title(tab_id, &title);
    tm.update_tab_url(tab_id, &url);

    // Emit to frontend
    use tauri::Emitter;
    let _ = app.emit("tab-title-updated", serde_json::json!({
        "id": tab_id,
        "title": &title,
        "url": &url
    }));

    // Record in history if URL changed and is valid
    if !url.is_empty() && url != "about:blank" && url != old_url {
        let mut hm = state.history_manager.lock().await;
        hm.add_entry(&url, &title);
    }

    Ok(())
}

// ─── Page Commands (JS eval via WebView) ───

#[tauri::command]
async fn page_exec(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    code: String,
) -> Result<serde_json::Value, String> {
    let tm = state.tab_manager.lock().await;
    tm.execute_js(&app, &code).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn page_translate(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    target_lang: String,
) -> Result<(), String> {
    let tm = state.tab_manager.lock().await;
    tm.translate_page(&app, &target_lang).await.map_err(|e| e.to_string())
}

// ─── Find count result (called from injected JS) ───

#[tauri::command]
async fn find_count_result(
    app: tauri::AppHandle,
    count: u32,
) -> Result<(), String> {
    use tauri::Emitter;
    let _ = app.emit("find-count-updated", serde_json::json!({ "count": count }));
    Ok(())
}

// ─── Chrome height & Favicon Commands ───

#[tauri::command]
async fn set_chrome_height(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    height: u32,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.set_chrome_height(height);
    tm.resize_active_tab(&app);
    Ok(())
}

#[tauri::command]
async fn tab_favicon_changed(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    tab_id: u32,
    favicon: String,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.update_tab_favicon(tab_id, &favicon);
    use tauri::Emitter;
    let _ = app.emit("tab-favicon-updated", serde_json::json!({
        "id": tab_id,
        "favicon": &favicon
    }));
    Ok(())
}

#[tauri::command]
async fn resize_tabs(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.resize_active_tab(&app);
    Ok(())
}

#[tauri::command]
async fn set_tab_offset(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    extra_top: u32,
) -> Result<(), String> {
    let mut tm = state.tab_manager.lock().await;
    tm.set_tab_offset(&app, extra_top);
    Ok(())
}

#[tauri::command]
async fn show_menu_overlay(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let tm = state.tab_manager.lock().await;
    let size = tm.get_last_window_size().unwrap_or(tauri::PhysicalSize::new(1280, 800));
    tm.show_menu_overlay(&app, size.width, size.height);
    Ok(())
}

#[tauri::command]
async fn hide_menu_overlay(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let tm = state.tab_manager.lock().await;
    tm.hide_menu_overlay(&app);
    use tauri::Emitter;
    let _ = app.emit("menu-overlay-closed", serde_json::json!({}));
    Ok(())
}

#[tauri::command]
async fn menu_overlay_action(
    app: tauri::AppHandle,
    action: String,
    payload: String,
) -> Result<(), String> {
    use tauri::Emitter;
    let _ = app.emit("menu-overlay-action", serde_json::json!({ "action": action, "payload": payload }));
    // Close the overlay after action
    let state = app.state::<AppState>();
    let tm = state.tab_manager.lock().await;
    tm.hide_menu_overlay(&app);
    Ok(())
}

#[tauri::command]
async fn screenshot_captured(data: String) -> Result<(), String> {
    // Save base64 PNG data to Downloads folder
    if let Some(downloads) = dirs::download_dir() {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let filename = format!("screenshot_{}.png", timestamp);
        let filepath = downloads.join(&filename);
        // Strip data URL prefix if present
        let b64 = if let Some(pos) = data.find(",") {
            &data[pos + 1..]
        } else {
            &data
        };
        use base64::Engine;
        match base64::engine::general_purpose::STANDARD.decode(b64) {
            Ok(bytes) => {
                std::fs::write(&filepath, bytes).map_err(|e| e.to_string())?;
                // Open the file
                let _ = open::that(&filepath);
                Ok(())
            }
            Err(e) => Err(format!("Base64 decode error: {}", e)),
        }
    } else {
        Err("Could not find downloads directory".to_string())
    }
}

// ─── CDP helper ───

async fn cdp_call_async(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, AppState>,
    method: &str,
    params: &str,
) -> Result<String, String> {
    let tm = state.tab_manager.lock().await;
    let webview = tm.get_active_webview(app).ok_or("No active tab")?;
    drop(tm);

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
    let m = method.to_string();
    let p = params.to_string();

    webview.with_webview(move |platform_webview| {
        use webview2_com::Microsoft::Web::WebView2::Win32::*;
        use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
        use windows::core::HSTRING;
        unsafe {
            let controller = platform_webview.controller();
            let core: ICoreWebView2 = controller.CoreWebView2().unwrap();
            let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(
                move |_r: windows::core::Result<()>, json: String| { let _ = tx.send(Ok(json)); Ok(()) },
            ));
            let _ = core.CallDevToolsProtocolMethod(&HSTRING::from(m.as_str()), &HSTRING::from(p.as_str()), &handler);
        }
    }).map_err(|e| format!("with_webview error: {:?}", e))?;

    tokio::time::timeout(std::time::Duration::from_secs(10), rx)
        .await
        .map_err(|_| "CDP timeout".to_string())?
        .map_err(|_| "CDP channel error".to_string())?
}

// ─── Full-page Screenshot Command ───
// Uses CDP via WebView2 with Emulation.setDeviceMetricsOverride to avoid responsive reflow
// Steps:
//   1. Get current viewport width + full page height
//   2. Override viewport to (same width, full height) — no responsive breakpoint change
//   3. Capture screenshot (entire page now fits in viewport)
//   4. Clear the override to restore original viewport

#[tauri::command]
async fn capture_full_screenshot(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Step 1: Get page dimensions
    let dim_json = cdp_call_async(&app, &state, "Runtime.evaluate",
        r#"{"expression":"JSON.stringify({w:window.innerWidth,h:Math.max(document.body.scrollHeight,document.documentElement.scrollHeight,document.body.offsetHeight,document.documentElement.offsetHeight),dpr:window.devicePixelRatio})","returnByValue":true}"#
    ).await?;

    let dim_resp: serde_json::Value = serde_json::from_str(&dim_json)
        .map_err(|e| format!("JSON parse error: {}", e))?;
    let dim_str = dim_resp["result"]["value"].as_str()
        .ok_or("No dimension value")?;
    let dims: serde_json::Value = serde_json::from_str(dim_str)
        .map_err(|e| format!("Dim parse error: {}", e))?;

    let css_width = dims["w"].as_f64().unwrap_or(1280.0) as i64;
    let css_height = dims["h"].as_f64().unwrap_or(800.0) as i64;
    let dpr = dims["dpr"].as_f64().unwrap_or(1.0);

    // Step 2: Override viewport to full page height (width stays same → no responsive break)
    let override_params = format!(
        r#"{{"width":{},"height":{},"deviceScaleFactor":{},"mobile":false}}"#,
        css_width, css_height, dpr
    );
    let _ = cdp_call_async(&app, &state, "Emulation.setDeviceMetricsOverride", &override_params).await;

    // Wait for the page to re-render at the new viewport height
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Step 3: Capture screenshot (no captureBeyondViewport needed — page fits in viewport)
    let capture_result = cdp_call_async(&app, &state, "Page.captureScreenshot",
        r#"{"format":"png"}"#
    ).await;

    // Step 4: Clear override immediately to restore original viewport
    let _ = cdp_call_async(&app, &state, "Emulation.clearDeviceMetricsOverride", "{}").await;

    // Process the screenshot result
    let json_str = capture_result?;
    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("JSON parse error: {}", e))?;
    let b64 = parsed["data"].as_str()
        .ok_or("No 'data' field in CDP response")?;

    use base64::Engine;
    let png_data = base64::engine::general_purpose::STANDARD.decode(b64)
        .map_err(|e| format!("Base64 decode error: {}", e))?;

    if let Some(downloads) = dirs::download_dir() {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let filename = format!("screenshot_{}.png", timestamp);
        let filepath = downloads.join(&filename);
        std::fs::write(&filepath, &png_data).map_err(|e| e.to_string())?;
        let _ = open::that(&filepath);
        Ok(())
    } else {
        Err("Could not find downloads directory".to_string())
    }
}

// ─── DevTools Command ───

#[tauri::command]
async fn open_devtools(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let tm = state.tab_manager.lock().await;
    tm.open_devtools(&app);
    Ok(())
}

// ─── UI action forwarding (from child webview shortcuts) ───

#[tauri::command]
async fn ui_action(app: tauri::AppHandle, action: String) -> Result<(), String> {
    use tauri::Emitter;
    // For actions that need keyboard input on the main webview, set focus to it
    if action == "focus-url" || action == "find" {
        if let Some(webview) = app.get_webview("main") {
            let _ = webview.set_focus();
        }
    }
    let _ = app.emit("ui-action", serde_json::json!({ "action": action }));
    Ok(())
}

fn main() {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("SumPlayer");
    std::fs::create_dir_all(&data_dir).ok();

    let tab_manager = Arc::new(Mutex::new(TabManager::new(data_dir.clone())));
    let history_manager = Arc::new(Mutex::new(HistoryManager::new(data_dir.clone())));
    let bookmark_manager = Arc::new(Mutex::new(BookmarkManager::new(data_dir.clone())));
    let download_manager = Arc::new(Mutex::new(DownloadManager::new(data_dir.clone())));
    let window_state_manager = Arc::new(Mutex::new(WindowStateManager::new(data_dir.clone())));
    let control_server = Arc::new(Mutex::new(ControlServer::new()));

    // Start control server automatically
    let cs = control_server.clone();
    let tm_for_cs = tab_manager.clone();
    let hm_for_cs = history_manager.clone();
    let bm_for_cs = bookmark_manager.clone();
    let dm_for_cs = download_manager.clone();
    let wsm_for_setup = window_state_manager.clone();

    let app_state = AppState {
        tab_manager,
        history_manager,
        bookmark_manager,
        download_manager,
        window_state_manager,
        control_server,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            // Tabs
            tab_create,
            tab_close,
            tab_switch,
            tab_list,
            tab_restore,
            tab_next,
            tab_prev,
            tab_pin,
            tab_move,
            // Navigation
            nav_go,
            nav_back,
            nav_forward,
            nav_reload,
            nav_home,
            // Zoom
            zoom_in,
            zoom_out,
            zoom_reset,
            // Find
            find_start,
            find_stop,
            // Bookmarks
            bookmark_add,
            bookmark_remove,
            bookmark_update,
            bookmark_check,
            bookmark_list,
            bookmark_all,
            bookmark_move,
            // History
            history_list,
            history_delete,
            history_clear,
            history_search,
            // Settings
            settings_get_home_page,
            settings_set_home_page,
            // Window
            window_minimize,
            window_maximize,
            window_close,
            window_fullscreen,
            // Control server
            control_server_toggle,
            control_server_status,
            // Downloads
            download_list,
            download_history,
            download_cancel,
            download_open,
            download_open_folder,
            download_clear_history,
            // Page
            page_exec,
            page_translate,
            // Webview events
            webview_title_changed,
            tab_favicon_changed,
            // Find count
            find_count_result,
            // Chrome height & resize
            set_chrome_height,
            resize_tabs,
            set_tab_offset,
            show_menu_overlay,
            hide_menu_overlay,
            menu_overlay_action,
            // Screenshot
            screenshot_captured,
            capture_full_screenshot,
            // DevTools
            open_devtools,
            // UI action forwarding
            ui_action,
        ])
        .setup(move |app| {
            // Restore window state from previous session
            let wsm = wsm_for_setup.clone();
            if let Some(win) = app.get_webview_window("main") {
                let state = wsm.blocking_lock().load();
                if state.is_maximized {
                    if state.width > 0 && state.height > 0 {
                        let _ = win.set_size(tauri::PhysicalSize::new(state.width, state.height));
                    }
                    let _ = win.maximize();
                } else if state.width > 0 && state.height > 0 {
                    // Get available monitor area to clamp window position
                    let monitors: Vec<_> = win.available_monitors()
                        .unwrap_or_default()
                        .into_iter()
                        .collect();
                    let mut wx = state.x;
                    let mut wy = state.y;
                    let mut ww = state.width;
                    let mut wh = state.height;

                    if !monitors.is_empty() {
                        // Check if window is visible on any monitor
                        let visible = monitors.iter().any(|m| {
                            let mp = m.position();
                            let ms = m.size();
                            let mx = mp.x;
                            let my = mp.y;
                            let mw = ms.width as i32;
                            let mh = ms.height as i32;
                            // At least 100px of the window must be within the monitor
                            wx < mx + mw - 100
                                && wx + ww as i32 > mx + 100
                                && wy < my + mh - 100
                                && wy + wh as i32 > my + 100
                        });

                        if !visible {
                            // Move to primary monitor (first one) with margin
                            let m = &monitors[0];
                            let mp = m.position();
                            let ms = m.size();
                            // Clamp size to monitor
                            if ww > ms.width { ww = ms.width; }
                            if wh > ms.height { wh = ms.height; }
                            // Center on monitor
                            wx = mp.x + (ms.width as i32 - ww as i32) / 2;
                            wy = mp.y + (ms.height as i32 - wh as i32) / 2;
                        }
                    }

                    let _ = win.set_size(tauri::PhysicalSize::new(ww, wh));
                    if wx >= 0 || wy >= 0 {
                        let _ = win.set_position(tauri::PhysicalPosition::new(wx, wy));
                    }
                }
            }

            // Auto-check for updates and install if available
            let update_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                let _ = update_handle.emit("update-status", serde_json::json!({"status": "checking"}));
                match update_handle.updater() {
                    Ok(updater) => {
                        match updater.check().await {
                            Ok(Some(update)) => {
                                let version = update.version.clone();
                                let _ = update_handle.emit("update-status", serde_json::json!({"status": "downloading", "version": &version}));
                                match update.download_and_install(|_, _| {}, || {}).await {
                                    Ok(_) => {
                                        let _ = update_handle.emit("update-status", serde_json::json!({"status": "installed", "version": &version}));
                                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                                        update_handle.restart();
                                    }
                                    Err(e) => {
                                        let _ = update_handle.emit("update-status", serde_json::json!({"status": "error", "message": format!("{}", e)}));
                                    }
                                }
                            }
                            Ok(None) => {
                                let _ = update_handle.emit("update-status", serde_json::json!({"status": "up-to-date"}));
                            }
                            Err(e) => {
                                let _ = update_handle.emit("update-status", serde_json::json!({"status": "error", "message": format!("{}", e)}));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = update_handle.emit("update-status", serde_json::json!({"status": "error", "message": format!("Updater init failed: {}", e)}));
                    }
                }
            });

            // Auto-start control server
            let cs = cs.clone();
            let tm = tm_for_cs.clone();
            let hm = hm_for_cs.clone();
            let bm = bm_for_cs.clone();
            let dm = dm_for_cs.clone();
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut server = cs.lock().await;
                let started = server.start(app_handle.clone(), tm, hm, bm, dm);
                use tauri::Emitter;
                let _ = app_handle.emit("control-server-status", serde_json::json!({ "active": started }));
            });

            // Download folder polling
            let dm_poll = app.state::<AppState>().download_manager.clone();
            let app_for_dl = app.handle().clone();
            std::thread::spawn(move || {
                let downloads_dir = dirs::download_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
                let mut known_files: std::collections::HashSet<String> = std::collections::HashSet::new();
                // Initialize with existing files
                if let Ok(entries) = std::fs::read_dir(&downloads_dir) {
                    for entry in entries.flatten() {
                        known_files.insert(entry.path().to_string_lossy().to_string());
                    }
                }
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    if let Ok(entries) = std::fs::read_dir(&downloads_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path().to_string_lossy().to_string();
                            if !known_files.contains(&path) {
                                known_files.insert(path.clone());
                                // New file detected
                                if let Ok(meta) = entry.metadata() {
                                    if meta.is_file() {
                                        let filename = entry.file_name().to_string_lossy().to_string();
                                        // Skip temp/partial files
                                        if filename.ends_with(".crdownload") || filename.ends_with(".tmp") || filename.ends_with(".part") {
                                            continue;
                                        }
                                        let size = meta.len();
                                        let rt = tokio::runtime::Builder::new_current_thread()
                                            .enable_all()
                                            .build();
                                        if let Ok(rt) = rt {
                                            let dm = dm_poll.clone();
                                            let app = app_for_dl.clone();
                                            rt.block_on(async {
                                                let mut dm = dm.lock().await;
                                                dm.add_completed_download(&filename, &path, size);
                                                use tauri::Emitter;
                                                let _ = app.emit("download-completed", serde_json::json!({
                                                    "filename": &filename,
                                                    "path": &path,
                                                    "size": size
                                                }));
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
            tauri::WindowEvent::Resized(size) => {
                let app = window.app_handle().clone();
                let new_size = *size;
                // Resize immediately with the event-provided size
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<AppState>();
                    let mut tm = state.tab_manager.lock().await;
                    tm.resize_active_tab_with_size(&app, new_size);
                });
                // Also resize again after a short delay for maximize/restore transitions
                let app2 = window.app_handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    tauri::async_runtime::block_on(async {
                        let state = app2.state::<AppState>();
                        let mut tm = state.tab_manager.lock().await;
                        tm.resize_active_tab(&app2);
                    });
                });
            }
            tauri::WindowEvent::CloseRequested { .. } => {
                // Save window state directly from the window parameter
                let state = window.app_handle().state::<AppState>();
                let wsm = state.window_state_manager.blocking_lock();
                if let Ok(pos) = window.outer_position() {
                    if let Ok(size) = window.outer_size() {
                        let is_maximized = window.is_maximized().unwrap_or(false);
                        eprintln!("[WindowState] Saving: x={} y={} w={} h={} max={}",
                            pos.x, pos.y, size.width, size.height, is_maximized);
                        wsm.save(
                            pos.x,
                            pos.y,
                            size.width,
                            size.height,
                            is_maximized,
                        );
                    }
                }
                drop(wsm);
                // Flush history
                let hm = state.history_manager.blocking_lock();
                hm.flush_save();
                drop(hm);
                // Stop control server
                let mut cs = state.control_server.blocking_lock();
                cs.stop();
            }
            _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
