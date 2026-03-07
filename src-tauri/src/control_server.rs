use std::sync::Arc;
use tokio::sync::Mutex;
use crate::bookmark::BookmarkManager;
use crate::download::DownloadManager;
use crate::history::HistoryManager;
use crate::tab_manager::TabManager;

pub struct ControlServer {
    running: bool,
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
}

impl ControlServer {
    pub fn new() -> Self {
        ControlServer {
            running: false,
            stop_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn start(
        &mut self,
        app_handle: tauri::AppHandle,
        tab_manager: Arc<Mutex<TabManager>>,
        history_manager: Arc<Mutex<HistoryManager>>,
        bookmark_manager: Arc<Mutex<BookmarkManager>>,
        download_manager: Arc<Mutex<DownloadManager>>,
    ) -> bool {
        if self.running {
            return true;
        }

        self.stop_flag
            .store(false, std::sync::atomic::Ordering::SeqCst);

        let stop_flag = self.stop_flag.clone();

        let server = match tiny_http::Server::http("127.0.0.1:17580") {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[ControlServer] Failed to bind port 17580: {}", e);
                return false;
            }
        };

        println!("[ControlServer] Listening on http://127.0.0.1:17580");
        self.running = true;

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            loop {
                if stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }

                let mut request =
                    match server.recv_timeout(std::time::Duration::from_millis(500)) {
                        Ok(Some(req)) => req,
                        Ok(None) => continue,
                        Err(_) => continue,
                    };

                // Only allow localhost
                let remote = request
                    .remote_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_default();
                if !remote.starts_with("127.0.0.1") && !remote.starts_with("[::1]") {
                    let response = tiny_http::Response::from_string(r#"{"error":"Forbidden"}"#)
                        .with_status_code(403)
                        .with_header(
                            tiny_http::Header::from_bytes(
                                &b"Content-Type"[..],
                                &b"application/json"[..],
                            )
                            .unwrap(),
                        );
                    let _ = request.respond(response);
                    continue;
                }

                let full_url = request.url().to_string();
                let path = full_url
                    .split('?')
                    .next()
                    .unwrap_or(&full_url)
                    .to_string();

                // Parse query params
                let query_params = parse_query_string(&full_url);

                // Read body for POST requests
                let mut body_str = String::new();
                let _ = request.as_reader().read_to_string(&mut body_str);
                let body: serde_json::Value = serde_json::from_str(&body_str)
                    .or_else(|_| {
                        // Try fixing common PowerShell JSON issues
                        let fixed = body_str
                            .replace(|c: char| c == '\'' , "\"");
                        serde_json::from_str(&fixed)
                    })
                    .unwrap_or(serde_json::json!({}));

                let app = app_handle.clone();
                let tm = tab_manager.clone();
                let hm = history_manager.clone();
                let bm = bookmark_manager.clone();
                let dm = download_manager.clone();

                let result = rt.block_on(async {
                    handle_route(&path, &body, &query_params, app, tm, hm, bm, dm).await
                });

                let json = serde_json::to_string(&result)
                    .unwrap_or_else(|_| r#"{"error":"serialize failed"}"#.to_string());
                let response = tiny_http::Response::from_string(json).with_header(
                    tiny_http::Header::from_bytes(
                        &b"Content-Type"[..],
                        &b"application/json"[..],
                    )
                    .unwrap(),
                );
                let _ = request.respond(response);
            }
        });

        true
    }

    pub fn stop(&mut self) {
        self.stop_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.running = false;
    }
}

fn parse_query_string(url: &str) -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();
    if let Some(qs) = url.split('?').nth(1) {
        for pair in qs.split('&') {
            let mut kv = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                params.insert(
                    urlencoding::decode(k).unwrap_or_default().to_string(),
                    urlencoding::decode(v).unwrap_or_default().to_string(),
                );
            }
        }
    }
    params
}

fn get_param<'a>(
    body: &'a serde_json::Value,
    params: &'a std::collections::HashMap<String, String>,
    key: &str,
) -> Option<String> {
    body.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| params.get(key).cloned())
}

fn get_param_u32(
    body: &serde_json::Value,
    params: &std::collections::HashMap<String, String>,
    key: &str,
) -> Option<u32> {
    body.get(key)
        .and_then(|v| v.as_u64().map(|n| n as u32).or_else(|| v.as_str()?.parse().ok()))
        .or_else(|| params.get(key)?.parse().ok())
}

fn get_param_f64(
    body: &serde_json::Value,
    params: &std::collections::HashMap<String, String>,
    key: &str,
    default: f64,
) -> f64 {
    body.get(key)
        .and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok()))
        .or_else(|| params.get(key)?.parse().ok())
        .unwrap_or(default)
}

fn get_param_bool(body: &serde_json::Value, key: &str, default: bool) -> bool {
    body.get(key)
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

#[allow(clippy::too_many_arguments)]
async fn handle_route(
    path: &str,
    body: &serde_json::Value,
    params: &std::collections::HashMap<String, String>,
    app: tauri::AppHandle,
    tm: Arc<Mutex<TabManager>>,
    hm: Arc<Mutex<HistoryManager>>,
    bm: Arc<Mutex<BookmarkManager>>,
    dm: Arc<Mutex<DownloadManager>>,
) -> serde_json::Value {
    match path {
        // ─── Status ───
        "/status" => {
            let tm = tm.lock().await;
            serde_json::json!({
                "running": true,
                "tabs": tm.list_tabs()
            })
        }

        // ─── Tab operations ───
        "/tab/create" => {
            let url = get_param(body, params, "url");
            let mut tm = tm.lock().await;
            match tm.create_tab(&app, url).await {
                Ok(id) => serde_json::json!({ "id": id }),
                Err(e) => serde_json::json!({ "error": e.to_string() }),
            }
        }

        "/tab/close" => {
            let id = get_param_u32(body, params, "id").unwrap_or(0);
            let mut tm = tm.lock().await;
            tm.close_tab(&app, id);
            serde_json::json!({ "ok": true })
        }

        "/tab/switch" => {
            let id = get_param_u32(body, params, "id").unwrap_or(0);
            let mut tm = tm.lock().await;
            tm.switch_tab(&app, id);
            serde_json::json!({ "ok": true })
        }

        "/tab/list" => {
            let tm = tm.lock().await;
            tm.list_tabs()
        }

        "/tab/active" => {
            let tm = tm.lock().await;
            match tm.get_active_tab() {
                Some(tab) => serde_json::json!({
                    "id": tab.id,
                    "title": &tab.title,
                    "url": &tab.url
                }),
                None => serde_json::json!(null),
            }
        }

        "/tab/restore" => {
            let mut tm = tm.lock().await;
            match tm.restore_closed_tab(&app).await {
                Ok(Some(id)) => serde_json::json!({ "id": id }),
                Ok(None) => serde_json::json!({ "id": null }),
                Err(e) => serde_json::json!({ "error": e.to_string() }),
            }
        }

        "/tab/pin" => {
            let id = get_param_u32(body, params, "id").unwrap_or(0);
            let mut tm = tm.lock().await;
            tm.toggle_pin_tab(&app, id);
            serde_json::json!({ "ok": true })
        }

        "/tab/move" => {
            let tab_id = get_param_u32(body, params, "tabId").unwrap_or(0);
            let before_tab_id = get_param_u32(body, params, "beforeTabId");
            let mut tm = tm.lock().await;
            tm.move_tab(&app, tab_id, before_tab_id);
            serde_json::json!({ "ok": true })
        }

        "/tab/next" => {
            let mut tm = tm.lock().await;
            tm.switch_to_next_tab(&app);
            serde_json::json!({ "ok": true })
        }

        "/tab/prev" => {
            let mut tm = tm.lock().await;
            tm.switch_to_prev_tab(&app);
            serde_json::json!({ "ok": true })
        }

        // ─── Navigation ───
        "/nav/go" => {
            let url = get_param(body, params, "url").unwrap_or_default();
            let mut tm = tm.lock().await;
            match tm.navigate_to(&app, &url).await {
                Ok(()) => serde_json::json!({ "ok": true }),
                Err(e) => serde_json::json!({ "error": e.to_string() }),
            }
        }

        "/nav/back" => {
            let tm = tm.lock().await;
            tm.go_back(&app);
            serde_json::json!({ "ok": true })
        }

        "/nav/forward" => {
            let tm = tm.lock().await;
            tm.go_forward(&app);
            serde_json::json!({ "ok": true })
        }

        "/nav/reload" => {
            let tm = tm.lock().await;
            tm.reload(&app);
            serde_json::json!({ "ok": true })
        }

        "/nav/home" => {
            let mut tm = tm.lock().await;
            match tm.go_home(&app).await {
                Ok(()) => serde_json::json!({ "ok": true }),
                Err(e) => serde_json::json!({ "error": e.to_string() }),
            }
        }

        // ─── Page interaction ───
        "/page/title" => {
            let tm = tm.lock().await;
            serde_json::json!({
                "title": tm.get_active_tab().map(|t| t.title.clone()).unwrap_or_default()
            })
        }

        "/page/url" => {
            let tm = tm.lock().await;
            serde_json::json!({
                "url": tm.get_active_tab().map(|t| t.url.clone()).unwrap_or_default()
            })
        }

        "/page/exec" => {
            let code = get_param(body, params, "code").unwrap_or_default();
            let tm = tm.lock().await;
            match tm.execute_js(&app, &code).await {
                Ok(result) => serde_json::json!({ "result": result }),
                Err(e) => serde_json::json!({ "error": e.to_string() }),
            }
        }

        "/page/content" => {
            let tm = tm.lock().await;
            let _ = tm.execute_js(&app, "document.body.innerText").await;
            serde_json::json!({ "ok": true, "note": "Content retrieved via JS eval" })
        }

        // ─── High-level browser control ───

        "/page/inspect" => {
            let scope_selector = get_param(body, params, "selector");
            let scope_js = match &scope_selector {
                Some(s) => format!(
                    "document.querySelector({}) || document",
                    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
                ),
                None => "document".to_string(),
            };
            let script = format!(
                r#"(() => {{
                    const scope = {scope_js};
                    const selectors = 'a[href], button, input, textarea, select, [role="button"], [onclick], [tabindex]';
                    const els = Array.from(scope.querySelectorAll(selectors));
                    const visible = els.filter(el => {{
                        const r = el.getBoundingClientRect();
                        const s = getComputedStyle(el);
                        return r.width > 0 && r.height > 0 && s.visibility !== 'hidden' && s.display !== 'none';
                    }});
                    window.__inspectedElements = visible;
                    return JSON.stringify({{
                        count: visible.length,
                        elements: visible.map((el, i) => {{
                            const r = el.getBoundingClientRect();
                            return {{
                                index: i,
                                tag: el.tagName.toLowerCase(),
                                type: el.getAttribute('type') || '',
                                id: el.id || '',
                                name: el.getAttribute('name') || '',
                                text: (el.innerText || el.value || el.getAttribute('aria-label') || el.getAttribute('title') || el.placeholder || '').substring(0, 80).trim(),
                                href: el.getAttribute('href') || '',
                                placeholder: el.getAttribute('placeholder') || '',
                                role: el.getAttribute('role') || '',
                                bounds: {{ x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) }}
                            }};
                        }})
                    }});
                }})()"#
            );
            let tm = tm.lock().await;
            let _ = tm.execute_js(&app, &script).await;
            serde_json::json!({ "ok": true, "note": "Inspect executed - elements stored in window.__inspectedElements" })
        }

        "/page/click" => {
            let index = get_param_u32(body, params, "index");
            let selector = get_param(body, params, "selector");
            let script = if let Some(idx) = index {
                format!(
                    r#"(() => {{
                        if (!window.__inspectedElements || !window.__inspectedElements[{idx}]) throw new Error('Element not found. Run /page/inspect first.');
                        const el = window.__inspectedElements[{idx}];
                        el.scrollIntoView({{block:'center'}});
                        el.focus();
                        el.click();
                    }})()"#
                )
            } else if let Some(sel) = selector {
                let sel_json = serde_json::to_string(&sel).unwrap_or_else(|_| "\"\"".to_string());
                format!(
                    r#"(() => {{
                        const el = document.querySelector({sel_json});
                        if (!el) throw new Error('Element not found');
                        el.scrollIntoView({{block:'center'}});
                        el.focus();
                        el.click();
                    }})()"#
                )
            } else {
                return serde_json::json!({ "error": "Provide index or selector" });
            };
            let tm = tm.lock().await;
            let _ = tm.execute_js(&app, &script).await;
            serde_json::json!({ "ok": true })
        }

        "/page/type" => {
            let index = get_param_u32(body, params, "index");
            let selector = get_param(body, params, "selector");
            let text = get_param(body, params, "text").unwrap_or_default();
            let clear = get_param_bool(body, "clear", true);
            let submit = get_param_bool(body, "submit", false);
            let text_json = serde_json::to_string(&text).unwrap_or_else(|_| "\"\"".to_string());
            let clear_js = if clear { "el.value = '';" } else { "" };
            let submit_js = if submit {
                "el.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter',code:'Enter',keyCode:13,bubbles:true}));"
            } else {
                ""
            };

            let script = if let Some(idx) = index {
                format!(
                    r#"(() => {{
                        if (!window.__inspectedElements || !window.__inspectedElements[{idx}]) throw new Error('Element not found. Run /page/inspect first.');
                        const el = window.__inspectedElements[{idx}];
                        el.scrollIntoView({{block:'center'}});
                        el.focus();
                        {clear_js}
                        el.value = {text_json};
                        el.dispatchEvent(new Event('input', {{bubbles: true}}));
                        el.dispatchEvent(new Event('change', {{bubbles: true}}));
                        {submit_js}
                    }})()"#
                )
            } else if let Some(sel) = selector {
                let sel_json = serde_json::to_string(&sel).unwrap_or_else(|_| "\"\"".to_string());
                format!(
                    r#"(() => {{
                        const el = document.querySelector({sel_json});
                        if (!el) throw new Error('Element not found');
                        el.scrollIntoView({{block:'center'}});
                        el.focus();
                        {clear_js}
                        el.value = {text_json};
                        el.dispatchEvent(new Event('input', {{bubbles: true}}));
                        el.dispatchEvent(new Event('change', {{bubbles: true}}));
                        {submit_js}
                    }})()"#
                )
            } else {
                return serde_json::json!({ "error": "Provide index or selector" });
            };
            let tm = tm.lock().await;
            let _ = tm.execute_js(&app, &script).await;
            serde_json::json!({ "ok": true })
        }

        "/page/wait" => {
            let wait_selector = get_param(body, params, "selector");
            let timeout = get_param_f64(body, params, "timeout", 5000.0) as u64;
            if let Some(sel) = wait_selector {
                let sel_json = serde_json::to_string(&sel).unwrap_or_else(|_| "\"\"".to_string());
                let script = format!(
                    r#"new Promise((resolve, reject) => {{
                        const start = Date.now();
                        const check = () => {{
                            if (document.querySelector({sel_json})) return resolve(true);
                            if (Date.now() - start > {timeout}) return reject(new Error('Timeout'));
                            requestAnimationFrame(check);
                        }};
                        check();
                    }})"#
                );
                let tm = tm.lock().await;
                let _ = tm.execute_js(&app, &script).await;
            } else {
                // Wait for load complete
                let tm = tm.lock().await;
                let _ = tm
                    .execute_js(
                        &app,
                        &format!(
                            r#"new Promise(r => {{
                                if (document.readyState === 'complete') return r(true);
                                const t = setTimeout(() => r(false), {timeout});
                                window.addEventListener('load', () => {{ clearTimeout(t); r(true); }}, {{once:true}});
                            }})"#
                        ),
                    )
                    .await;
            }
            serde_json::json!({ "ok": true })
        }

        "/page/scroll" => {
            let direction = get_param(body, params, "direction").unwrap_or_else(|| "down".to_string());
            let amount = get_param_f64(body, params, "amount", 500.0);
            let scroll_selector = get_param(body, params, "selector");
            let script = if let Some(sel) = scroll_selector {
                let sel_json = serde_json::to_string(&sel).unwrap_or_else(|_| "\"\"".to_string());
                format!(
                    "document.querySelector({sel_json})?.scrollIntoView({{behavior:'smooth',block:'center'}})"
                )
            } else {
                let dy = if direction == "up" { -amount } else { amount };
                format!("window.scrollBy(0, {dy})")
            };
            let tm = tm.lock().await;
            let _ = tm.execute_js(&app, &script).await;
            serde_json::json!({ "ok": true })
        }

        "/page/submit" => {
            let form_selector = get_param(body, params, "selector")
                .unwrap_or_else(|| "form".to_string());
            let form_sel_json =
                serde_json::to_string(&form_selector).unwrap_or_else(|_| "\"form\"".to_string());

            // Fill fields if provided
            if let Some(fields) = body.get("fields").and_then(|v| v.as_object()) {
                let mut fill_script = String::new();
                for (sel, val) in fields {
                    let sel_json =
                        serde_json::to_string(sel).unwrap_or_else(|_| "\"\"".to_string());
                    let val_str = val.as_str().unwrap_or("");
                    let val_json =
                        serde_json::to_string(val_str).unwrap_or_else(|_| "\"\"".to_string());
                    fill_script.push_str(&format!(
                        "(() => {{ const el = document.querySelector({sel_json}); if(el){{el.value={val_json};el.dispatchEvent(new Event('input',{{bubbles:true}}));}} }})();"
                    ));
                }
                let tm = tm.lock().await;
                let _ = tm.execute_js(&app, &fill_script).await;
            }

            if get_param_bool(body, "submit", true) {
                let tm = tm.lock().await;
                let _ = tm
                    .execute_js(
                        &app,
                        &format!("document.querySelector({form_sel_json})?.submit()"),
                    )
                    .await;
            }
            serde_json::json!({ "ok": true })
        }

        // ─── Zoom ───
        "/zoom/in" => {
            let mut tm = tm.lock().await;
            tm.zoom_in(&app);
            serde_json::json!({ "ok": true })
        }

        "/zoom/out" => {
            let mut tm = tm.lock().await;
            tm.zoom_out(&app);
            serde_json::json!({ "ok": true })
        }

        "/zoom/reset" => {
            let mut tm = tm.lock().await;
            tm.zoom_reset(&app);
            serde_json::json!({ "ok": true })
        }

        // ─── Find in page ───
        "/find" => {
            let find_text = get_param(body, params, "text").unwrap_or_default();
            let forward = get_param_bool(body, "forward", true);
            let tm = tm.lock().await;
            if !find_text.is_empty() {
                tm.find_in_page(&app, &find_text, forward);
            } else {
                tm.stop_find(&app);
            }
            serde_json::json!({ "ok": true })
        }

        // ─── Fullscreen ───
        "/fullscreen/toggle" => {
            use tauri::Manager;
            if let Some(win) = app.get_window("main") {
                let is_fs = win.is_fullscreen().unwrap_or(false);
                let _ = win.set_fullscreen(!is_fs);
            }
            serde_json::json!({ "ok": true })
        }

        // ─── Storage (localStorage / sessionStorage) ───
        "/storage/get" => {
            let storage_type = if get_param(body, params, "type")
                .unwrap_or_default()
                == "session"
            {
                "sessionStorage"
            } else {
                "localStorage"
            };
            let key = get_param(body, params, "key");
            let script = if let Some(k) = key {
                let k_json = serde_json::to_string(&k).unwrap_or_else(|_| "\"\"".to_string());
                format!("{storage_type}.getItem({k_json})")
            } else {
                format!("JSON.stringify({storage_type})")
            };
            let tm = tm.lock().await;
            let _ = tm.execute_js(&app, &script).await;
            serde_json::json!({ "ok": true })
        }

        "/storage/set" => {
            let storage_type = if get_param(body, params, "type")
                .unwrap_or_default()
                == "session"
            {
                "sessionStorage"
            } else {
                "localStorage"
            };
            let key = get_param(body, params, "key").unwrap_or_default();
            let value = get_param(body, params, "value").unwrap_or_default();
            let k_json = serde_json::to_string(&key).unwrap_or_else(|_| "\"\"".to_string());
            let v_json = serde_json::to_string(&value).unwrap_or_else(|_| "\"\"".to_string());
            let tm = tm.lock().await;
            let _ = tm
                .execute_js(&app, &format!("{storage_type}.setItem({k_json}, {v_json})"))
                .await;
            serde_json::json!({ "ok": true })
        }

        "/storage/clear" => {
            let storage_type = if get_param(body, params, "type")
                .unwrap_or_default()
                == "session"
            {
                "sessionStorage"
            } else {
                "localStorage"
            };
            let key = get_param(body, params, "key");
            let tm = tm.lock().await;
            if let Some(k) = key {
                let k_json = serde_json::to_string(&k).unwrap_or_else(|_| "\"\"".to_string());
                let _ = tm
                    .execute_js(&app, &format!("{storage_type}.removeItem({k_json})"))
                    .await;
            } else {
                let _ = tm
                    .execute_js(&app, &format!("{storage_type}.clear()"))
                    .await;
            }
            serde_json::json!({ "ok": true })
        }

        // ─── Console Capture ───
        "/console/start" => {
            let script = r#"(() => {
                if (window.__sp_console_logs) return;
                window.__sp_console_logs = [];
                const orig = {};
                ['log','warn','error','info','debug'].forEach(level => {
                    orig[level] = console[level];
                    console[level] = function(...args) {
                        window.__sp_console_logs.push({
                            level, message: args.map(a => typeof a === 'object' ? JSON.stringify(a) : String(a)).join(' '),
                            timestamp: Date.now()
                        });
                        orig[level].apply(console, args);
                    };
                });
                window.__sp_console_restore = () => {
                    Object.keys(orig).forEach(k => { console[k] = orig[k]; });
                    delete window.__sp_console_logs;
                    delete window.__sp_console_restore;
                };
            })()"#;
            let tm = tm.lock().await;
            let _ = tm.execute_js(&app, script).await;
            serde_json::json!({ "ok": true })
        }

        "/console/stop" => {
            let tm = tm.lock().await;
            let _ = tm
                .execute_js(&app, "if(window.__sp_console_restore) window.__sp_console_restore()")
                .await;
            serde_json::json!({ "ok": true })
        }

        "/console/get" => {
            let tm = tm.lock().await;
            let _ = tm
                .execute_js(
                    &app,
                    "JSON.stringify(window.__sp_console_logs || [])",
                )
                .await;
            serde_json::json!({ "ok": true, "note": "Logs retrieved via JS eval" })
        }

        // ─── Network Capture ───
        "/network/start" => {
            let script = r#"(() => {
                if (window.__sp_network_logs) return;
                window.__sp_network_logs = [];
                const origFetch = window.fetch;
                window.fetch = async function(...args) {
                    const start = Date.now();
                    const url = typeof args[0] === 'string' ? args[0] : args[0]?.url || '';
                    const method = args[1]?.method || 'GET';
                    try {
                        const resp = await origFetch.apply(this, args);
                        window.__sp_network_logs.push({ url, method, status: resp.status, duration: Date.now() - start, timestamp: start });
                        return resp;
                    } catch(e) {
                        window.__sp_network_logs.push({ url, method, status: 0, error: e.message, duration: Date.now() - start, timestamp: start });
                        throw e;
                    }
                };
                const origXHR = XMLHttpRequest.prototype.open;
                XMLHttpRequest.prototype.open = function(method, url) {
                    this.__sp_method = method;
                    this.__sp_url = url;
                    this.__sp_start = Date.now();
                    this.addEventListener('loadend', () => {
                        window.__sp_network_logs.push({ url: this.__sp_url, method: this.__sp_method, status: this.status, duration: Date.now() - this.__sp_start, timestamp: this.__sp_start });
                    });
                    return origXHR.apply(this, arguments);
                };
                window.__sp_network_restore = () => {
                    window.fetch = origFetch;
                    XMLHttpRequest.prototype.open = origXHR;
                    delete window.__sp_network_logs;
                    delete window.__sp_network_restore;
                };
            })()"#;
            let tm = tm.lock().await;
            let _ = tm.execute_js(&app, script).await;
            serde_json::json!({ "ok": true })
        }

        "/network/stop" => {
            let tm = tm.lock().await;
            let _ = tm
                .execute_js(&app, "if(window.__sp_network_restore) window.__sp_network_restore()")
                .await;
            serde_json::json!({ "ok": true })
        }

        "/network/get" => {
            let tm = tm.lock().await;
            let _ = tm
                .execute_js(
                    &app,
                    "JSON.stringify(window.__sp_network_logs || [])",
                )
                .await;
            serde_json::json!({ "ok": true, "note": "Network logs retrieved via JS eval" })
        }

        // ─── Page Performance ───
        "/page/performance" => {
            let script = r#"(() => {
                const nav = performance.getEntriesByType('navigation')[0];
                if (nav) {
                    return JSON.stringify({
                        dns: nav.domainLookupEnd - nav.domainLookupStart,
                        tcp: nav.connectEnd - nav.connectStart,
                        ttfb: nav.responseStart - nav.requestStart,
                        download: nav.responseEnd - nav.responseStart,
                        domParse: nav.domInteractive - nav.responseEnd,
                        domReady: nav.domContentLoadedEventEnd - nav.fetchStart,
                        load: nav.loadEventEnd - nav.fetchStart,
                        transferSize: nav.transferSize,
                        encodedBodySize: nav.encodedBodySize,
                        decodedBodySize: nav.decodedBodySize,
                    });
                }
                return JSON.stringify({});
            })()"#;
            let tm = tm.lock().await;
            let _ = tm.execute_js(&app, script).await;
            serde_json::json!({ "ok": true })
        }

        // ─── Viewport ───
        "/viewport/get" => {
            use tauri::Manager;
            if let Some(win) = app.get_window("main") {
                if let Ok(size) = win.inner_size() {
                    return serde_json::json!({
                        "width": size.width,
                        "height": size.height
                    });
                }
            }
            serde_json::json!({ "width": 0, "height": 0 })
        }

        "/viewport/set" => {
            use tauri::Manager;
            let width = get_param_f64(body, params, "width", 1280.0) as u32;
            let height = get_param_f64(body, params, "height", 800.0) as u32;
            if let Some(win) = app.get_window("main") {
                let _ = win.set_size(tauri::LogicalSize::new(width, height));
            }
            serde_json::json!({ "ok": true })
        }

        // ─── History ───
        "/history/list" => {
            let hm = hm.lock().await;
            let query = get_param(body, params, "query");
            let limit = get_param_f64(body, params, "limit", 50.0) as usize;
            let entries = hm.get_entries(query.as_deref(), limit, 0);
            serde_json::json!({ "entries": entries })
        }

        "/history/search" => {
            let hm = hm.lock().await;
            let query = get_param(body, params, "query")
                .or_else(|| get_param(body, params, "q"))
                .unwrap_or_default();
            let limit = get_param_f64(body, params, "limit", 8.0) as usize;
            let entries = hm.search(&query, limit);
            serde_json::json!({ "entries": entries })
        }

        "/history/clear" => {
            let mut hm = hm.lock().await;
            hm.clear_all();
            serde_json::json!({ "ok": true })
        }

        // ─── Bookmarks ───
        "/bookmark/add" => {
            let mut bm = bm.lock().await;
            let url = get_param(body, params, "url").unwrap_or_default();
            let title = get_param(body, params, "title").unwrap_or_default();
            let folder_id = get_param(body, params, "folderId");
            let bookmark = bm.add_bookmark(&url, &title, folder_id.as_deref(), None);
            serde_json::json!({ "bookmark": bookmark })
        }

        "/bookmark/remove" => {
            let mut bm = bm.lock().await;
            let id = get_param(body, params, "id").unwrap_or_default();
            bm.remove_bookmark(&id);
            serde_json::json!({ "ok": true })
        }

        "/bookmark/list" => {
            let bm = bm.lock().await;
            let bookmarks = bm.get_all_bookmarks();
            serde_json::json!({ "bookmarks": bookmarks })
        }

        "/bookmark/check" => {
            let bm = bm.lock().await;
            let url = get_param(body, params, "url").unwrap_or_default();
            let bookmark = bm.is_bookmarked(&url);
            serde_json::json!({ "bookmark": bookmark })
        }

        // ─── Downloads ───
        "/download/list" => {
            let dm = dm.lock().await;
            serde_json::json!({
                "active": dm.get_active_downloads(),
                "history": dm.get_history()
            })
        }

        // ─── Page Errors ───
        "/page/errors" => {
            let script = r#"JSON.stringify(
                (window.__sp_error_logs || []).concat(
                    (window.__sp_console_logs || []).filter(function(e){ return e.level === 'error'; })
                )
            )"#;
            let tm = tm.lock().await;
            let _ = tm.execute_js(&app, script).await;
            serde_json::json!({ "ok": true, "note": "Error logs retrieved via JS eval" })
        }

        // ─── Cookie Management (JS document.cookie) ───
        "/cookie/get" => {
            let key = get_param(body, params, "name");
            let script = if let Some(k) = key {
                let k_json = serde_json::to_string(&k).unwrap_or_else(|_| "\"\"".to_string());
                format!(
                    r#"(() => {{
                        var name = {k_json} + '=';
                        var cookies = document.cookie.split(';');
                        for(var i=0;i<cookies.length;i++){{
                            var c = cookies[i].trim();
                            if(c.indexOf(name)===0) return c.substring(name.length);
                        }}
                        return null;
                    }})()"#
                )
            } else {
                r#"(() => {
                    var result = {};
                    document.cookie.split(';').forEach(function(c){
                        var parts = c.trim().split('=');
                        if(parts[0]) result[parts[0]] = parts.slice(1).join('=');
                    });
                    return JSON.stringify(result);
                })()"#.to_string()
            };
            let tm = tm.lock().await;
            let _ = tm.execute_js(&app, &script).await;
            serde_json::json!({ "ok": true, "note": "Cookie retrieved via JS eval (HttpOnly cookies not accessible)" })
        }

        "/cookie/set" => {
            let name = get_param(body, params, "name").unwrap_or_default();
            let value = get_param(body, params, "value").unwrap_or_default();
            let path = get_param(body, params, "path").unwrap_or_else(|| "/".to_string());
            let max_age = get_param(body, params, "maxAge");
            let n_json = serde_json::to_string(&name).unwrap_or_else(|_| "\"\"".to_string());
            let v_json = serde_json::to_string(&value).unwrap_or_else(|_| "\"\"".to_string());
            let p_json = serde_json::to_string(&path).unwrap_or_else(|_| "\"/\"".to_string());
            let age_part = match max_age {
                Some(a) => format!(";max-age={}", a),
                None => String::new(),
            };
            let script = format!(
                "document.cookie = {n_json} + '=' + {v_json} + ';path=' + {p_json} + '{age_part}'"
            );
            let tm = tm.lock().await;
            let _ = tm.execute_js(&app, &script).await;
            serde_json::json!({ "ok": true })
        }

        "/cookie/delete" => {
            let name = get_param(body, params, "name").unwrap_or_default();
            let n_json = serde_json::to_string(&name).unwrap_or_else(|_| "\"\"".to_string());
            let script = format!(
                "document.cookie = {n_json} + '=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/'"
            );
            let tm = tm.lock().await;
            let _ = tm.execute_js(&app, &script).await;
            serde_json::json!({ "ok": true })
        }

        "/cookie/clear" => {
            let script = r#"document.cookie.split(';').forEach(function(c){
                var name = c.trim().split('=')[0];
                if(name) document.cookie = name + '=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/';
            })"#;
            let tm = tm.lock().await;
            let _ = tm.execute_js(&app, script).await;
            serde_json::json!({ "ok": true })
        }

        // ─── CDP JS eval (returns result) ───
        "/page/cdp-eval" => {
            let code = get_param(body, params, "code").unwrap_or_default();
            if code.is_empty() {
                return serde_json::json!({ "error": "No code provided" });
            }
            let tm_locked = tm.lock().await;
            if let Some(webview) = tm_locked.get_active_webview(&app) {
                drop(tm_locked);
                let (tx, rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
                let expr = serde_json::json!({
                    "expression": code,
                    "returnByValue": true
                }).to_string();
                let _ = webview.with_webview(move |pv| {
                    use webview2_com::Microsoft::Web::WebView2::Win32::*;
                    use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
                    use windows::core::HSTRING;
                    unsafe {
                        let controller = pv.controller();
                        let core: ICoreWebView2 = controller.CoreWebView2().unwrap();
                        let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(
                            move |_r: windows::core::Result<()>, json: String| { let _ = tx.send(Ok(json)); Ok(()) },
                        ));
                        let _ = core.CallDevToolsProtocolMethod(&HSTRING::from("Runtime.evaluate"), &HSTRING::from(expr.as_str()), &handler);
                    }
                });
                match tokio::time::timeout(std::time::Duration::from_secs(5), rx).await {
                    Ok(Ok(Ok(json_str))) => {
                        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap_or_default();
                        parsed
                    }
                    _ => serde_json::json!({ "error": "CDP eval timeout" })
                }
            } else {
                drop(tm_locked);
                serde_json::json!({ "error": "No active tab" })
            }
        }

        // ─── Screenshot (via CDP - 4-step Emulation override approach) ───
        "/page/screenshot" | "/page/screenshot/full" => {
            // Helper closure for CDP calls
            let cdp_call = |app_handle: &tauri::AppHandle, tm_arc: &std::sync::Arc<tokio::sync::Mutex<crate::tab_manager::TabManager>>, method: String, params: String| {
                let app_h = app_handle.clone();
                let tm_c = tm_arc.clone();
                async move {
                    let tm_l = tm_c.lock().await;
                    let wv = tm_l.get_active_webview(&app_h).ok_or("No active tab".to_string())?;
                    drop(tm_l);
                    let (tx, rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
                    let _ = wv.with_webview(move |pv| {
                        use webview2_com::Microsoft::Web::WebView2::Win32::*;
                        use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
                        use windows::core::HSTRING;
                        unsafe {
                            let controller = pv.controller();
                            let core: ICoreWebView2 = controller.CoreWebView2().unwrap();
                            let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(
                                move |_r: windows::core::Result<()>, json: String| { let _ = tx.send(Ok(json)); Ok(()) },
                            ));
                            let _ = core.CallDevToolsProtocolMethod(&HSTRING::from(method.as_str()), &HSTRING::from(params.as_str()), &handler);
                        }
                    });
                    tokio::time::timeout(std::time::Duration::from_secs(10), rx)
                        .await.map_err(|_| "CDP timeout".to_string())?
                        .map_err(|_| "CDP channel error".to_string())?
                }
            };

            // Step 1: Get page dimensions
            let dim_json = match cdp_call(&app, &tm, "Runtime.evaluate".to_string(),
                r#"{"expression":"JSON.stringify({w:window.innerWidth,h:Math.max(document.body.scrollHeight,document.documentElement.scrollHeight,document.body.offsetHeight,document.documentElement.offsetHeight),dpr:window.devicePixelRatio})","returnByValue":true}"#.to_string()
            ).await {
                Ok(j) => j,
                Err(e) => return serde_json::json!({ "error": e })
            };

            let dim_resp: serde_json::Value = serde_json::from_str(&dim_json).unwrap_or_default();
            let dim_str = dim_resp["result"]["value"].as_str().unwrap_or("{}");
            let dims: serde_json::Value = serde_json::from_str(dim_str).unwrap_or_default();
            let css_w = dims["w"].as_f64().unwrap_or(1280.0) as i64;
            let css_h = dims["h"].as_f64().unwrap_or(800.0) as i64;
            let dpr = dims["dpr"].as_f64().unwrap_or(1.0);

            // Step 2: Override viewport to full page height (width stays same → no responsive break)
            let override_params = format!(
                r#"{{"width":{},"height":{},"deviceScaleFactor":{},"mobile":false}}"#,
                css_w, css_h, dpr
            );
            let _ = cdp_call(&app, &tm, "Emulation.setDeviceMetricsOverride".to_string(), override_params).await;
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;

            // Step 3: Capture screenshot (no captureBeyondViewport needed)
            let capture_result = cdp_call(&app, &tm, "Page.captureScreenshot".to_string(), r#"{"format":"png"}"#.to_string()).await;

            // Step 4: Clear override to restore original viewport
            let _ = cdp_call(&app, &tm, "Emulation.clearDeviceMetricsOverride".to_string(), "{}".to_string()).await;

            match capture_result {
                Ok(json_str) => {
                    let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap_or_default();
                    if let Some(b64) = parsed["data"].as_str() {
                        use base64::Engine;
                        if let Ok(png_data) = base64::engine::general_purpose::STANDARD.decode(b64) {
                            if let Some(downloads) = dirs::download_dir() {
                                let timestamp = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();
                                let filename = format!("screenshot_{}.png", timestamp);
                                let filepath = downloads.join(&filename);
                                let _ = std::fs::write(&filepath, &png_data);
                                let _ = open::that(&filepath);
                                serde_json::json!({ "ok": true, "file": filepath.to_string_lossy(), "size": format!("{}x{}", css_w, css_h) })
                            } else {
                                serde_json::json!({ "error": "Downloads directory not found" })
                            }
                        } else {
                            serde_json::json!({ "error": "Base64 decode failed" })
                        }
                    } else {
                        serde_json::json!({ "error": "No data in CDP response", "raw": json_str })
                    }
                }
                Err(e) => serde_json::json!({ "error": e })
            }
        }

        // ─── DevTools toggle ───
        "/devtools/toggle" => {
            let tm = tm.lock().await;
            tm.open_devtools(&app);
            serde_json::json!({ "ok": true })
        }

        // ─── UI Action (trigger menu-overlay-action event) ───
        "/ui/action" => {
            use tauri::Emitter;
            let action = get_param(body, params, "action").unwrap_or_default();
            let payload = get_param(body, params, "payload").unwrap_or_default();
            let _ = app.emit("menu-overlay-action", serde_json::json!({ "action": action, "payload": payload }));
            serde_json::json!({ "ok": true })
        }

        // ─── Screenshot save (receives base64 PNG from JS) ───
        "/screenshot/save" => {
            let data = get_param(body, params, "data").unwrap_or_default();
            if data.is_empty() {
                serde_json::json!({ "error": "No data provided" })
            } else {
                let b64 = if let Some(pos) = data.find(',') { &data[pos + 1..] } else { &data[..] };
                use base64::Engine;
                match base64::engine::general_purpose::STANDARD.decode(b64) {
                    Ok(bytes) => {
                        if let Some(downloads) = dirs::download_dir() {
                            let ts = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default().as_secs();
                            let path = downloads.join(format!("screenshot_{}.png", ts));
                            match std::fs::write(&path, bytes) {
                                Ok(_) => {
                                    let _ = open::that(&path);
                                    serde_json::json!({ "ok": true, "path": path.to_string_lossy() })
                                }
                                Err(e) => serde_json::json!({ "error": format!("Write error: {}", e) })
                            }
                        } else {
                            serde_json::json!({ "error": "No downloads folder" })
                        }
                    }
                    Err(e) => serde_json::json!({ "error": format!("Base64 decode: {}", e) })
                }
            }
        }

        _ => serde_json::json!({ "error": "Unknown route", "path": path }),
    }
}
