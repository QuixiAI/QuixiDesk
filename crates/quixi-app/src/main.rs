//! `QuixiDesk` — Native Cross-Platform Rust Desktop Application (`tao` + `wry` + Tokio + Rayon)
//! Zero Node.js, zero Electron processes. Serves `ion-dist` over `app://localhost` and bridges
//! all 1,400 `$eipc_message$` channels to `QuixiEngine` & `EipcRouter`.

use quixi_engine::QuixiEngine;
use quixi_ipc::EipcRouter;
use quixi_proto::EipcInvokePayload;
use serde_json::Value;
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tao::{
    dpi::{LogicalPosition, LogicalSize},
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::{
    http::{header::CONTENT_TYPE, Response, StatusCode},
    WebViewBuilder,
};

#[cfg(target_os = "macos")]
use tao::platform::macos::WindowBuilderExtMacOS;
#[cfg(target_os = "macos")]
use wry::WebViewExtMacOS;

/// Installs a native macOS `NSMenu` bar (`QuixiDesk`, `Edit`, `Window`) on `NSApp` so that
/// standard AppKit responder selectors (`copy:` Cmd+C, `paste:` Cmd+V, `cut:` Cmd+X,
/// `selectAll:` Cmd+A, `undo:` Cmd+Z, `redo:` Cmd+Shift+Z, `terminate:` Cmd+Q, `hide:` Cmd+H,
/// `performClose:` Cmd+W, `performMiniaturize:` Cmd+M) are routed through the First Responder
/// (`WKWebView`).
#[cfg(target_os = "macos")]
#[allow(deprecated, unexpected_cfgs)]
fn setup_macos_main_menu() {
    use cocoa::appkit::{NSApp, NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSAutoreleasePool, NSString};
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let app = NSApp();
        if app == nil {
            return;
        }

        let main_menu = NSMenu::new(nil).autorelease();

        // 1. App Menu ("QuixiDesk")
        let app_menu_item = NSMenuItem::new(nil).autorelease();
        main_menu.addItem_(app_menu_item);
        let app_menu = NSMenu::alloc(nil)
            .initWithTitle_(NSString::alloc(nil).init_str("QuixiDesk"))
            .autorelease();

        let hide_title = NSString::alloc(nil).init_str("Hide QuixiDesk");
        let hide_key = NSString::alloc(nil).init_str("h");
        let hide_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(hide_title, sel!(hide:), hide_key)
            .autorelease();
        app_menu.addItem_(hide_item);

        let hide_others_title = NSString::alloc(nil).init_str("Hide Others");
        let hide_others_key = NSString::alloc(nil).init_str("h");
        let hide_others_item: id = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                hide_others_title,
                sel!(hideOtherApplications:),
                hide_others_key,
            )
            .autorelease();
        let cmd_opt = NSEventModifierFlags::NSCommandKeyMask
            | NSEventModifierFlags::NSAlternateKeyMask;
        let _: () = msg_send![hide_others_item, setKeyEquivalentModifierMask: cmd_opt];
        app_menu.addItem_(hide_others_item);

        let show_all_title = NSString::alloc(nil).init_str("Show All");
        let empty_key = NSString::alloc(nil).init_str("");
        let show_all_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                show_all_title,
                sel!(unhideAllApplications:),
                empty_key,
            )
            .autorelease();
        app_menu.addItem_(show_all_item);

        app_menu.addItem_(NSMenuItem::separatorItem(nil));

        let quit_title = NSString::alloc(nil).init_str("Quit QuixiDesk");
        let quit_key = NSString::alloc(nil).init_str("q");
        let quit_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(quit_title, sel!(terminate:), quit_key)
            .autorelease();
        app_menu.addItem_(quit_item);
        app_menu_item.setSubmenu_(app_menu);

        // 2. Edit Menu ("Edit") — required by macOS AppKit / WKWebView for Cmd+C, Cmd+V, Cmd+X, Cmd+A, Cmd+Z
        let edit_menu_item = NSMenuItem::new(nil).autorelease();
        main_menu.addItem_(edit_menu_item);
        let edit_menu = NSMenu::alloc(nil)
            .initWithTitle_(NSString::alloc(nil).init_str("Edit"))
            .autorelease();

        let undo_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                NSString::alloc(nil).init_str("Undo"),
                sel!(undo:),
                NSString::alloc(nil).init_str("z"),
            )
            .autorelease();
        edit_menu.addItem_(undo_item);

        let redo_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                NSString::alloc(nil).init_str("Redo"),
                sel!(redo:),
                NSString::alloc(nil).init_str("Z"),
            )
            .autorelease();
        edit_menu.addItem_(redo_item);

        edit_menu.addItem_(NSMenuItem::separatorItem(nil));

        let cut_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                NSString::alloc(nil).init_str("Cut"),
                sel!(cut:),
                NSString::alloc(nil).init_str("x"),
            )
            .autorelease();
        edit_menu.addItem_(cut_item);

        let copy_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                NSString::alloc(nil).init_str("Copy"),
                sel!(copy:),
                NSString::alloc(nil).init_str("c"),
            )
            .autorelease();
        edit_menu.addItem_(copy_item);

        let paste_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                NSString::alloc(nil).init_str("Paste"),
                sel!(paste:),
                NSString::alloc(nil).init_str("v"),
            )
            .autorelease();
        edit_menu.addItem_(paste_item);

        let select_all_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                NSString::alloc(nil).init_str("Select All"),
                sel!(selectAll:),
                NSString::alloc(nil).init_str("a"),
            )
            .autorelease();
        edit_menu.addItem_(select_all_item);

        edit_menu_item.setSubmenu_(edit_menu);

        // 3. Window Menu ("Window")
        let window_menu_item = NSMenuItem::new(nil).autorelease();
        main_menu.addItem_(window_menu_item);
        let window_menu = NSMenu::alloc(nil)
            .initWithTitle_(NSString::alloc(nil).init_str("Window"))
            .autorelease();

        let min_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                NSString::alloc(nil).init_str("Minimize"),
                sel!(performMiniaturize:),
                NSString::alloc(nil).init_str("m"),
            )
            .autorelease();
        window_menu.addItem_(min_item);

        let close_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                NSString::alloc(nil).init_str("Close Window"),
                sel!(performClose:),
                NSString::alloc(nil).init_str("w"),
            )
            .autorelease();
        window_menu.addItem_(close_item);
        window_menu_item.setSubmenu_(window_menu);

        app.setMainMenu_(main_menu);
    }
}

#[derive(Debug, Clone)]
enum UserEvent {
    EmitIpc {
        channel: String,
        payload: Value,
    },
    DragWindow,
    ToggleMaximize,
    MinimizeWindow,
    CloseWindow,
    CaptureSnapshot {
        tx: std::sync::mpsc::Sender<Result<Vec<u8>, String>>,
    },
    AutomationEval {
        script: String,
        tx: std::sync::mpsc::Sender<String>,
    },
}

#[cfg(target_os = "macos")]
#[allow(deprecated, unexpected_cfgs)]
fn capture_wkwebview_snapshot_macos(
    webview: &wry::WebView,
    tx: std::sync::mpsc::Sender<Result<Vec<u8>, String>>,
) {
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};

    let tx_cell = std::sync::Mutex::new(Some(tx));
    let handler = block::ConcreteBlock::new(move |ns_image: id, ns_error: id| unsafe {
        if ns_image != nil && ns_error == nil {
            let tiff_data: id = msg_send![ns_image, TIFFRepresentation];
            if tiff_data != nil {
                let bitmap_rep: id =
                    msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff_data];
                if bitmap_rep != nil {
                    let empty_dict: id = msg_send![class!(NSDictionary), dictionary];
                    // NSPNGFileType = 4usize
                    let png_data: id = msg_send![
                        bitmap_rep,
                        representationUsingType: 4usize
                        properties: empty_dict
                    ];
                    if png_data != nil {
                        let len: usize = msg_send![png_data, length];
                        let ptr: *const u8 = msg_send![png_data, bytes];
                        if !ptr.is_null() && len > 0 {
                            let bytes = std::slice::from_raw_parts(ptr, len).to_vec();
                            if let Ok(mut guard) = tx_cell.lock() {
                                if let Some(sender) = guard.take() {
                                    let _ = sender.send(Ok(bytes));
                                }
                            }
                            return;
                        }
                    }
                }
            }
        }
        if let Ok(mut guard) = tx_cell.lock() {
            if let Some(sender) = guard.take() {
                let _ = sender.send(Err("WKWebView takeSnapshotWithConfiguration failed".into()));
            }
        }
    });
    let handler = handler.copy();
    unsafe {
        let retained_wv = webview.webview();
        let wk_webview: id = (&*retained_wv) as *const _ as id;
        let nil_cfg: id = nil;
        let _: () = msg_send![
            wk_webview,
            takeSnapshotWithConfiguration: nil_cfg
            completionHandler: &*handler
        ];
    }
}

fn unwrap_webview_eval_json(raw: &str) -> Value {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return serde_json::json!({ "ok": true });
    }
    if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
        if let Some(inner_str) = val.as_str() {
            if let Ok(inner_val) = serde_json::from_str::<Value>(inner_str) {
                return inner_val;
            }
        }
        return val;
    }
    serde_json::json!({ "ok": true, "raw": trimmed })
}

fn start_automation_http_server(proxy: tao::event_loop::EventLoopProxy<UserEvent>) {
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::Duration;

        let addr = "127.0.0.1:19455";
        let mut listener_opt = None;
        for _ in 0..5 {
            match TcpListener::bind(addr) {
                Ok(l) => {
                    listener_opt = Some(l);
                    break;
                }
                Err(_) => std::thread::sleep(Duration::from_millis(200)),
            }
        }
        let Some(listener) = listener_opt else {
            tracing::warn!("Could not bind QuixiDesk automation bridge on {}", addr);
            return;
        };
        tracing::info!("QuixiDesk native UI & screenshot bridge listening on http://{}", addr);

        for stream in listener.incoming().flatten() {
            let proxy = proxy.clone();
            std::thread::spawn(move || {
                let mut stream = stream;
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

                let mut buf = Vec::with_capacity(8192);
                let mut temp = [0u8; 4096];
                let mut header_end = None;
                let mut content_length = 0usize;

                loop {
                    match stream.read(&mut temp) {
                        Ok(0) => break,
                        Ok(n) => {
                            buf.extend_from_slice(&temp[..n]);
                            if header_end.is_none() {
                                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                                    let hdr_end = pos + 4;
                                    header_end = Some(hdr_end);
                                    let hdr_str = String::from_utf8_lossy(&buf[..hdr_end]);
                                    for line in hdr_str.lines() {
                                        if let Some((k, v)) = line.split_once(':') {
                                            if k.trim().eq_ignore_ascii_case("content-length") {
                                                content_length = v.trim().parse().unwrap_or(0);
                                            }
                                        }
                                    }
                                }
                            }
                            if let Some(hdr_end) = header_end {
                                if buf.len() >= hdr_end + content_length {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }

                if buf.is_empty() {
                    return;
                }

                let hdr_end = header_end.unwrap_or(buf.len());
                let req_header = String::from_utf8_lossy(&buf[..hdr_end]);
                let first_line = req_header.lines().next().unwrap_or("");
                let mut parts = first_line.split_whitespace();
                let method = parts.next().unwrap_or("GET").to_uppercase();
                let path = parts.next().unwrap_or("/");
                let body_bytes = if hdr_end <= buf.len() {
                    &buf[hdr_end..]
                } else {
                    &[][..]
                };

                let write_resp =
                    |stream: &mut std::net::TcpStream, status: &str, ct: &str, body: &[u8]| {
                        let hdr = format!(
                            "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
                            status,
                            ct,
                            body.len()
                        );
                        let _ = stream.write_all(hdr.as_bytes());
                        let _ = stream.write_all(body);
                        let _ = stream.flush();
                    };

                if method == "GET" && path.starts_with("/screenshot") {
                    let (tx, rx) = std::sync::mpsc::channel();
                    if proxy.send_event(UserEvent::CaptureSnapshot { tx }).is_ok() {
                        if let Ok(Ok(png_bytes)) = rx.recv_timeout(Duration::from_secs(4)) {
                            if let Some(home) = dirs::home_dir() {
                                let snap_path = home.join(".quixi/last_snapshot.png");
                                let _ = fs::write(&snap_path, &png_bytes);
                            }
                            write_resp(&mut stream, "200 OK", "image/png", &png_bytes);
                            return;
                        }
                    }
                    write_resp(
                        &mut stream,
                        "500 Internal Server Error",
                        "application/json",
                        b"{\"ok\":false,\"error\":\"snapshot_failed\"}",
                    );
                    return;
                }

                if method == "GET" && path.starts_with("/state") {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let script = "window.__quixi_automation__ ? JSON.stringify(window.__quixi_automation__.getState()) : JSON.stringify({ok:false})".to_string();
                    if proxy
                        .send_event(UserEvent::AutomationEval { script, tx })
                        .is_ok()
                    {
                        if let Ok(raw) = rx.recv_timeout(Duration::from_secs(4)) {
                            let val = unwrap_webview_eval_json(&raw);
                            let out = serde_json::to_vec_pretty(&val).unwrap_or_default();
                            write_resp(&mut stream, "200 OK", "application/json", &out);
                            return;
                        }
                    }
                    write_resp(
                        &mut stream,
                        "500 Internal Server Error",
                        "application/json",
                        b"{\"ok\":false,\"error\":\"eval_timeout\"}",
                    );
                    return;
                }

                if method == "POST" && path.starts_with("/action") {
                    let payload: Value =
                        serde_json::from_slice(body_bytes).unwrap_or_else(|_| serde_json::json!({}));
                    let payload_str =
                        serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
                    let script = format!(
                        "window.__quixi_automation__ ? JSON.stringify(window.__quixi_automation__.performAction({})) : JSON.stringify({{ok:false}})",
                        payload_str
                    );
                    let (tx, rx) = std::sync::mpsc::channel();
                    if proxy
                        .send_event(UserEvent::AutomationEval { script, tx })
                        .is_ok()
                    {
                        if let Ok(raw) = rx.recv_timeout(Duration::from_secs(4)) {
                            let val = unwrap_webview_eval_json(&raw);
                            let out = serde_json::to_vec_pretty(&val).unwrap_or_default();
                            write_resp(&mut stream, "200 OK", "application/json", &out);
                            return;
                        }
                    }
                    write_resp(
                        &mut stream,
                        "500 Internal Server Error",
                        "application/json",
                        b"{\"ok\":false,\"error\":\"action_timeout\"}",
                    );
                    return;
                }

                write_resp(
                    &mut stream,
                    "200 OK",
                    "application/json",
                    b"{\"ok\":true,\"service\":\"quixi-automation-bridge\"}",
                );
            });
        }
    });
}

fn locate_resources_dir() -> PathBuf {
    if let Ok(env_dir) = std::env::var("QUIXI_RESOURCES_DIR") {
        let p = PathBuf::from(env_dir);
        if p.exists() {
            return p;
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            // macOS .app bundle: QuixiDesk.app/Contents/MacOS/QuixiDesk -> ../Resources
            let bundle_res = exe_dir.join("../Resources");
            if bundle_res.join("ion-dist/index.html").exists() {
                return bundle_res.canonicalize().unwrap_or(bundle_res);
            }
            // Cargo target/release/QuixiDesk -> ../../resources
            let workspace_res = exe_dir.join("../../resources");
            if workspace_res.join("ion-dist/index.html").exists() {
                return workspace_res.canonicalize().unwrap_or(workspace_res);
            }
        }
    }

    let fallback = PathBuf::from(quixi_proto::paths::workspace_join("resources"));
    if fallback.exists() {
        return fallback;
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("resources")
}

fn guess_mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" | "cjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "wasm" => "application/wasm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    }
}

fn build_http_response(
    status: u16,
    content_type: &str,
    body: Vec<u8>,
) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(StatusCode::from_u16(status).unwrap_or(StatusCode::OK))
        .header(CONTENT_TYPE, content_type)
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, PUT, PATCH, DELETE, OPTIONS")
        .header("Access-Control-Allow-Headers", "*")
        .header("Cache-Control", "no-cache")
        .body(Cow::Owned(body))
        .unwrap()
}

fn serve_static_or_spa(resources_dir: &Path, req_path: &str) -> Response<Cow<'static, [u8]>> {
    let clean_path = req_path.trim_start_matches('/');
    let ion_dist = resources_dir.join("ion-dist");
    let renderer_dir = resources_dir.join("renderer");

    // 1. Direct static file in ion-dist
    if !clean_path.is_empty() {
        let candidate = ion_dist.join(clean_path);
        if candidate.is_file() {
            if let Ok(bytes) = fs::read(&candidate) {
                return build_http_response(200, guess_mime_type(&candidate), bytes);
            }
        }
        // 2. Direct static file in renderer/ (about.html, quick-window.html, find-in-page.html, etc.)
        let renderer_candidate = renderer_dir.join(clean_path);
        if renderer_candidate.is_file() {
            if let Ok(bytes) = fs::read(&renderer_candidate) {
                return build_http_response(200, guess_mime_type(&renderer_candidate), bytes);
            }
        }
        // 3. Direct static file in resources/ root (e.g. fonts/, en-US.json)
        let root_candidate = resources_dir.join(clean_path);
        if root_candidate.is_file() {
            if let Ok(bytes) = fs::read(&root_candidate) {
                return build_http_response(200, guess_mime_type(&root_candidate), bytes);
            }
        }
    }

    // 4. Missing asset extension (.js, .css, .png, .woff2, .svg, .map) -> 404 instead of HTML fallback
    if let Some(ext) = Path::new(clean_path).extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_ascii_lowercase();
        if matches!(
            ext_lower.as_str(),
            "js" | "css" | "png" | "jpg" | "svg" | "woff2" | "woff" | "ttf" | "wasm" | "ico" | "map"
        ) {
            return build_http_response(404, "text/plain; charset=utf-8", b"Not Found".to_vec());
        }
    }

    // 5. SPA fallback -> ion-dist/index.html for all client routes (/new, /chat/*, /cowork*, /code*, /settings*, etc.)
    let index_html = ion_dist.join("index.html");
    match fs::read(&index_html) {
        Ok(bytes) => build_http_response(200, "text/html; charset=utf-8", bytes),
        Err(err) => build_http_response(
            500,
            "text/plain; charset=utf-8",
            format!("Failed to load index.html: {}", err).into_bytes(),
        ),
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Support multi-call invocation when QuixiDesk binary is invoked as a helper
    if let Some(arg1) = std::env::args().nth(1) {
        if arg1 == "--version" || arg1 == "-v" {
            println!("QuixiDesk 3.6556.2");
            return Ok(());
        }
    }

    let resources_dir = locate_resources_dir();
    tracing::info!("QuixiDesk booting with resources_dir: {:?}", resources_dir);

    // Start background Tokio runtime for async engine operations
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _rt_guard = tokio_rt.enter();

    let engine = QuixiEngine::new(resources_dir.clone());
    let router = Arc::new(EipcRouter::new(engine.clone()));

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    #[cfg(target_os = "macos")]
    setup_macos_main_menu();

    // Wire server-push `$eipc_message$` events from QuixiEngine to the WebView
    let emit_proxy = proxy.clone();
    engine.set_emitter(Arc::new(move |channel: &str, payload: Value| {
        let _ = emit_proxy.send_event(UserEvent::EmitIpc {
            channel: channel.to_string(),
            payload,
        });
    }));

    // Build native OS window matching Claude.app dimensions and titlebar inset
    let mut win_builder = WindowBuilder::new()
        .with_title("QuixiDesk")
        .with_inner_size(LogicalSize::new(1280.0, 840.0))
        .with_min_inner_size(LogicalSize::new(480.0, 360.0));

    #[cfg(target_os = "macos")]
    {
        win_builder = win_builder
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true)
            .with_title_hidden(true)
            .with_traffic_light_inset(LogicalPosition::new(16.0, 16.0));
    }

    let window = win_builder.build(&event_loop)?;

    // Read combined Electron-free preload bridge
    let preload_path = resources_dir.join("preload/combined_preload.js");
    let mut init_script = fs::read_to_string(&preload_path).unwrap_or_else(|e| {
        tracing::error!("Failed to read {:?}: {}", preload_path, e);
        String::new()
    });

    // Append native window drag, titlebar double-click, and diagnostic telemetry bridge
    init_script.push_str(
        r#"
;(function() {
  var origFetch = window.fetch;
  function reportToRust(kind, data) {
    try {
      origFetch('app://localhost/__eipc__/diag', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ kind: kind, href: location.href, data: data })
      }).catch(function(){});
    } catch (_) {}
  }

  window.addEventListener('error', function(e) {
    reportToRust('window.error', {
      message: String(e.message || ''),
      filename: String(e.filename || ''),
      lineno: e.lineno,
      colno: e.colno
    });
  });

  window.addEventListener('unhandledrejection', function(e) {
    var reason = e.reason;
    reportToRust('unhandledrejection', {
      message: reason && reason.message ? String(reason.message) : String(reason),
      stack: reason && reason.stack ? String(reason.stack).slice(0, 800) : null
    });
  });

  var origConsoleError = console.error;
  console.error = function() {
    try {
      var args = Array.prototype.slice.call(arguments).map(function(a) {
        if (a instanceof Error) return a.message + '\n' + (a.stack || '').slice(0, 600);
        if (typeof a === 'object' && a !== null) {
          try { return JSON.stringify(a).slice(0, 400); } catch (_) { return String(a); }
        }
        return String(a);
      });
      reportToRust('console.error', { args: args });
    } catch (_) {}
    return origConsoleError.apply(console, arguments);
  };

  function checkMountStatus(tag) {
    var root = document.getElementById('root');
    var rootChildren = root ? root.children.length : 0;
    var readyAttr = document.documentElement.getAttribute('data-boot-ui-ready');
    var textSample = document.body ? (document.body.innerText || '').replace(/\s+/g, ' ').trim().slice(0, 400) : '';
    reportToRust('mount_status', {
      tag: tag || 'initial',
      rootChildren: rootChildren,
      bootUiReady: readyAttr,
      title: document.title,
      pathname: location.pathname,
      textSample: textSample
    });
  }

  setTimeout(function() { checkMountStatus('boot_2.5s'); }, 2500);
})();
"#,
    );

    if std::env::var("QUIXI_E2E_TEST").is_ok() {
        init_script.push_str(
            r#"
;(function() {
  window.__QUIXI_E2E_MODE__ = true;
  setTimeout(async function() {
    try {
      var stRes = await fetch('app://localhost/api/quixi/providers');
      var st = await stRes.json();
      var top5 = (st.providers || []).slice(0, 5).map(function(p) { return p.id; });
      var overlayEl = document.getElementById('quixi-provider-overlay');
      var provLogoSvgs = overlayEl ? overlayEl.querySelectorAll('.qp-prov-svg[data-provider-logo]').length : 0;
      var brandBadgeSvg = document.querySelector('#qp-brand-badge svg circle');
      fetch('app://localhost/__eipc__/diag', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          kind: 'e2e_provider_first_load_check',
          overlayShownOnFirstLoad: !!overlayEl,
          providerLogoSvgCount: provLogoSvgs,
          quixiBrandBadgeHasSvg: !!brandBadgeSvg,
          top5Providers: top5,
          totalProviders: (st.providers || []).length,
          onboardingCompleted: st.onboardingCompleted
        })
      }).catch(function(){});
      if (overlayEl) {
        var contBtn = document.getElementById('qp-continue-btn');
        if (contBtn) contBtn.click();
      }
    } catch (e) {}
  }, 1200);

  function nav(path, tag) {
    try {
      window.history.pushState({}, '', path);
      window.dispatchEvent(new PopStateEvent('popstate'));
      setTimeout(function() {
        var fullText = document.body ? (document.body.innerText || '').replace(/\s+/g, ' ').trim() : '';
        var textSample = fullText.slice(0, 400);
        var textTail = fullText.length > 400 ? fullText.slice(-500) : fullText;
        fetch('app://localhost/__eipc__/diag', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            kind: 'e2e_route_check',
            tag: tag,
            pathname: location.pathname,
            title: document.title,
            textSample: textSample,
            textTail: textTail
          })
        }).catch(function(){});
      }, 900);
    } catch (e) {}
  }

  setTimeout(function() { nav('/new', 'route_new'); }, 1800);
  setTimeout(function() { nav('/epitaxy', 'route_code_epitaxy'); }, 2500);
  setTimeout(function() { nav('/projects', 'route_projects'); }, 3200);
  setTimeout(function() { nav('/scheduled-task', 'route_scheduled'); }, 3900);
  setTimeout(function() { nav('/customize', 'route_customize'); }, 4600);
  setTimeout(function() { nav('/recents', 'route_recents'); }, 5300);
  setTimeout(async function() {
    try {
      var cw = window['claude.web'] || (window.claude && window.claude.web);
      var sidObj = await cw.LocalAgentModeSessions.start({
        message: 'Check git status in workspace',
        sessionType: 'cowork',
        userSelectedFolders: ['__QUIXI_WORKSPACE__']
      });
      var sid = (sidObj && sidObj.sessionId) ? sidObj.sessionId : String(sidObj);
      nav('/cowork/' + sid, 'route_cowork_session');
      var cidObj = await cw.LocalSessions.start({
        prompt: 'List files in workspace',
        cwd: '__QUIXI_WORKSPACE__'
      });
      var cid = (cidObj && cidObj.sessionId) ? cidObj.sessionId : String(cidObj);
      setTimeout(async function() {
        var agentTranscript = await cw.LocalAgentModeSessions.getTranscript(sid);
        var codeTranscript = await cw.LocalSessions.getTranscript(cid);
        var skills = await cw.LocalAgentModeSessions.listLocalSkills();
        var docxFiles = await cw.LocalAgentModeSessions.getLocalSkillFiles('docx');
        var gitInfo = await cw.LocalSessions.getGitInfo('__QUIXI_WORKSPACE__');
        var gitCommits = await cw.LocalSessions.getGitCommits('__QUIXI_WORKSPACE__');
        var shells = await cw.LocalSessions.getTerminalShells();
        var csim = window['claude.simulator'] || (window.claude && window.claude.simulator);
        var simDevices = (csim && csim.Simulator) ? await csim.Simulator.listDevices() : [];
        var serverCandidates = await cw.Launch.detectServerCandidates('__QUIXI_WORKSPACE__');
        await cw.CoworkMemory.writeGlobalMemory('# QuixiDesk Global Memory\n');
        var mem = await cw.CoworkMemory.readGlobalMemory();
        var cs = window['claude.settings'] || (window.claude && window.claude.settings);
        var prefs = cs ? await cs.AppPreferences.getPreferences() : null;
        var sysInfo = cs ? await cs.DesktopInfo.getSystemInfo() : null;
        var localMcp = await cw.LocalAgentModeSessions.getLocalMcpServers();
        var directMcp = await cw.LocalAgentModeSessions.getDirectMcpServerStatuses();
        var cuPos = await cw.LocalAgentModeSessions.directMcpCallTool('b0a3b6e5-7ca0-462a-8e6f-bac087408b17', 'cursor_position', {});
        var chromeTabs = await cw.LocalAgentModeSessions.directMcpCallTool('a8f3c7e2-4b9d-4f1a-8c3e-9d2a5b7f8e1c', 'tabs_context_mcp', { createIfEmpty: true });
        var tccState = cw.ComputerUseTcc ? await cw.ComputerUseTcc.getState() : null;
        var tccApps = cw.ComputerUseTcc ? await cw.ComputerUseTcc.listInstalledApps() : [];
        var chromeExts = cw.ChromeExtension ? await cw.ChromeExtension.getConnectedExtensions() : [];
        var quixiLogos = Array.prototype.slice.call(document.querySelectorAll('svg path')).filter(function(p) {
          return (p.getAttribute('fill') || '').toUpperCase().indexOf('18C9B0') !== -1;
        }).length;
        var claudePaths = Array.prototype.slice.call(document.querySelectorAll('svg path[d]')).filter(function(p) {
          if (p.closest && p.closest('#quixi-provider-overlay')) return false;
          var d = p.getAttribute('d') || '';
          return d.indexOf('M16.597') === 0 || d.indexOf('M13.827') === 0 || d.indexOf('M20.328') === 0 || d.indexOf('M17.304') === 0 || d.indexOf('M46.187') === 0 || d.indexOf('M123.516') === 0;
        }).length;
        fetch('app://localhost/__eipc__/diag', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            kind: 'e2e_sessions_check',
            agentSessionId: sid,
            agentTranscriptLen: agentTranscript ? agentTranscript.length : -1,
            codeSessionId: cid,
            codeTranscriptLen: codeTranscript ? codeTranscript.length : -1,
            skillsCount: skills ? skills.length : -1,
            docxFilesCount: docxFiles ? docxFiles.length : -1,
            gitBranch: gitInfo ? gitInfo.branch : null,
            gitCommitsCount: gitCommits ? gitCommits.length : -1,
            shellsCount: (shells && shells.shells) ? shells.shells.length : -1,
            simDevicesCount: simDevices ? simDevices.length : -1,
            serverCandidatesCount: serverCandidates ? serverCandidates.length : -1,
            globalMemoryLen: mem ? mem.length : -1,
            prefsMenuBarEnabled: prefs ? prefs.menuBarEnabled : null,
            sysInfoPlatform: sysInfo ? sysInfo.platform : null,
            localMcpCount: localMcp ? localMcp.length : -1,
            directMcpCount: directMcp ? directMcp.length : -1,
            computerUseCursorPos: (cuPos && cuPos.content && cuPos.content[0]) ? cuPos.content[0].text : null,
            browserUseTabsContext: (chromeTabs && chromeTabs.content && chromeTabs.content[0]) ? chromeTabs.content[0].text.slice(0, 120) : null,
            tccAccessibility: tccState ? tccState.accessibility : null,
            tccAppsCount: tccApps ? tccApps.length : -1,
            chromeExtsCount: chromeExts ? chromeExts.length : -1,
            workspaceQuixiLogosCount: quixiLogos,
            workspaceClaudeOrAnthropicLogosCount: claudePaths
          })
        }).catch(function(){});
      }, 800);
    } catch (err) {
      fetch('app://localhost/__eipc__/diag', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ kind: 'e2e_sessions_err', error: String(err) })
      }).catch(function(){});
    }
  }, 6000);
  setTimeout(function() { nav('/settings', 'route_settings'); }, 7400);
})();
"#,
        );
    }

    init_script.push_str(
        r#"
;(function() {
  var origFetch = window.fetch;

  // Native OS Clipboard & Keyboard Edit Bridge (Cmd+C, Cmd+X, navigator.clipboard.writeText/readText)
  function writeNativeClipboard(text) {
    if (typeof text !== 'string') text = String(text == null ? '' : text);
    try {
      return origFetch('app://localhost/__eipc__/clipboard', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action: 'write', text: text })
      }).then(function() {}).catch(function() {});
    } catch (_) {
      return Promise.resolve();
    }
  }

  function readNativeClipboard() {
    try {
      return origFetch('app://localhost/__eipc__/clipboard', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action: 'read' })
      }).then(function(r) { return r.json(); }).then(function(d) {
        return (d && typeof d.text === 'string') ? d.text : '';
      }).catch(function() { return ''; });
    } catch (_) {
      return Promise.resolve('');
    }
  }

  window.__quixi_copy_text__ = writeNativeClipboard;
  window.__quixi_read_clipboard__ = readNativeClipboard;

  try {
    var origClipboard = navigator.clipboard;
    var origWriteText = origClipboard && typeof origClipboard.writeText === 'function'
      ? origClipboard.writeText.bind(origClipboard)
      : null;
    var origReadText = origClipboard && typeof origClipboard.readText === 'function'
      ? origClipboard.readText.bind(origClipboard)
      : null;

    var customClipboard = {
      writeText: function(text) {
        var s = String(text == null ? '' : text);
        if (origWriteText) {
          try { origWriteText(s).catch(function() {}); } catch (_) {}
        }
        return writeNativeClipboard(s);
      },
      readText: function() {
        return readNativeClipboard().then(function(t) {
          if (t) return t;
          if (origReadText) {
            return origReadText().catch(function() { return ''; });
          }
          return '';
        });
      }
    };

    if (origClipboard) {
      try {
        origClipboard.writeText = customClipboard.writeText;
        origClipboard.readText = customClipboard.readText;
      } catch (_) {}
    }
    try {
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        get: function() { return origClipboard || customClipboard; }
      });
    } catch (_) {}
  } catch (_) {}

  function getSelectedDomText() {
    try {
      var active = document.activeElement;
      if (active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA')) {
        if (typeof active.selectionStart === 'number' && typeof active.selectionEnd === 'number' && active.selectionStart !== active.selectionEnd) {
          return String(active.value || '').substring(active.selectionStart, active.selectionEnd);
        }
      }
      var sel = window.getSelection ? window.getSelection() : null;
      if (sel && sel.rangeCount > 0) {
        var str = sel.toString();
        if (str) return str;
      }
    } catch (_) {}
    return '';
  }

  document.addEventListener('copy', function() {
    var sel = getSelectedDomText();
    if (sel) writeNativeClipboard(sel);
  }, true);

  document.addEventListener('cut', function() {
    var sel = getSelectedDomText();
    if (sel) writeNativeClipboard(sel);
  }, true);

  window.addEventListener('keydown', function(e) {
    if (!(e.metaKey || e.ctrlKey) || e.altKey) return;
    var k = String(e.key || '').toLowerCase();
    if (!e.shiftKey && k === 'c') {
      var sel = getSelectedDomText();
      if (sel) {
        writeNativeClipboard(sel);
      }
    } else if (!e.shiftKey && k === 'x') {
      var cutSel = getSelectedDomText();
      if (cutSel) {
        writeNativeClipboard(cutSel);
      }
    }
  }, true);

  document.addEventListener('mousedown', function(e) {
    if (e.button !== 0) return;
    var el = e.target;
    if (!el || typeof el.closest !== 'function') return;
    if (el.closest('.draggable-none, button, a, input, textarea, select, [role="button"], [role="menuitem"], [role="tab"], [contenteditable="true"]')) {
      return;
    }
    if (el.closest('.draggable, .df-titlebar, #desktop-boot-drag-strip')) {
      try {
        origFetch('app://localhost/__eipc__/window-action', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ action: e.detail === 2 ? 'toggle_maximize' : 'drag' })
        }).catch(function(){});
      } catch (_) {}
    }
  }, false);

  // QuixiDesk Native Computer Use & Accessibility Automation Bridge
  function collectInteractiveElements() {
    var nodes = Array.prototype.slice.call(
      document.querySelectorAll('button, a, input, textarea, select, [role="button"], [role="tab"], [role="menuitem"], [role="option"], [contenteditable="true"]')
    );
    var out = [];
    var idx = 1;
    for (var i = 0; i < nodes.length; i++) {
      var el = nodes[i];
      if (!el || !el.getBoundingClientRect) continue;
      var rect = el.getBoundingClientRect();
      if (rect.width < 2 || rect.height < 2) continue;
      var style = window.getComputedStyle ? window.getComputedStyle(el) : null;
      if (style && (style.display === 'none' || style.visibility === 'hidden' || parseFloat(style.opacity || '1') === 0)) {
        continue;
      }
      var hiddenAncestor = false;
      var anc = el.parentElement;
      while (anc && anc !== document.body) {
        var ancStyle = window.getComputedStyle ? window.getComputedStyle(anc) : null;
        if (ancStyle && (ancStyle.display === 'none' || ancStyle.visibility === 'hidden' || parseFloat(ancStyle.opacity || '1') === 0)) {
          hiddenAncestor = true;
          break;
        }
        anc = anc.parentElement;
      }
      if (hiddenAncestor) continue;
      var tag = (el.tagName || '').toLowerCase();
      var role = el.getAttribute('role') || (
        tag === 'button' ? 'AXButton' :
        tag === 'a' ? 'AXLink' :
        (tag === 'input' || tag === 'textarea') ? 'AXTextField' : 'AXElement'
      );
      var label = (
        el.getAttribute('aria-label') ||
        el.getAttribute('title') ||
        el.getAttribute('placeholder') ||
        el.innerText ||
        el.textContent ||
        el.value ||
        ''
      ).replace(/\s+/g, ' ').trim().slice(0, 120);
      el.setAttribute('data-quixi-ax-idx', String(idx));
      out.push({
        index: idx,
        tag: tag,
        role: role,
        text: label,
        placeholder: el.getAttribute('placeholder') || '',
        x: Math.round(rect.left),
        y: Math.round(rect.top),
        width: Math.round(rect.width),
        height: Math.round(rect.height),
        centerX: Math.round(rect.left + rect.width / 2),
        centerY: Math.round(rect.top + rect.height / 2)
      });
      idx++;
      if (out.length >= 120) break;
    }
    return out;
  }

  window.__quixi_automation__ = {
    getState: function() {
      var els = collectInteractiveElements();
      var textSample = document.body
        ? (document.body.innerText || '').replace(/\s+/g, ' ').trim().slice(0, 1200)
        : '';
      return {
        ok: true,
        title: document.title || 'QuixiDesk',
        url: location.href,
        pathname: location.pathname,
        width: window.innerWidth,
        height: window.innerHeight,
        elementsCount: els.length,
        elements: els,
        textSample: textSample
      };
    },
    performAction: function(payload) {
      payload = payload || {};
      var act = String(payload.action || 'click').toLowerCase();
      var els = collectInteractiveElements();
      var targetEl = null;

      var elIdx = typeof payload.element_index === 'number' ? payload.element_index : (typeof payload.index === 'number' ? payload.index : 0);
      if (elIdx > 0) {
        targetEl = document.querySelector('[data-quixi-ax-idx="' + elIdx + '"]');
      }
      if (!targetEl && payload.selector) {
        try { targetEl = document.querySelector(payload.selector); } catch (_) {}
      }
      var queryStr = payload.query || (act !== 'type' && act !== 'key' ? payload.text : null);
      if (!targetEl && queryStr) {
        var q = String(queryStr).toLowerCase();
        for (var i = 0; i < els.length; i++) {
          if ((els[i].text || '').toLowerCase().indexOf(q) !== -1) {
            targetEl = document.querySelector('[data-quixi-ax-idx="' + els[i].index + '"]');
            break;
          }
        }
      }

      if (act === 'navigate') {
        var dest = String(payload.path || payload.url || '/new');
        window.history.pushState({}, '', dest);
        window.dispatchEvent(new PopStateEvent('popstate'));
        return { ok: true, action: 'navigate', path: location.pathname };
      }

      if (act === 'eval' || act === 'evaluate') {
        try {
          var codeStr = String(payload.script || payload.expression || payload.code || 'document.title');
          var evalRes = (0, eval)(codeStr);
          return { ok: true, action: 'eval', result: evalRes };
        } catch (err) {
          return { ok: false, action: 'eval', error: String(err) };
        }
      }

      if (act === 'click' || act === 'left_click' || act === 'double_click' || act === 'right_click') {
        var x = typeof payload.x === 'number' ? payload.x : 640;
        var y = typeof payload.y === 'number' ? payload.y : 400;
        if (targetEl) {
          var r = targetEl.getBoundingClientRect();
          x = Math.round(r.left + r.width / 2);
          y = Math.round(r.top + r.height / 2);
        } else {
          targetEl = document.elementFromPoint(x, y);
        }
        if (targetEl) {
          var clickable = (typeof targetEl.closest === 'function' && targetEl.closest('button, a, [role="button"], [role="tab"], [role="menuitem"], [role="option"], input, textarea, [contenteditable="true"]')) || targetEl;
          try {
            var evOpts = { bubbles: true, cancelable: true, view: window, clientX: x, clientY: y, button: 0 };
            clickable.dispatchEvent(new PointerEvent('pointerdown', evOpts));
            clickable.dispatchEvent(new MouseEvent('mousedown', evOpts));
            if (typeof clickable.focus === 'function') clickable.focus();
            clickable.dispatchEvent(new PointerEvent('pointerup', evOpts));
            clickable.dispatchEvent(new MouseEvent('mouseup', evOpts));
            if (typeof clickable.click === 'function') {
              clickable.click();
            } else {
              clickable.dispatchEvent(new MouseEvent('click', evOpts));
            }
          } catch (_) {
            try { if (typeof clickable.click === 'function') clickable.click(); } catch (_) {}
          }
          var desc = (clickable.getAttribute && (clickable.getAttribute('aria-label') || clickable.innerText || clickable.tagName)) || '';
          return {
            ok: true,
            action: act,
            x: x,
            y: y,
            clickedTag: (clickable.tagName || '').toLowerCase(),
            clickedText: String(desc).replace(/\s+/g, ' ').trim().slice(0, 80),
            pathname: location.pathname
          };
        }
        return { ok: false, action: act, x: x, y: y, error: 'no_element_at_point' };
      }

      if (act === 'type') {
        var txt = String(payload.text || '');
        if (!targetEl) {
          var active = document.activeElement;
          if (active && (active.tagName === 'INPUT' || active.tagName === 'TEXTAREA' || active.getAttribute('contenteditable') === 'true')) {
            targetEl = active;
          } else {
            targetEl = document.querySelector('textarea, [contenteditable="true"], input[type="text"], input:not([type="hidden"])');
          }
        }
        if (targetEl) {
          if (typeof targetEl.focus === 'function') targetEl.focus();
          if (targetEl.getAttribute && targetEl.getAttribute('contenteditable') === 'true') {
            var inserted = false;
            try {
              document.execCommand('selectAll', false, null);
              inserted = document.execCommand('insertText', false, txt);
            } catch (_) {}
            if (!inserted) {
              targetEl.textContent = txt;
              targetEl.dispatchEvent(new InputEvent('input', { bubbles: true, data: txt, inputType: 'insertText' }));
            }
          } else if (targetEl.tagName === 'TEXTAREA') {
            var taSetter = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value');
            if (taSetter && taSetter.set) taSetter.set.call(targetEl, txt);
            else targetEl.value = txt;
            targetEl.dispatchEvent(new Event('input', { bubbles: true }));
            targetEl.dispatchEvent(new Event('change', { bubbles: true }));
          } else {
            var inSetter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value');
            if (inSetter && inSetter.set) inSetter.set.call(targetEl, txt);
            else targetEl.value = txt;
            targetEl.dispatchEvent(new Event('input', { bubbles: true }));
            targetEl.dispatchEvent(new Event('change', { bubbles: true }));
          }
          return { ok: true, action: 'type', length: txt.length, tag: (targetEl.tagName || '').toLowerCase() };
        }
        return { ok: false, action: 'type', error: 'no_input_element' };
      }

      if (act === 'key') {
        var rawCombo = String(payload.key || payload.combo || payload.text || 'Enter');
        var lower = rawCombo.toLowerCase();
        var keyName = lower === 'return' || lower === 'enter' ? 'Enter' :
                      lower === 'escape' || lower === 'esc' ? 'Escape' :
                      lower === 'tab' ? 'Tab' :
                      lower === 'backspace' || lower === 'delete' ? 'Backspace' :
                      rawCombo.split('+').pop() || 'Enter';
        var keyTarget = document.activeElement || document.body;
        var kev = {
          key: keyName,
          code: keyName === 'Enter' ? 'Enter' : keyName === 'Escape' ? 'Escape' : 'Key' + keyName.toUpperCase(),
          bubbles: true,
          cancelable: true,
          metaKey: lower.indexOf('cmd') !== -1 || lower.indexOf('meta') !== -1,
          ctrlKey: lower.indexOf('ctrl') !== -1,
          shiftKey: lower.indexOf('shift') !== -1,
          altKey: lower.indexOf('alt') !== -1 || lower.indexOf('opt') !== -1
        };
        keyTarget.dispatchEvent(new KeyboardEvent('keydown', kev));
        keyTarget.dispatchEvent(new KeyboardEvent('keyup', kev));
        return { ok: true, action: 'key', key: keyName };
      }

      return { ok: true, action: act };
    }
  };
})();
"#,
    );

    init_script.push_str(include_str!("provider_ui.js"));

    // The E2E harness needs the live workspace root, not a baked-in developer path.
    init_script = init_script.replace(
        "__QUIXI_WORKSPACE__",
        quixi_proto::paths::default_workspace_str(),
    );

    let protocol_resources = resources_dir.clone();
    let protocol_engine = engine.clone();
    let protocol_router = router.clone();
    let protocol_proxy = proxy.clone();

    let user_agent = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) QuixiDesk/3.6556.2 Claude/3.6556.2 Electron/44.9.3 Safari/605.1.15";

    let webview = WebViewBuilder::new()
        .with_user_agent(user_agent)
        .with_initialization_script(&init_script)
        .with_devtools(true)
        .with_clipboard(true)
        .with_custom_protocol("app".into(), move |_webview_id, request| {
            let method = request.method().as_str().to_uppercase();
            let uri = request.uri();
            let path = uri.path();
            let path_and_query = uri
                .path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or(path);

            if method == "OPTIONS" {
                return build_http_response(204, "text/plain", Vec::new());
            }

            // 1. Native Window Actions, Clipboard & Diagnostics
            if path == "/__eipc__/diag" {
                if let Ok(val) = serde_json::from_slice::<Value>(request.body()) {
                    tracing::info!("[WebView Diag] {}", val);
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                }
                return build_http_response(200, "application/json", b"{\"ok\":true}".to_vec());
            }

            if path == "/__eipc__/clipboard" {
                if let Ok(body_val) = serde_json::from_slice::<Value>(request.body()) {
                    match body_val.get("action").and_then(|v| v.as_str()).unwrap_or("") {
                        "write" => {
                            let text = body_val
                                .get("text")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let _ = quixi_platform::ComputerUse::write_clipboard(text);
                            return build_http_response(
                                200,
                                "application/json; charset=utf-8",
                                b"{\"ok\":true}".to_vec(),
                            );
                        }
                        "read" => {
                            let text = quixi_platform::ComputerUse::read_clipboard();
                            let resp = serde_json::json!({ "ok": true, "text": text });
                            return build_http_response(
                                200,
                                "application/json; charset=utf-8",
                                serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec()),
                            );
                        }
                        _ => {}
                    }
                }
                return build_http_response(200, "application/json", b"{\"ok\":true}".to_vec());
            }

            if path == "/__eipc__/window-action" {
                if let Ok(body_val) = serde_json::from_slice::<Value>(request.body()) {
                    match body_val.get("action").and_then(|v| v.as_str()).unwrap_or("") {
                        "drag" => {
                            let _ = protocol_proxy.send_event(UserEvent::DragWindow);
                        }
                        "toggle_maximize" => {
                            let _ = protocol_proxy.send_event(UserEvent::ToggleMaximize);
                        }
                        "minimize" => {
                            let _ = protocol_proxy.send_event(UserEvent::MinimizeWindow);
                        }
                        "close" => {
                            let _ = protocol_proxy.send_event(UserEvent::CloseWindow);
                        }
                        _ => {}
                    }
                }
                return build_http_response(200, "application/json", b"{\"ok\":true}".to_vec());
            }

            // 2. `$eipc_message$` RPC & Sync Store Bridge (`/__eipc__/invoke`, `/__eipc__/sync`, `/__eipc__/send`)
            if path.starts_with("/__eipc__/") {
                match serde_json::from_slice::<EipcInvokePayload>(request.body()) {
                    Ok(payload) => {
                        if payload.channel.contains("WindowControl_$_minimize") {
                            let _ = protocol_proxy.send_event(UserEvent::MinimizeWindow);
                        } else if payload.channel.contains("WindowControl_$_maximize") {
                            let _ = protocol_proxy.send_event(UserEvent::ToggleMaximize);
                        } else if payload.channel.contains("WindowControl_$_close") {
                            let _ = protocol_proxy.send_event(UserEvent::CloseWindow);
                        }
                        let reply = protocol_router.dispatch(&payload);
                        let bytes = serde_json::to_vec(&reply).unwrap_or_else(|_| b"{}".to_vec());
                        return build_http_response(200, "application/json; charset=utf-8", bytes);
                    }
                    Err(err) => {
                        let err_json = serde_json::json!({
                            "error": { "message": format!("Invalid EIPC payload: {}", err) }
                        });
                        return build_http_response(
                            400,
                            "application/json; charset=utf-8",
                            serde_json::to_vec(&err_json).unwrap_or_default(),
                        );
                    }
                }
            }

            // 3. Local REST & SSE API (`/api/*`, `/edge-api/*`, `/plugins/*`, `/marketplaces/*`, `/v1/*`, `/v2/*`, `/dust/*`)
            if path.starts_with("/api/")
                || path == "/api"
                || path.starts_with("/edge-api/")
                || path == "/edge-api"
                || path.starts_with("/plugins/")
                || path.starts_with("/marketplaces/")
                || path.starts_with("/v1/")
                || path.starts_with("/v2/")
                || path.starts_with("/dust")
            {
                let (status, content_type, body) =
                    protocol_engine.handle_api_request(&method, path_and_query, request.body());
                tracing::info!("[API] {} {} -> {}", method, path_and_query, status);
                return build_http_response(status, &content_type, body);
            }

            // 4. Static assets & SPA fallback (`ion-dist`, `renderer`, `fonts`)
            serve_static_or_spa(&protocol_resources, path)
        })
        .with_custom_protocol("cowork-artifact".into(), move |_webview_id, _request| {
            build_http_response(
                200,
                "text/html; charset=utf-8",
                b"<!doctype html><html><body></body></html>".to_vec(),
            )
        })
        .with_url("app://localhost/")
        .build(&window)?;

    start_automation_http_server(proxy.clone());

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {
                #[cfg(target_os = "macos")]
                setup_macos_main_menu();
                tracing::info!("QuixiDesk native window initialized (app://localhost/)");
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(user_evt) => match user_evt {
                UserEvent::EmitIpc { channel, payload } => {
                    if let (Ok(ch_str), Ok(pay_str)) = (
                        serde_json::to_string(&channel),
                        serde_json::to_string(&payload),
                    ) {
                        let script = format!(
                            "if (typeof window.__quixi_eipc_emit__ === 'function') {{ window.__quixi_eipc_emit__({}, {}); }}",
                            ch_str, pay_str
                        );
                        let _ = webview.evaluate_script(&script);
                    }
                }
                UserEvent::DragWindow => {
                    let _ = window.drag_window();
                }
                UserEvent::ToggleMaximize => {
                    window.set_maximized(!window.is_maximized());
                }
                UserEvent::MinimizeWindow => {
                    window.set_minimized(true);
                }
                UserEvent::CloseWindow => {
                    *control_flow = ControlFlow::Exit;
                }
                UserEvent::CaptureSnapshot { tx } => {
                    #[cfg(target_os = "macos")]
                    {
                        capture_wkwebview_snapshot_macos(&webview, tx);
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        let _ = tx.send(Err("Snapshot only supported on macOS".into()));
                    }
                }
                UserEvent::AutomationEval { script, tx } => {
                    let tx_cb = tx.clone();
                    if webview
                        .evaluate_script_with_callback(&script, move |res| {
                            let _ = tx_cb.send(res);
                        })
                        .is_err()
                    {
                        let _ = tx.send("{\"ok\":false}".to_string());
                    }
                }
            },
            _ => {}
        }
    });
}
