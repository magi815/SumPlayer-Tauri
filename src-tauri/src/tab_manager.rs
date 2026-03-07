use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub id: u32,
    pub title: String,
    pub url: String,
    pub favicon: String,
    pub zoom_factor: f64,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClosedTabInfo {
    url: String,
    title: String,
}

pub struct TabManager {
    tabs: Vec<Tab>,
    active_tab_id: Option<u32>,
    next_tab_id: u32,
    closed_tabs: Vec<ClosedTabInfo>,
    max_closed_tabs: usize,
    data_dir: PathBuf,
    home_page: String,
    chrome_height: u32,
    extra_offset: u32,
    tab_hidden: bool,
    last_window_size: Option<tauri::PhysicalSize<u32>>,
}

impl TabManager {
    pub fn new(data_dir: PathBuf) -> Self {
        let home_page = Self::load_home_page(&data_dir);
        TabManager {
            tabs: Vec::new(),
            active_tab_id: None,
            next_tab_id: 1,
            closed_tabs: Vec::new(),
            max_closed_tabs: 10,
            data_dir,
            home_page,
            chrome_height: 114,
            extra_offset: 0,
            tab_hidden: false,
            last_window_size: None,
        }
    }

    fn load_home_page(data_dir: &PathBuf) -> String {
        let path = data_dir.join("settings.json");
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&data) {
                if let Some(hp) = settings.get("homePage").and_then(|v| v.as_str()) {
                    return hp.to_string();
                }
            }
        }
        "https://www.youtube.com".to_string()
    }

    pub fn get_home_page(&self) -> String {
        self.home_page.clone()
    }

    pub fn set_home_page(&mut self, url: &str) {
        self.home_page = url.to_string();
        let path = self.data_dir.join("settings.json");
        let settings = serde_json::json!({ "homePage": url });
        let _ = std::fs::write(&path, serde_json::to_string_pretty(&settings).unwrap());
    }

    pub async fn create_tab(
        &mut self,
        app: &AppHandle,
        url: Option<String>,
    ) -> Result<u32, Box<dyn std::error::Error>> {
        let url = url.unwrap_or_else(|| self.home_page.clone());
        let id = self.next_tab_id;
        self.next_tab_id += 1;

        let tab = Tab {
            id,
            title: "New Tab".to_string(),
            url: url.clone(),
            favicon: String::new(),
            zoom_factor: 1.0,
            pinned: false,
        };

        self.tabs.push(tab.clone());
        self.active_tab_id = Some(id);

        // Emit tab-created event to frontend
        let _ = app.emit("tab-created", serde_json::json!({
            "id": id,
            "title": "New Tab",
            "url": &url,
            "favicon": ""
        }));

        // In Tauri v2, we create a new webview for each tab
        // add_child is on Window, not WebviewWindow
        let label = format!("tab-{}", id);
        let main_window = app.get_window("main");
        if let Some(win) = main_window {
            let tab_id = id;
            let app_handle = app.clone();

            // YouTube Ad Bypass — 6-layer ad blocker injected at document-start
            let yt_adblock_init = r#"(function(){
                'use strict';
                if(window.__sp_adbypass) return;
                window.__sp_adbypass = true;

                var AD_FIELDS = ['adPlacements','playerAds','adSlots','adBreakHeartbeatParams','adBreakParams'];
                var ENFORCEMENT_FIELDS = ['enforcementMessageViewModel','enforcementMessage','adBlockerOverlay','adBlockDetected'];

                function stripAds(obj){
                    if(!obj||typeof obj!=='object') return obj;
                    for(var i=0;i<AD_FIELDS.length;i++){ if(AD_FIELDS[i] in obj) delete obj[AD_FIELDS[i]]; }
                    if(obj.playerResponse&&typeof obj.playerResponse==='object') stripAds(obj.playerResponse);
                    return obj;
                }
                function hasAdData(obj){
                    if(!obj||typeof obj!=='object') return false;
                    return !!(obj.adPlacements||obj.playerAds||obj.adSlots||
                        (obj.playerResponse&&(obj.playerResponse.adPlacements||obj.playerResponse.playerAds||obj.playerResponse.adSlots)));
                }
                function stripEnforcement(obj,depth){
                    if(!obj||typeof obj!=='object'||depth>12) return false;
                    var stripped=false;
                    if(Array.isArray(obj)){
                        for(var i=obj.length-1;i>=0;i--){
                            var item=obj[i];
                            if(item&&typeof item==='object'){
                                var popup=item.openPopupAction&&item.openPopupAction.popup;
                                if(popup&&(popup.enforcementMessageViewModel||popup.confirmDialogRenderer)){
                                    var s=JSON.stringify(item);
                                    if(s.indexOf('nforcement')!==-1||s.indexOf('dBlocker')!==-1){obj.splice(i,1);stripped=true;continue;}
                                }
                                if(stripEnforcement(item,depth+1)) stripped=true;
                            }
                        }
                        return stripped;
                    }
                    for(var j=0;j<ENFORCEMENT_FIELDS.length;j++){if(ENFORCEMENT_FIELDS[j] in obj){delete obj[ENFORCEMENT_FIELDS[j]];stripped=true;}}
                    if(Array.isArray(obj.actions)){
                        var before=obj.actions.length;
                        obj.actions=obj.actions.filter(function(a){
                            if(!a||typeof a!=='object') return true;
                            var p=(a.openPopupAction&&a.openPopupAction.popup)||(a.showDialogCommand&&a.showDialogCommand.dialog);
                            if(!p) return true;
                            if(p.enforcementMessageViewModel||p.confirmDialogRenderer){
                                var ss=JSON.stringify(p).substring(0,2000);
                                if(ss.indexOf('nforcement')!==-1||ss.indexOf('dBlocker')!==-1) return false;
                            }
                            return true;
                        });
                        if(obj.actions.length<before) stripped=true;
                    }
                    var keys=Object.keys(obj);
                    for(var k=0;k<keys.length;k++){var val=obj[keys[k]];if(val&&typeof val==='object'){if(stripEnforcement(val,depth+1)) stripped=true;}}
                    return stripped;
                }

                // Layer 1: Trap ytInitialPlayerResponse & ytInitialData
                function trapProp(name){
                    var val=window[name];
                    try{
                        Object.defineProperty(window,name,{
                            get:function(){return val;},
                            set:function(v){
                                if(v&&typeof v==='object'){
                                    if(hasAdData(v)) stripAds(v);
                                    try{stripEnforcement(v,0);}catch(e){}
                                }
                                val=v;
                            },
                            configurable:true,enumerable:true
                        });
                    }catch(e){}
                }
                trapProp('ytInitialPlayerResponse');
                trapProp('ytInitialData');

                // Layer 2: Override JSON.parse
                var nativeParse=JSON.parse;
                JSON.parse=function(text,reviver){
                    var result=nativeParse.call(this,text,reviver);
                    try{
                        if(result&&typeof result==='object'){
                            if(hasAdData(result)) stripAds(result);
                            if(typeof text==='string'&&text.length>200&&(text.indexOf('nforcement')!==-1||text.indexOf('dBlocker')!==-1)){
                                stripEnforcement(result,0);
                            }
                        }
                    }catch(e){}
                    return result;
                };

                // Layer 3: Override Response.prototype.json()
                var YT_PATHS=['/youtubei/v1/','/get_midroll_','/player?','/next?'];
                var nativeJson=Response.prototype.json;
                Response.prototype.json=function(){
                    var self=this;
                    return nativeJson.call(this).then(function(data){
                        try{
                            var url=self.url||'';
                            var isYT=false;
                            for(var i=0;i<YT_PATHS.length;i++){if(url.indexOf(YT_PATHS[i])!==-1){isYT=true;break;}}
                            if(data&&typeof data==='object'&&isYT){
                                if(hasAdData(data)) stripAds(data);
                                stripEnforcement(data,0);
                            }
                        }catch(e){}
                        return data;
                    });
                };

                // Layer 4: Block ad video streams
                function isAdVideoUrl(u){return typeof u==='string'&&u.indexOf('googlevideo.com/videoplayback')!==-1&&/[?&]ctier=/.test(u);}
                var nativeFetch=window.fetch;
                window.fetch=function(input,init){
                    var url=(typeof input==='string')?input:(input&&input.url?input.url:'');
                    if(isAdVideoUrl(url)) return Promise.resolve(new Response('',{status:204}));
                    return nativeFetch.call(this,input,init);
                };
                var nativeXHROpen=XMLHttpRequest.prototype.open;
                XMLHttpRequest.prototype.open=function(method,url){
                    this._spBlocked=isAdVideoUrl(url);
                    return nativeXHROpen.apply(this,arguments);
                };
                var nativeXHRSend=XMLHttpRequest.prototype.send;
                XMLHttpRequest.prototype.send=function(){
                    if(this._spBlocked){
                        Object.defineProperty(this,'readyState',{value:4});
                        Object.defineProperty(this,'status',{value:204});
                        Object.defineProperty(this,'responseText',{value:''});
                        this.dispatchEvent(new Event('readystatechange'));
                        this.dispatchEvent(new Event('load'));
                        this.dispatchEvent(new Event('loadend'));
                        return;
                    }
                    return nativeXHRSend.apply(this,arguments);
                };
                // Block window.open to ad URLs
                var origOpen=window.open;
                window.open=function(url){
                    if(url&&typeof url==='string'&&(url.indexOf('googleadservices.com')!==-1||url.indexOf('doubleclick.net')!==-1||url.indexOf('googlesyndication.com')!==-1||url.indexOf('/pagead/')!==-1||url.indexOf('/aclk?')!==-1)) return null;
                    return origOpen.apply(this,arguments);
                };

                // Layer 5: CSS hiding
                var style=document.createElement('style');
                style.textContent='.ytp-ad-module,.ytp-ad-overlay-container,.ytp-ad-message-container,.ytp-ad-preview-container,.ytp-ad-skip-button-container,.ytp-ad-text,.ytp-ad-image-overlay,.video-ads,#player-ads,ytd-action-companion-ad-renderer,ytd-promoted-sparkles-web-renderer,ytd-ad-slot-renderer,ytd-banner-promo-renderer,ytd-statement-banner-renderer,ytd-promoted-video-renderer,ytd-display-ad-renderer,ytd-primetime-promo-renderer,#masthead-ad,ytd-enforcement-message-view-model,tp-yt-iron-overlay-backdrop.opened{display:none!important}';
                (document.head||document.documentElement).appendChild(style);

                // Layer 6: MutationObserver for enforcement popups
                var obs=new MutationObserver(function(mutations){
                    for(var m=0;m<mutations.length;m++){
                        for(var n=0;n<mutations[m].addedNodes.length;n++){
                            var node=mutations[m].addedNodes[n];
                            if(!(node instanceof HTMLElement)) continue;
                            var adSlot=node.matches&&node.matches('ytd-ad-slot-renderer')?node:(node.querySelector&&node.querySelector('ytd-ad-slot-renderer'));
                            if(adSlot){var container=adSlot.closest('ytd-rich-item-renderer');if(container){container.style.setProperty('display','none','important');}}
                            var enforcement=node.matches&&node.matches('ytd-enforcement-message-view-model,tp-yt-paper-dialog')?node:(node.querySelector&&node.querySelector('ytd-enforcement-message-view-model'));
                            if(enforcement){
                                var dialog=enforcement.closest&&enforcement.closest('tp-yt-paper-dialog')||enforcement;
                                dialog.style.display='none';dialog.removeAttribute('opened');
                                var backdrop=document.querySelector('tp-yt-iron-overlay-backdrop');
                                if(backdrop){backdrop.style.display='none';backdrop.classList.remove('opened');}
                                setTimeout(function(){try{dialog.remove();}catch(e){}},0);
                            }
                            var mealbar=node.matches&&node.matches('ytd-mealbar-promo-renderer')?node:(node.querySelector&&node.querySelector('ytd-mealbar-promo-renderer'));
                            if(mealbar&&/ad.?blocker|interruption|allow.*ads/i.test(mealbar.textContent||'')){mealbar.remove();}
                        }
                    }
                });
                obs.observe(document.documentElement,{childList:true,subtree:true});

                window.addEventListener('yt-navigate-finish',function(){
                    requestAnimationFrame(function(){
                        var slots=document.querySelectorAll('ytd-ad-slot-renderer');
                        for(var i=0;i<slots.length;i++){
                            var c=slots[i].closest('ytd-rich-item-renderer');
                            if(c) c.style.setProperty('display','none','important');
                        }
                    });
                });
            })()"#;

            let builder = tauri::webview::WebviewBuilder::new(
                &label,
                tauri::WebviewUrl::External(
                    url.parse()
                        .unwrap_or_else(|_| "https://www.youtube.com".parse().unwrap()),
                ),
            )
            .initialization_script(yt_adblock_init)
            .on_page_load(move |webview, payload| {
                if let tauri::webview::PageLoadEvent::Finished = payload.event() {
                    let url_str = payload.url().to_string();

                    // Auto-close sponsor/ad redirect pages
                    if url_str.contains("googleadservices.com")
                        || url_str.contains("doubleclick.net/")
                        || url_str.contains("googlesyndication.com")
                        || url_str.contains("/pagead/")
                        || url_str.contains("/aclk?")
                    {
                        let _ = webview.eval(&format!(
                            "setTimeout(function(){{ if(window.__TAURI__) window.__TAURI__.core.invoke('tab_close', {{ id: {} }}); }}, 100)",
                            tab_id
                        ));
                        return;
                    }

                    // Inject script to report title/URL back to Rust
                    let js = format!(
                        r#"setTimeout(function(){{ if(window.__TAURI__){{
                            window.__TAURI__.core.invoke('webview_title_changed', {{ tabId: {tab_id}, title: document.title, url: window.location.href }});
                            // Favicon detection
                            (function(){{
                                var icon = '';
                                var link = document.querySelector('link[rel~="icon"]') || document.querySelector('link[rel="shortcut icon"]');
                                if(link) icon = link.href;
                                else icon = window.location.origin + '/favicon.ico';
                                if(icon) window.__TAURI__.core.invoke('tab_favicon_changed', {{ tabId: {tab_id}, favicon: icon }});
                            }})();
                            // Error capture (window.onerror + unhandledrejection)
                            if(!window.__sp_error_logs){{
                                window.__sp_error_logs = [];
                                window.addEventListener('error', function(e){{
                                    window.__sp_error_logs.push({{ type:'error', message: e.message || String(e), source: e.filename || '', line: e.lineno || 0, col: e.colno || 0, timestamp: Date.now() }});
                                }});
                                window.addEventListener('unhandledrejection', function(e){{
                                    window.__sp_error_logs.push({{ type:'unhandledrejection', message: e.reason ? (e.reason.message || String(e.reason)) : 'Unknown', timestamp: Date.now() }});
                                }});
                            }}
                            // Keyboard shortcut forwarding to main UI
                            if(!window.__sp_shortcuts){{
                                window.__sp_shortcuts = true;
                                document.addEventListener('keydown', function(e){{
                                    var ctrl = e.ctrlKey || e.metaKey;
                                    if(ctrl && e.key === 't'){{ e.preventDefault(); window.__TAURI__.core.invoke('tab_create', {{url: null}}); return; }}
                                    if(ctrl && e.key === 'w'){{ e.preventDefault(); window.__TAURI__.core.invoke('tab_close', {{id: {tab_id}}}); return; }}
                                    if(ctrl && e.shiftKey && e.key === 'T'){{ e.preventDefault(); window.__TAURI__.core.invoke('tab_restore'); return; }}
                                    if(ctrl && !e.shiftKey && e.key === 'Tab'){{ e.preventDefault(); window.__TAURI__.core.invoke('tab_next'); return; }}
                                    if(ctrl && e.shiftKey && e.key === 'Tab'){{ e.preventDefault(); window.__TAURI__.core.invoke('tab_prev'); return; }}
                                    if(ctrl && e.key === 'l'){{ e.preventDefault(); window.__TAURI__.core.invoke('ui_action', {{action: 'focus-url'}}); return; }}
                                    if(ctrl && e.key === 'f'){{ e.preventDefault(); window.__TAURI__.core.invoke('ui_action', {{action: 'find'}}); return; }}
                                    if(ctrl && e.key === 'd'){{ e.preventDefault(); window.__TAURI__.core.invoke('ui_action', {{action: 'bookmark'}}); return; }}
                                    if(ctrl && e.key === 'h'){{ e.preventDefault(); window.__TAURI__.core.invoke('ui_action', {{action: 'history'}}); return; }}
                                    if(ctrl && e.key === 'j'){{ e.preventDefault(); window.__TAURI__.core.invoke('ui_action', {{action: 'downloads'}}); return; }}
                                    if(ctrl && (e.key === '=' || e.key === '+')){{ e.preventDefault(); window.__TAURI__.core.invoke('zoom_in'); return; }}
                                    if(ctrl && e.key === '-'){{ e.preventDefault(); window.__TAURI__.core.invoke('zoom_out'); return; }}
                                    if(ctrl && e.key === '0'){{ e.preventDefault(); window.__TAURI__.core.invoke('zoom_reset'); return; }}
                                    if(e.key === 'F5' || (ctrl && e.key === 'r')){{ e.preventDefault(); window.location.reload(); return; }}
                                    if(e.key === 'F11'){{ e.preventDefault(); window.__TAURI__.core.invoke('window_fullscreen'); return; }}
                                    if(e.altKey && e.key === 'ArrowLeft'){{ e.preventDefault(); window.history.back(); return; }}
                                    if(e.altKey && e.key === 'ArrowRight'){{ e.preventDefault(); window.history.forward(); return; }}
                                }});
                            }}
                            // Ctrl+scroll zoom
                            if(!window.__sp_ctrl_zoom){{
                                window.__sp_ctrl_zoom = true;
                                document.addEventListener('wheel', function(e){{
                                    if(e.ctrlKey){{
                                        e.preventDefault();
                                        if(e.deltaY < 0) window.__TAURI__.core.invoke('zoom_in');
                                        else window.__TAURI__.core.invoke('zoom_out');
                                    }}
                                }}, {{ passive: false }});
                            }}
                            // Right-click context menu
                            if(!window.__sp_ctx_menu){{
                                window.__sp_ctx_menu = true;
                                document.addEventListener('contextmenu', function(e){{
                                    e.preventDefault();
                                    var old = document.getElementById('__sp_ctx_menu');
                                    if(old) old.remove();
                                    var sel = window.getSelection().toString().trim();
                                    var link = e.target.closest('a');
                                    var img = e.target.closest('img');
                                    var menu = document.createElement('div');
                                    menu.id = '__sp_ctx_menu';
                                    menu.style.cssText = 'position:fixed;z-index:2147483647;background:#2c2d32;border:1px solid rgba(255,255,255,0.12);border-radius:10px;padding:4px;min-width:160px;box-shadow:0 8px 24px rgba(0,0,0,0.35);font-family:-apple-system,BlinkMacSystemFont,sans-serif;font-size:13px;color:#e4e5e9;';
                                    menu.style.left = Math.min(e.clientX, window.innerWidth - 180) + 'px';
                                    menu.style.top = Math.min(e.clientY, window.innerHeight - 200) + 'px';
                                    function item(label, fn){{
                                        var d = document.createElement('div');
                                        d.textContent = label;
                                        d.style.cssText = 'padding:7px 14px;border-radius:6px;cursor:pointer;transition:background 0.12s;';
                                        d.onmouseenter = function(){{ d.style.background='rgba(255,255,255,0.06)'; }};
                                        d.onmouseleave = function(){{ d.style.background='transparent'; }};
                                        d.onclick = function(){{ fn(); menu.remove(); }};
                                        menu.appendChild(d);
                                    }}
                                    if(sel) item('Copy', function(){{ navigator.clipboard.writeText(sel); }});
                                    item('Paste', function(){{ navigator.clipboard.readText().then(function(t){{ document.execCommand('insertText', false, t); }}); }});
                                    item('Select All', function(){{ document.execCommand('selectAll'); }});
                                    if(link) item('Open Link in New Tab', function(){{ window.__TAURI__.core.invoke('tab_create', {{url: link.href}}); }});
                                    if(img) item('Save Image As...', function(){{
                                        var a = document.createElement('a'); a.href = img.src; a.download = img.alt || 'image'; a.click();
                                    }});
                                    item('Reload', function(){{ window.location.reload(); }});
                                    document.body.appendChild(menu);
                                    var close = function(){{ menu.remove(); document.removeEventListener('click', close); }};
                                    setTimeout(function(){{ document.addEventListener('click', close); }}, 10);
                                }});
                            }}
                        }} }}, 300)"#
                    );
                    let _ = webview.eval(&js);

                    // YouTube Shorts auto-advance injection (works on any youtube.com page)
                    if url_str.contains("youtube.com") {
                        let shorts_js = r#"(function(){
                            if(window.__sp_shorts_injected) return;
                            window.__sp_shorts_injected = true;
                            window.__sp_shorts_auto = true;

                            function isShorts(){
                                if(location.pathname.startsWith('/shorts/')) return true;
                                if(document.querySelector('ytd-shorts,ytd-reel-video-renderer,#shorts-container')) return true;
                                return false;
                            }

                            function updateBtn(btn, on){
                                btn.textContent = on ? '\u23F8 Auto' : '\u25B6 Auto';
                                btn.style.background = on ? 'rgba(76,175,80,0.85)' : 'rgba(30,30,30,0.85)';
                                btn.style.borderColor = on ? 'rgba(76,175,80,0.5)' : 'rgba(255,255,255,0.15)';
                            }
                            function ensureBtn(){
                                var btn = document.getElementById('__sp_shorts_btn');
                                if(btn) return btn;
                                btn = document.createElement('button');
                                btn.id = '__sp_shorts_btn';
                                updateBtn(btn, window.__sp_shorts_auto);
                                btn.style.cssText = 'display:none;position:fixed;bottom:24px;right:20px;z-index:2147483646;padding:10px 18px;border-radius:24px;border:2px solid rgba(255,255,255,0.15);background:rgba(30,30,30,0.85);color:#fff;font-size:14px;font-weight:600;cursor:pointer;font-family:-apple-system,BlinkMacSystemFont,sans-serif;backdrop-filter:blur(12px);-webkit-backdrop-filter:blur(12px);box-shadow:0 4px 16px rgba(0,0,0,0.4);transition:all 0.2s ease;user-select:none;';
                                if(window.__sp_shorts_auto){
                                    btn.style.background = 'rgba(76,175,80,0.85)';
                                    btn.style.borderColor = 'rgba(76,175,80,0.5)';
                                }
                                btn.onmouseenter = function(){ btn.style.transform='scale(1.05)'; };
                                btn.onmouseleave = function(){ btn.style.transform='scale(1)'; };
                                btn.onclick = function(){
                                    window.__sp_shorts_auto = !window.__sp_shorts_auto;
                                    updateBtn(btn, window.__sp_shorts_auto);
                                };
                                document.body.appendChild(btn);
                                return btn;
                            }

                            function goNext(){
                                var nextBtn = document.querySelector('#navigation-button-down button')
                                    || document.querySelector('button[aria-label="Next video"]')
                                    || document.querySelector('button[aria-label="다음 동영상"]');
                                if(nextBtn){ nextBtn.click(); return; }
                                var container = document.querySelector('#shorts-container')
                                    || document.querySelector('ytd-shorts');
                                if(container){ container.scrollBy({top:window.innerHeight,behavior:'smooth'}); return; }
                                var target = document.querySelector('ytd-shorts') || document.activeElement || document.body;
                                target.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowDown',code:'ArrowDown',keyCode:40,bubbles:true,cancelable:true}));
                            }

                            setInterval(function(){
                                var onShorts = isShorts();
                                var btn = ensureBtn();
                                btn.style.display = onShorts ? 'block' : 'none';
                                if(!onShorts && window.__sp_shorts_auto){
                                    window.__sp_shorts_auto = false;
                                    updateBtn(btn, false);
                                }
                                if(onShorts && window.__sp_shorts_auto){
                                    var videos = document.querySelectorAll('video');
                                    var video = null;
                                    for(var i=0;i<videos.length;i++){
                                        if(!videos[i].paused && videos[i].duration > 0){ video = videos[i]; break; }
                                    }
                                    if(video && video.currentTime >= video.duration - 0.5){
                                        goNext();
                                    }
                                }
                            }, 400);
                        })()"#;
                        let _ = webview.eval(shorts_js);

                        // Fallback: skip any ads that slip through the initialization script
                        let adskip_fallback_js = r#"(function(){
                            if(window.__sp_adskip_fallback) return;
                            window.__sp_adskip_fallback = true;
                            setInterval(function(){
                                // Click skip buttons
                                var skipBtn = document.querySelector('.ytp-ad-skip-button-slot button,button.ytp-ad-skip-button,button.ytp-ad-skip-button-modern,.ytp-skip-ad-button,[class*="skip-button"]');
                                if(skipBtn && skipBtn.offsetParent !== null){ skipBtn.click(); return; }
                                var btns = document.querySelectorAll('button');
                                for(var i=0;i<btns.length;i++){
                                    var txt=btns[i].textContent.trim();
                                    if((txt.indexOf('건너뛰기')!==-1||txt.indexOf('Skip')!==-1)&&btns[i].offsetParent!==null){
                                        var p=btns[i].closest('.ytp-ad-module,.ytp-ad-player-overlay,[class*="ad-"]');
                                        if(p||document.querySelector('.ad-showing,.ad-interrupting')){btns[i].click();return;}
                                    }
                                }
                                // Fast-forward any playing ad
                                var adShowing=document.querySelector('.ad-showing,.ad-interrupting');
                                if(adShowing){
                                    var v=document.querySelector('video');
                                    if(v){v.muted=true;if(v.duration>0)v.currentTime=v.duration;}
                                }
                            }, 500);
                        })()"#;
                        let _ = webview.eval(adskip_fallback_js);
                    }

                    // Emit navigation event to frontend
                    let _ = app_handle.emit(
                        "tab-url-updated",
                        serde_json::json!({
                            "id": tab_id,
                            "url": &url_str
                        }),
                    );
                }
            });

            let ch = self.chrome_height;
            let win_size = win.inner_size().unwrap_or(tauri::PhysicalSize::new(1280, 800));
            let _ = win.add_child(
                builder,
                tauri::PhysicalPosition::new(0i32, ch as i32),
                tauri::PhysicalSize::new(
                    win_size.width,
                    win_size.height.saturating_sub(ch),
                ),
            );
        }

        // Emit tab-switched
        let _ = app.emit("tab-switched", serde_json::json!({
            "id": id,
            "url": &self.tabs.last().map(|t| t.url.clone()).unwrap_or_default(),
            "title": "New Tab",
            "favicon": ""
        }));

        Ok(id)
    }

    pub fn close_tab(&mut self, app: &AppHandle, id: u32) -> bool {
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            let tab = &self.tabs[idx];

            // Save to closed tabs
            if !tab.url.is_empty() && tab.url != "about:blank" {
                self.closed_tabs.push(ClosedTabInfo {
                    url: tab.url.clone(),
                    title: tab.title.clone(),
                });
                if self.closed_tabs.len() > self.max_closed_tabs {
                    self.closed_tabs.remove(0);
                }
            }

            // Remove from tabs list first (before closing webview)
            self.tabs.remove(idx);
            let _ = app.emit("tab-closed", serde_json::json!({ "id": id }));

            // Switch to another tab BEFORE closing the old webview
            if self.active_tab_id == Some(id) {
                if !self.tabs.is_empty() {
                    let new_idx = if idx < self.tabs.len() { idx } else { self.tabs.len() - 1 };
                    let new_active = self.tabs[new_idx].id;
                    self.active_tab_id = Some(new_active);
                    self.switch_tab(app, new_active);
                } else {
                    self.active_tab_id = None;
                }
            }

            // Move old webview off-screen first, then close with delay
            // (WebView2 relayouts remaining webviews when one is closed synchronously)
            let label = format!("tab-{}", id);
            if let Some(webview) = app.get_webview(&label) {
                let _ = webview.set_position(tauri::PhysicalPosition::new(-20000i32, -20000i32));
                let _ = webview.set_size(tauri::PhysicalSize::new(1u32, 1u32));
                // Schedule actual close after delay, then emit resize request
                let webview_label = label.clone();
                let app_clone = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if let Some(wv) = app_clone.get_webview(&webview_label) {
                        let _ = wv.close();
                    }
                    // After close, wait for WebView2 relayout then trigger resize
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let _ = app_clone.emit("_internal_resize", serde_json::json!({}));
                });
            }

            // Return true if no tabs remain (caller should create a new one)
            return self.tabs.is_empty();
        }
        false
    }

    pub fn switch_tab(&mut self, app: &AppHandle, id: u32) {
        if let Some(tab) = self.tabs.iter().find(|t| t.id == id) {
            self.active_tab_id = Some(id);

            // Hide all webviews, show the active one
            for t in &self.tabs {
                let label = format!("tab-{}", t.id);
                if let Some(webview) = app.get_webview(&label) {
                    if t.id == id {
                        let top = self.chrome_height + self.extra_offset;
                        let _ = webview.set_position(tauri::PhysicalPosition::new(0i32, top as i32));
                        // Make visible by setting proper size
                        let size_opt = app.get_webview_window("main")
                            .and_then(|win| win.inner_size().ok())
                            .or(self.last_window_size);
                        if let Some(size) = size_opt {
                            let _ = webview.set_size(tauri::PhysicalSize::new(
                                size.width,
                                size.height.saturating_sub(top),
                            ));
                        }
                    } else {
                        // Hide by moving off-screen
                        let _ = webview.set_position(tauri::PhysicalPosition::new(-10000i32, -10000i32));
                    }
                }
            }

            let _ = app.emit("tab-switched", serde_json::json!({
                "id": id,
                "url": &tab.url,
                "title": &tab.title,
                "favicon": &tab.favicon
            }));
        }
    }

    pub async fn restore_closed_tab(
        &mut self,
        app: &AppHandle,
    ) -> Result<Option<u32>, Box<dyn std::error::Error>> {
        if let Some(closed) = self.closed_tabs.pop() {
            let id = self.create_tab(app, Some(closed.url)).await?;
            return Ok(Some(id));
        }
        Ok(None)
    }

    pub fn switch_to_next_tab(&mut self, app: &AppHandle) {
        if self.tabs.len() <= 1 {
            return;
        }
        if let Some(active_id) = self.active_tab_id {
            if let Some(idx) = self.tabs.iter().position(|t| t.id == active_id) {
                let next_idx = (idx + 1) % self.tabs.len();
                let next_id = self.tabs[next_idx].id;
                self.switch_tab(app, next_id);
            }
        }
    }

    pub fn switch_to_prev_tab(&mut self, app: &AppHandle) {
        if self.tabs.len() <= 1 {
            return;
        }
        if let Some(active_id) = self.active_tab_id {
            if let Some(idx) = self.tabs.iter().position(|t| t.id == active_id) {
                let prev_idx = if idx == 0 { self.tabs.len() - 1 } else { idx - 1 };
                let prev_id = self.tabs[prev_idx].id;
                self.switch_tab(app, prev_id);
            }
        }
    }

    pub fn toggle_pin_tab(&mut self, app: &AppHandle, id: u32) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.pinned = !tab.pinned;
            let _ = app.emit("tab-pinned", serde_json::json!({
                "id": id,
                "pinned": tab.pinned
            }));
        }
    }

    pub fn move_tab(&mut self, app: &AppHandle, tab_id: u32, before_tab_id: Option<u32>) {
        let from_idx = self.tabs.iter().position(|t| t.id == tab_id);
        if let Some(from) = from_idx {
            let moved = self.tabs.remove(from);
            if let Some(before_id) = before_tab_id {
                if let Some(to) = self.tabs.iter().position(|t| t.id == before_id) {
                    self.tabs.insert(to, moved);
                } else {
                    self.tabs.push(moved);
                }
            } else {
                self.tabs.push(moved);
            }
            let order: Vec<u32> = self.tabs.iter().map(|t| t.id).collect();
            let _ = app.emit("tabs-reordered", serde_json::json!({ "order": order }));
        }
    }

    pub async fn navigate_to(
        &mut self,
        app: &AppHandle,
        url: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let resolved_url = Self::resolve_url(url);

        if let Some(active_id) = self.active_tab_id {
            // Update tab URL
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == active_id) {
                tab.url = resolved_url.clone();
            }

            let label = format!("tab-{}", active_id);
            if let Some(webview) = app.get_webview(&label) {
                let url_parsed: tauri::Url = resolved_url.parse().map_err(|e: url::ParseError| e.to_string())?;
                let _ = webview.navigate(url_parsed);
            }

            let _ = app.emit("tab-url-updated", serde_json::json!({
                "id": active_id,
                "url": &resolved_url
            }));
        }
        Ok(())
    }

    fn resolve_url(url: &str) -> String {
        if url.starts_with("http://") || url.starts_with("https://") {
            return url.to_string();
        }
        if url.contains('.') && !url.contains(' ') {
            return format!("https://{}", url);
        }
        format!(
            "https://www.google.com/search?q={}",
            urlencoding::encode(url)
        )
    }

    pub fn go_back(&self, app: &AppHandle) {
        if let Some(active_id) = self.active_tab_id {
            let label = format!("tab-{}", active_id);
            if let Some(webview) = app.get_webview(&label) {
                // Tauri v2 doesn't have direct back/forward; use JS
                let _ = webview.eval("window.history.back()");
            }
        }
    }

    pub fn go_forward(&self, app: &AppHandle) {
        if let Some(active_id) = self.active_tab_id {
            let label = format!("tab-{}", active_id);
            if let Some(webview) = app.get_webview(&label) {
                let _ = webview.eval("window.history.forward()");
            }
        }
    }

    pub fn reload(&self, app: &AppHandle) {
        if let Some(active_id) = self.active_tab_id {
            let label = format!("tab-{}", active_id);
            if let Some(webview) = app.get_webview(&label) {
                let _ = webview.eval("window.scrollTo(0,0);window.location.reload()");
            }
        }
    }

    pub async fn go_home(
        &mut self,
        app: &AppHandle,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let home = self.home_page.clone();
        self.navigate_to(app, &home).await
    }

    // Zoom via JS (WebView2 zoom is CSS transform)
    pub fn zoom_in(&mut self, app: &AppHandle) {
        if let Some(active_id) = self.active_tab_id {
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == active_id) {
                tab.zoom_factor = (tab.zoom_factor + 0.1).min(3.0);
                let zf = tab.zoom_factor;
                let label = format!("tab-{}", active_id);
                if let Some(webview) = app.get_webview(&label) {
                    let _ = webview.eval(&format!("document.body.style.zoom = '{}'", zf));
                }
                let zoom = (zf * 100.0).round() as u32;
                let _ = app.emit("zoom-changed", serde_json::json!({ "zoom": zoom }));
            }
        }
    }

    pub fn zoom_out(&mut self, app: &AppHandle) {
        if let Some(active_id) = self.active_tab_id {
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == active_id) {
                tab.zoom_factor = (tab.zoom_factor - 0.1).max(0.3);
                let zf = tab.zoom_factor;
                let label = format!("tab-{}", active_id);
                if let Some(webview) = app.get_webview(&label) {
                    let _ = webview.eval(&format!("document.body.style.zoom = '{}'", zf));
                }
                let zoom = (zf * 100.0).round() as u32;
                let _ = app.emit("zoom-changed", serde_json::json!({ "zoom": zoom }));
            }
        }
    }

    pub fn zoom_reset(&mut self, app: &AppHandle) {
        if let Some(active_id) = self.active_tab_id {
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == active_id) {
                tab.zoom_factor = 1.0;
                let label = format!("tab-{}", active_id);
                if let Some(webview) = app.get_webview(&label) {
                    let _ = webview.eval("document.body.style.zoom = '1'");
                }
                let _ = app.emit("zoom-changed", serde_json::json!({ "zoom": 100 }));
            }
        }
    }

    pub fn find_in_page(&self, app: &AppHandle, text: &str, forward: bool) {
        if let Some(active_id) = self.active_tab_id {
            let label = format!("tab-{}", active_id);
            if let Some(webview) = app.get_webview(&label) {
                // Use window.find() for basic find functionality
                let _ = webview.eval(&format!(
                    "window.find('{}', false, {}, true, false, true, false)",
                    text.replace('\'', "\\'").replace('\\', "\\\\"),
                    if forward { "false" } else { "true" }
                ));
            }
        }
    }

    pub fn stop_find(&self, app: &AppHandle) {
        if let Some(active_id) = self.active_tab_id {
            let label = format!("tab-{}", active_id);
            if let Some(webview) = app.get_webview(&label) {
                let _ = webview.eval("window.getSelection().removeAllRanges()");
            }
        }
    }

    pub fn list_tabs(&self) -> serde_json::Value {
        let tabs: Vec<serde_json::Value> = self
            .tabs
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "title": &t.title,
                    "url": &t.url,
                    "active": self.active_tab_id == Some(t.id),
                    "pinned": t.pinned,
                })
            })
            .collect();
        serde_json::json!({ "tabs": tabs })
    }

    pub fn get_active_tab(&self) -> Option<&Tab> {
        self.active_tab_id
            .and_then(|id| self.tabs.iter().find(|t| t.id == id))
    }

    pub fn get_active_tab_id(&self) -> Option<u32> {
        self.active_tab_id
    }

    pub async fn execute_js(
        &self,
        app: &AppHandle,
        code: &str,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        if let Some(active_id) = self.active_tab_id {
            let label = format!("tab-{}", active_id);
            if let Some(webview) = app.get_webview(&label) {
                match webview.eval(code) {
                    Ok(_) => {
                        return Ok(serde_json::json!({ "executed": true }));
                    }
                    Err(e) => {
                        return Ok(serde_json::json!({ "error": format!("{:?}", e) }));
                    }
                }
            }
        }
        Ok(serde_json::json!(null))
    }

    pub async fn translate_page(
        &self,
        app: &AppHandle,
        target_lang: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let lang_names: HashMap<&str, &str> = [
            ("ko", "한국어"),
            ("en", "English"),
            ("ja", "日本語"),
            ("zh-CN", "中文"),
            ("es", "Español"),
            ("fr", "Français"),
            ("de", "Deutsch"),
        ]
        .into_iter()
        .collect();

        let lang_label = lang_names.get(target_lang).unwrap_or(&target_lang);

        let script = format!(
            r#"(async function() {{
                var LANG = '{}';
                var LANG_LABEL = '{}';
                var SKIP = new Set(['SCRIPT','STYLE','NOSCRIPT','CODE','PRE','TEXTAREA','INPUT','SVG','MATH','KBD']);
                var prev = document.getElementById('__sp_translate_bar');
                if (prev) {{ prev.querySelector('.__sp_close').click(); }}
                var nodes = [];
                function walk(el) {{
                    for (var i = 0; i < el.childNodes.length; i++) {{
                        var n = el.childNodes[i];
                        if (n.nodeType === 3) {{
                            var t = n.textContent.trim();
                            if (t.length >= 2) nodes.push({{ node: n, original: n.textContent }});
                        }} else if (n.nodeType === 1 && !SKIP.has(n.tagName) && n.id !== '__sp_translate_bar') {{
                            walk(n);
                        }}
                    }}
                }}
                walk(document.body);
                if (nodes.length === 0) return;
                var bar = document.createElement('div');
                bar.id = '__sp_translate_bar';
                bar.style.cssText = 'position:fixed;top:0;left:0;right:0;height:34px;background:#25262b;border-bottom:2px solid rgba(124,92,252,0.4);display:flex;align-items:center;padding:0 14px;gap:10px;z-index:2147483647;font-family:-apple-system,BlinkMacSystemFont,sans-serif;font-size:13px;color:#e4e5e9;box-shadow:0 2px 8px rgba(0,0,0,0.3);';
                var status = document.createElement('span');
                status.textContent = LANG_LABEL + ' 번역 중...';
                status.style.color = '#7c5cfc';
                bar.appendChild(status);
                var spacer = document.createElement('div');
                spacer.style.flex = '1';
                bar.appendChild(spacer);
                var closeBtn = document.createElement('button');
                closeBtn.className = '__sp_close';
                closeBtn.style.cssText = 'background:none;border:1px solid rgba(255,255,255,0.1);color:#8b8d93;cursor:pointer;font-size:12px;padding:4px 10px;border-radius:6px;font-family:inherit;';
                closeBtn.textContent = '원문 보기';
                closeBtn.onclick = function() {{
                    nodes.forEach(function(e) {{ e.node.textContent = e.original; }});
                    bar.remove();
                    document.body.style.marginTop = oldMargin;
                }};
                bar.appendChild(closeBtn);
                var oldMargin = document.body.style.marginTop || '';
                document.body.prepend(bar);
                document.body.style.marginTop = (parseInt(oldMargin || '0') + 34) + 'px';
                var CHUNK = 40;
                var done = 0;
                for (var i = 0; i < nodes.length; i += CHUNK) {{
                    var chunk = nodes.slice(i, i + CHUNK);
                    var texts = chunk.map(function(e) {{ return e.original.trim(); }});
                    var joined = texts.join('\\n');
                    try {{
                        var resp = await fetch('https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl=' + LANG + '&dt=t&dj=1&q=' + encodeURIComponent(joined));
                        var data = await resp.json();
                        if (data.sentences) {{
                            var full = data.sentences.filter(function(s) {{ return s.trans; }}).map(function(s) {{ return s.trans; }}).join('');
                            var parts = full.split('\\n');
                            chunk.forEach(function(el, idx) {{
                                if (parts[idx] !== undefined && parts[idx] !== '') {{
                                    el.node.textContent = el.original.replace(el.original.trim(), parts[idx]);
                                }}
                            }});
                        }}
                    }} catch (err) {{ console.error('Translate chunk error:', err); }}
                    done += chunk.length;
                    status.textContent = LANG_LABEL + ' 번역 중... (' + Math.min(done, nodes.length) + '/' + nodes.length + ')';
                }}
                status.textContent = LANG_LABEL + '로 번역 완료';
                status.style.color = '#5ce0d8';
            }})()"#,
            target_lang, lang_label
        );

        self.execute_js(app, &script).await?;
        Ok(())
    }

    /// Update tab metadata (called from URL change events)
    pub fn update_tab_url(&mut self, id: u32, url: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.url = url.to_string();
        }
    }

    pub fn update_tab_title(&mut self, id: u32, title: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.title = title.to_string();
        }
    }

    pub fn update_tab_favicon(&mut self, id: u32, favicon: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.favicon = favicon.to_string();
        }
    }

    pub fn set_chrome_height(&mut self, height: u32) {
        self.chrome_height = height;
    }

    pub fn get_chrome_height(&self) -> u32 {
        self.chrome_height
    }

    pub fn get_last_window_size(&self) -> Option<tauri::PhysicalSize<u32>> {
        self.last_window_size
    }

    pub fn resize_active_tab(&mut self, app: &AppHandle) {
        // Try get_webview_window first, fallback to stored size
        let size_opt = if let Some(win) = app.get_webview_window("main") {
            win.inner_size().ok()
        } else {
            // Window lookup fails in async command context - use stored size
            self.last_window_size
        };
        if let Some(size) = size_opt {
            self.resize_active_tab_with_size(app, size);
        }
    }

    pub fn resize_active_tab_with_size(&mut self, app: &AppHandle, size: tauri::PhysicalSize<u32>) {
        // Store size for use when window lookup fails in async context
        self.last_window_size = Some(size);
        // Don't resize if tab is fully hidden (e.g. panel overlay is open)
        if self.tab_hidden {
            return;
        }
        if let Some(active_id) = self.active_tab_id {
            let label = format!("tab-{}", active_id);
            if let Some(webview) = app.get_webview(&label) {
                let top = self.chrome_height + self.extra_offset;
                let _ = webview.set_position(tauri::PhysicalPosition::new(0i32, top as i32));
                let _ = webview.set_size(tauri::PhysicalSize::new(
                    size.width,
                    size.height.saturating_sub(top),
                ));
            }
        }
    }

    pub fn open_devtools(&self, app: &AppHandle) {
        if let Some(active_id) = self.active_tab_id {
            let label = format!("tab-{}", active_id);
            if let Some(webview) = app.get_webview(&label) {
                webview.open_devtools();
            }
        }
    }

    pub fn get_active_webview(&self, app: &AppHandle) -> Option<tauri::Webview> {
        if let Some(active_id) = self.active_tab_id {
            let label = format!("tab-{}", active_id);
            app.get_webview(&label)
        } else {
            None
        }
    }

    /// Show an overlay webview on top of tab content (for menus/popups)
    pub fn show_menu_overlay(&self, app: &AppHandle, width: u32, height: u32) {
        // Close existing overlay first
        self.hide_menu_overlay(app);

        if let Some(win) = app.get_window("main") {
            let builder = tauri::webview::WebviewBuilder::new(
                "menu-overlay",
                tauri::WebviewUrl::App("menu-overlay.html".into()),
            )
            .transparent(true);

            let _ = win.add_child(
                builder,
                tauri::PhysicalPosition::new(0i32, 0i32),
                tauri::PhysicalSize::new(width, height),
            );
        }
    }

    /// Hide the menu overlay webview
    pub fn hide_menu_overlay(&self, app: &AppHandle) {
        if let Some(wv) = app.get_webview("menu-overlay") {
            let _ = wv.close();
        }
    }

    /// Push active tab webview down by extra_top pixels to make room for popup menus,
    /// or restore to normal position when extra_top is 0.
    /// Using 9999 as a special value to hide the tab completely (for full-page overlays).
    pub fn set_tab_offset(&mut self, app: &AppHandle, extra_top: u32) {
        if extra_top == 9999 {
            // Hide completely (for full-page panels like history/downloads)
            self.tab_hidden = true;
            if let Some(active_id) = self.active_tab_id {
                let label = format!("tab-{}", active_id);
                if let Some(webview) = app.get_webview(&label) {
                    let _ = webview.set_position(tauri::PhysicalPosition::new(-20000i32, -20000i32));
                }
            }
        } else {
            self.tab_hidden = false;
            self.extra_offset = extra_top;
            self.resize_active_tab(app);
        }
    }
}
