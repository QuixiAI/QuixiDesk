//! `QuixiDesk` — Native Cross-Platform Rust Desktop Application (`tao` + `wry` + Tokio + Rayon)
//! Zero Node.js, zero Electron processes. Serves the original interface over `app://localhost` and bridges
//! desktop service and event channels to `QuixiEngine` & `EipcRouter`.

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

// WebView2 maps Wry's custom scheme onto a virtual HTTP host.
#[cfg(target_os = "windows")]
const APP_ORIGIN: &str = "http://app.localhost";
#[cfg(not(target_os = "windows"))]
const APP_ORIGIN: &str = "app://localhost";

fn trusted_app_origin(origin: Option<&http::HeaderValue>) -> bool {
    origin.is_none_or(|value| value == APP_ORIGIN)
}

fn exit_event_loop(control_flow: &mut ControlFlow) {
    *control_flow = ControlFlow::Exit;
    #[cfg(target_os = "windows")]
    {
        // Tao waits for the redraw batch to finish before honoring Exit.
        // A WebView2 host on an inactive desktop may never receive WM_PAINT.
        // Called on the UI thread, WM_QUIT lets its native message pump finish
        // normally, including LoopDestroyed, without waiting for a repaint.
        #[link(name = "user32")]
        extern "system" { fn PostQuitMessage(exit_code: i32); }
        unsafe { PostQuitMessage(0); }
    }
}

#[cfg(test)]
mod origin_tests {
    use super::*;

    #[test]
    fn only_the_platform_app_origin_can_access_native_routes() {
        assert!(trusted_app_origin(None));
        assert!(trusted_app_origin(Some(&http::HeaderValue::from_static(APP_ORIGIN))));
        for origin in ["null", "https://example.com", "http://app.localhost.evil", "http://localhost:19458"] {
            assert!(!trusted_app_origin(Some(&http::HeaderValue::from_str(origin).unwrap())));
        }
        let response = build_http_response(200, "application/json", b"{}".to_vec());
        assert_eq!(response.headers()["Access-Control-Allow-Origin"], APP_ORIGIN);
    }
}

#[cfg(target_os = "macos")]
use tao::platform::macos::WindowBuilderExtMacOS;
#[cfg(target_os = "macos")]
use wry::WebViewExtMacOS;
#[cfg(target_os = "macos")]
use wry::WebViewBuilderExtDarwin;

#[cfg(target_os = "macos")]
static MENU_PROXY: std::sync::OnceLock<std::sync::Mutex<tao::event_loop::EventLoopProxy<UserEvent>>> = std::sync::OnceLock::new();

#[cfg(target_os = "macos")]
extern "C" fn open_settings_menu(_: &objc::runtime::Object, _: objc::runtime::Sel, _: cocoa::base::id) {
    if let Some(proxy) = MENU_PROXY.get() { let _ = proxy.lock().unwrap().send_event(UserEvent::OpenSettings); }
}

#[cfg(target_os = "macos")]
extern "C" fn quit_menu(_: &objc::runtime::Object, _: objc::runtime::Sel, _: cocoa::base::id) {
    if let Some(proxy) = MENU_PROXY.get() { let _ = proxy.lock().unwrap().send_event(UserEvent::CloseWindow); }
}

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

        let about = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            NSString::alloc(nil).init_str("About QuixiDesk"), sel!(orderFrontStandardAboutPanel:),
            NSString::alloc(nil).init_str("")).autorelease();
        app_menu.addItem_(about);
        let cls = objc::runtime::Class::get("QuixiDeskMenuActions").unwrap_or_else(|| {
            let mut decl = objc::declare::ClassDecl::new("QuixiDeskMenuActions", objc::class!(NSObject)).unwrap();
            decl.add_method(sel!(openSettings:), open_settings_menu as extern "C" fn(&objc::runtime::Object, objc::runtime::Sel, id));
            decl.add_method(sel!(quitSafely:), quit_menu as extern "C" fn(&objc::runtime::Object, objc::runtime::Sel, id));
            decl.register()
        });
        let target: id = msg_send![cls, new];
        let settings = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(
            NSString::alloc(nil).init_str("Settings…"), sel!(openSettings:),
            NSString::alloc(nil).init_str(",")).autorelease();
        let _: () = msg_send![settings, setTarget: target];
        app_menu.addItem_(settings);
        app_menu.addItem_(NSMenuItem::separatorItem(nil));

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
            .initWithTitle_action_keyEquivalent_(quit_title, sel!(quitSafely:), quit_key)
            .autorelease();
        let _: () = msg_send![quit_item, setTarget: target];
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

/// Present an app-owned sheet, without AppleScript/Finder automation permissions.
#[cfg(target_os = "macos")]
#[allow(deprecated, unexpected_cfgs)]
fn show_folder_sheet(window: &tao::window::Window, prompt: &str, multiple: bool,
    tx: std::sync::mpsc::Sender<Vec<String>>) {
    use cocoa::appkit::{NSApp, NSApplication};
    use cocoa::base::{id, nil, YES, NO};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};
    use tao::platform::macos::WindowExtMacOS;
    unsafe {
        let parent = window.ns_window() as id;
        let existing: id = msg_send![parent, attachedSheet];
        if existing != nil { let _ = tx.send(vec![]); return; }
        let panel: id = msg_send![class!(NSOpenPanel), openPanel];
        let _: () = msg_send![panel, setCanChooseDirectories: YES];
        let _: () = msg_send![panel, setCanChooseFiles: NO];
        let _: () = msg_send![panel, setAllowsMultipleSelection: if multiple { YES } else { NO }];
        let _: () = msg_send![panel, setCanCreateDirectories: YES];
        let message = NSString::alloc(nil).init_str(prompt);
        let _: () = msg_send![panel, setMessage: message];
        let _: () = msg_send![message, release];
        NSApp().activateIgnoringOtherApps_(YES);
        let panel_address = panel as usize;
        let handler = block::ConcreteBlock::new(move |response: isize| {
            let mut paths = vec![];
            if response == 1 {
                let urls: id = msg_send![panel_address as id, URLs];
                let count: usize = msg_send![urls, count];
                for index in 0..count {
                    let url: id = msg_send![urls, objectAtIndex: index];
                    let path: id = msg_send![url, path];
                    let utf8: *const std::ffi::c_char = msg_send![path, UTF8String];
                    if !utf8.is_null() { paths.push(std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned()); }
                }
            }
            let _ = tx.send(paths);
        }).copy();
        let _: () = msg_send![panel, beginSheetModalForWindow: parent completionHandler: &*handler];
    }
}

#[derive(Debug, Clone)]
enum UserEvent {
    OpenSettings,
    EmitIpc {
        channel: String,
        payload: Value,
    },
    PickFolders {
        prompt: String,
        multiple: bool,
        tx: std::sync::mpsc::Sender<Vec<String>>,
    },
    DragWindow,
    ToggleMaximize,
    MinimizeWindow,
    CloseWindow,
    #[cfg(target_os = "linux")]
    WriteClipboard {
        text: String,
        tx: std::sync::mpsc::Sender<Result<(), String>>,
    },
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

fn start_automation_http_server(proxy: tao::event_loop::EventLoopProxy<UserEvent>) -> anyhow::Result<()> {
    let addr = format!("127.0.0.1:{}", quixi_proto::paths::automation_port());
    // Fail startup rather than attach to another process's automation bridge.
    let listener = std::net::TcpListener::bind(&addr)?;
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        use std::time::Duration;

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
                // The local automation bridge is for native clients, never web content.
                if req_header.lines().any(|line| line.split_once(':').is_some_and(|(key, _)| key.trim().eq_ignore_ascii_case("origin"))) {
                    let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    return;
                }
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
    Ok(())
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
            let portable = exe_dir.join("resources");
            if portable.join("ui/index.html").exists() { return portable; }
            // macOS .app bundle: QuixiDesk.app/Contents/MacOS/QuixiDesk -> ../Resources
            let bundle_res = exe_dir.join("../Resources");
            if bundle_res.join("ui/index.html").exists() {
                return bundle_res.canonicalize().unwrap_or(bundle_res);
            }
            // Cargo target/release/QuixiDesk -> ../../resources
            let workspace_res = exe_dir.join("../../resources");
            if workspace_res.join("ui/index.html").exists() {
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
        .header("Access-Control-Allow-Origin", APP_ORIGIN)
        .header("Access-Control-Allow-Methods", "GET, POST, PUT, PATCH, DELETE, OPTIONS")
        .header("Access-Control-Allow-Headers", "*")
        .header("Cache-Control", "no-cache")
        .body(Cow::Owned(body))
        .unwrap()
}

fn serve_static_or_spa(resources_dir: &Path, req_path: &str) -> Response<Cow<'static, [u8]>> {
    let clean_path = req_path.trim_start_matches('/');
    if Path::new(clean_path).components().any(|part| matches!(part, std::path::Component::ParentDir | std::path::Component::Prefix(_))) {
        return build_http_response(404, "text/plain", b"Not found".to_vec());
    }
    let ui_dir = resources_dir.join("ui");

    // 1. Direct static file in the UI directory
    if !clean_path.is_empty() {
        let candidate = ui_dir.join(clean_path);
        if candidate.is_file() {
            if let Ok(bytes) = fs::read(&candidate) {
                return build_http_response(200, guess_mime_type(&candidate), bytes);
            }
        }
        // 3. Direct static file in resources/ root (e.g. icons and local libraries)
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

    // 5. SPA fallback -> ui/index.html for all client routes (/new, /chat/*, /cowork*, /code*, /settings*, etc.)
    let index_html = ui_dir.join("index.html");
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
            println!("QuixiDesk 3.6556.7");
            return Ok(());
        }
    }

    let args: Vec<String> = std::env::args().collect();
    let option_value = |name: &str| -> anyhow::Result<Option<String>> {
        let mut matches = args.iter().enumerate().filter(|(_, arg)| arg.as_str() == name);
        let Some((index, _)) = matches.next() else { return Ok(None); };
        if matches.next().is_some() { anyhow::bail!("Duplicate {name}"); }
        let value = args.get(index + 1).filter(|value| !value.starts_with("--") && !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("{name} requires a path"))?;
        Ok(Some(value.clone()))
    };
    let mock_account = option_value("--mock-account")?;
    let test_profile = option_value("--test-profile")?
        .or_else(|| std::env::var("QUIXI_TEST_PROFILE").ok());
    if let Some(root) = &test_profile {
        #[cfg(target_os = "macos")]
        {
            let version = std::process::Command::new("/usr/bin/sw_vers").arg("-productVersion").output()?;
            let major: u32 = String::from_utf8_lossy(&version.stdout).split('.').next().unwrap_or("").trim().parse()?;
            if major < 14 { anyhow::bail!("Isolated test web storage requires macOS 14 or later"); }
        }
        quixi_proto::paths::configure_test_profile(Path::new(root)).map_err(anyhow::Error::msg)?;
        let profile = quixi_proto::paths::test_profile().unwrap();
        for sub in ["workspace", "tmp", "home/Documents", "home/Downloads", "home/Desktop"] { fs::create_dir_all(profile.join(sub))?; }
    }
    let resources_dir = locate_resources_dir();
    tracing::info!("QuixiDesk booting with resources_dir: {:?}", resources_dir);

    // Start background Tokio runtime for async engine operations
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _rt_guard = tokio_rt.enter();

    let engine = QuixiEngine::new(resources_dir.clone());
    if let Some(profile) = quixi_proto::paths::test_profile() {
        let fixture_path = mock_account.as_ref().map(PathBuf::from)
            .unwrap_or_else(|| profile.join("mock-account.json"));
        if !fixture_path.exists() {
            if mock_account.is_some() { anyhow::bail!("Mock account fixture does not exist: {}",fixture_path.display()); }
            let mut fixture = engine.build_live_bootstrap();
            let account = &mut fixture["account"];
            account["uuid"] = serde_json::json!(uuid::Uuid::new_v4().to_string());
            account["email_address"] = serde_json::json!("qa@example.invalid");
            account["full_name"] = serde_json::json!("Mock QA Account");
            account["display_name"] = serde_json::json!("Mock QA");
            if let Some(memberships) = account["memberships"].as_array_mut() {
                for (index, member) in memberships.iter_mut().enumerate() {
                    member["organization"]["name"] = serde_json::json!(format!("Mock Organization {}",index+1));
                }
            }
            let fixture = serde_json::json!({"account":fixture["account"],"current_user_access":fixture["current_user_access"],"featureFlagsOrgUuid":fixture["featureFlagsOrgUuid"]});
            fs::write(&fixture_path, serde_json::to_vec_pretty(&fixture)?)?;
        }
        let fixture: Value = serde_json::from_slice(&fs::read(&fixture_path)?)?;
        engine.apply_mock_account(&fixture).map_err(anyhow::Error::msg)?;
        tracing::info!("Test profile active: {} (mock account data only; providers configured separately)",profile.display());
    } else if mock_account.is_some() {
        anyhow::bail!("--mock-account requires --test-profile");
    }
    let router = Arc::new(EipcRouter::new(engine.clone()));

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    #[cfg(target_os = "macos")]
    let _ = MENU_PROXY.set(std::sync::Mutex::new(event_loop.create_proxy()));
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

    // Build the native workspace window and titlebar inset.
    let mut win_builder = WindowBuilder::new()
        .with_title(if test_profile.is_some() { "QuixiDesk — Test Profile" } else { "QuixiDesk" })
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

    // Only original, repository-owned UI code runs in the main frame.
    let init_script = include_str!("native_bridge.js").replace("__QUIXI_APP_ORIGIN__", APP_ORIGIN)
        + include_str!("storage_ui.js");

    #[cfg(target_os = "macos")]
    {
        let picker_proxy = proxy.clone();
        quixi_platform::set_folder_picker(Arc::new(move |prompt, multiple| {
            let (tx, rx) = std::sync::mpsc::channel();
            if picker_proxy.send_event(UserEvent::PickFolders {
                prompt: prompt.unwrap_or("Choose a folder").to_string(), multiple, tx,
            }).is_err() { return vec![]; }
            rx.recv().unwrap_or_default()
        }));
    }

    let protocol_resources = resources_dir.clone();
    let protocol_engine = engine.clone();
    let protocol_router = router.clone();
    let protocol_proxy = proxy.clone();

    let user_agent = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) QuixiDesk/3.6556.7 Safari/605.1.15";

    let mut web_context = wry::WebContext::new(quixi_proto::paths::test_profile().map(|p| p.join("website-data")));
    let mut webview_builder = if test_profile.is_some() { WebViewBuilder::with_web_context(&mut web_context) } else { WebViewBuilder::new() };
    #[cfg(target_os = "macos")]
    if let Some(profile) = quixi_proto::paths::test_profile() {
        let file = profile.join("web-store-id");
        let id: [u8; 16] = if file.exists() { fs::read(&file)?.try_into().map_err(|_| anyhow::anyhow!("Invalid test web store ID"))? }
            else { let id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos().to_le_bytes(); fs::write(file, id)?; id };
        webview_builder = webview_builder.with_data_store_identifier(id);
    }
    let webview_builder = webview_builder
        .with_user_agent(user_agent)
        .with_initialization_script_for_main_only(&init_script, true)
        .with_devtools(true)
        .with_clipboard(true)
        .with_asynchronous_custom_protocol("app".into(), move |_webview_id, request, responder| {
            let protocol_resources = protocol_resources.clone();
            let protocol_engine = protocol_engine.clone();
            let protocol_router = protocol_router.clone();
            let protocol_proxy = protocol_proxy.clone();
            let is_ipc = request.uri().path().starts_with("/__eipc__/")
                || request.uri().path().starts_with("/api/")
                || request.uri().path().starts_with("/edge-api/");
            let handle = move || {
                let method = request.method().as_str().to_uppercase();
                let uri = request.uri();
                let path = uri.path();
                // Opaque sandbox frames and remote websites must never invoke app APIs.
                if !trusted_app_origin(request.headers().get("Origin")) {
                    return build_http_response(403, "text/plain", b"Untrusted origin".to_vec());
                }
                let path_and_query = uri
                    .path_and_query()
                    .map(|pq| pq.as_str())
                    .unwrap_or(path);

                if method == "OPTIONS" {
                    return build_http_response(204, "text/plain", Vec::new());
                }

                // 1. Native Window Actions, Clipboard & Diagnostics
                if path == "/__eipc__/open-external" && method == "POST" {
                    let parsed = serde_json::from_slice::<Value>(request.body()).ok()
                        .and_then(|body| body["url"].as_str().and_then(|s| url::Url::parse(s).ok()));
                    let Some(url) = parsed.filter(|u| matches!(u.scheme(), "http" | "https") && u.username().is_empty() && u.password().is_none()) else {
                        return build_http_response(400, "application/json", b"{\"error\":\"Only HTTP and HTTPS links are supported\"}".to_vec());
                    };
                    #[cfg(target_os = "macos")]
                    let mut command = std::process::Command::new("/usr/bin/open");
                    #[cfg(target_os = "linux")]
                    let mut command = std::process::Command::new("xdg-open");
                    #[cfg(target_os = "windows")]
                    let mut command = {
                        let mut c = std::process::Command::new("rundll32.exe");
                        c.arg("url.dll,FileProtocolHandler"); c
                    };
                    return match command.arg(url.as_str()).spawn() {
                        Ok(_) => build_http_response(200, "application/json", b"{\"ok\":true}".to_vec()),
                        Err(_) => build_http_response(500, "application/json", b"{\"error\":\"Could not open the system browser\"}".to_vec()),
                    };
                }
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
                                #[cfg(target_os = "linux")]
                                let written = {
                                    let (tx, rx) = std::sync::mpsc::channel();
                                    protocol_proxy.send_event(UserEvent::WriteClipboard { text: text.to_string(), tx }).is_ok()
                                        && matches!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(Ok(())))
                                };
                                #[cfg(not(target_os = "linux"))]
                                let written = quixi_platform::ComputerUse::write_clipboard(text).is_ok();
                                if !written {
                                    return build_http_response(500, "application/json", b"{\"ok\":false}".to_vec());
                                }
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

                // 4. Static assets & SPA fallback (the original interface, `renderer`, `fonts`)
                serve_static_or_spa(&protocol_resources, path)
            };
            // Native pickers and other IPC work may block; asset delivery stays ordered.
            if is_ipc {
                std::thread::spawn(move || responder.respond(handle()));
            } else {
                responder.respond(handle());
            }
        })
        .with_custom_protocol("cowork-artifact".into(), move |_webview_id, _request| {
            build_http_response(
                200,
                "text/html; charset=utf-8",
                b"<!doctype html><html><body></body></html>".to_vec(),
            )
        })
        .with_url("app://localhost/");

    // Linux Tao windows own a GTK container; the raw window-handle builder
    // does not support that handle kind on Wayland/X11 GTK backends.
    #[cfg(target_os = "linux")]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        webview_builder.build_gtk(window.default_vbox().ok_or_else(|| anyhow::anyhow!("GTK window container unavailable"))?)?
    };
    #[cfg(not(target_os = "linux"))]
    let webview = webview_builder.build(&window)?;

    start_automation_http_server(proxy.clone())?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {
                #[cfg(target_os = "macos")]
                setup_macos_main_menu();
                tracing::info!("QuixiDesk native window initialized (app://localhost/)");
            }
            Event::LoopDestroyed => {
                tracing::info!("QuixiDesk native event loop stopped");
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if engine.has_unsaved_conversations() {
                    let state = engine.session_storage_status();
                    let _ = webview.evaluate_script(&format!("window.__quixi_show_storage_status__?.({state});"));
                } else { exit_event_loop(control_flow); }
            }
            Event::UserEvent(user_evt) => match user_evt {
                #[cfg(target_os = "linux")]
                UserEvent::WriteClipboard { text, tx } => {
                    // GTK clipboard ownership must be established on the GTK event thread,
                    // while the custom-protocol worker waits for a real native result.
                    let result = if gtk::gdk::Display::default().is_some() {
                        let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
                        clipboard.set_text(&text);
                        clipboard.store();
                        // Preserve the existing in-app clipboard read bridge as well.
                        quixi_platform::ComputerUse::write_clipboard(&text).map_err(|error| error.to_string())
                    } else { Err("GTK display unavailable".to_string()) };
                    let _ = tx.send(result);
                }
                UserEvent::OpenSettings => {
                    let _ = webview.evaluate_script("window.quixiNavigate?.('settings');");
                }
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
                UserEvent::PickFolders { prompt, multiple, tx } => {
                    #[cfg(target_os = "macos")]
                    show_folder_sheet(&window, &prompt, multiple, tx);
                    #[cfg(not(target_os = "macos"))]
                    { let _ = (prompt, multiple); let _ = tx.send(vec![]); }
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
                    if engine.has_unsaved_conversations() {
                        tracing::warn!("Close blocked: conversations have no durable save");
                        let state = engine.session_storage_status();
                        let _ = webview.evaluate_script(&format!("window.__quixi_show_storage_status__?.({state});"));
                    } else {
                        tracing::info!("Closing QuixiDesk with conversations durably saved");
                        exit_event_loop(control_flow);
                    }
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
