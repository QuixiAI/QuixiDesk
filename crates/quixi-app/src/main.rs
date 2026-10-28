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

#[cfg(target_os = "linux")]
use tao::platform::unix::WindowExtUnix;
#[cfg(target_os = "linux")]
use wry::WebViewBuilderExtUnix;

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
        if p.join("ion-dist/index.html").exists() {
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
            // Linux/Windows install layout: <prefix>/bin/QuixiDesk -> <prefix>/share/quixidesk/resources
            let share_res = exe_dir.join("../share/quixidesk/resources");
            if share_res.join("ion-dist/index.html").exists() {
                return share_res.canonicalize().unwrap_or(share_res);
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

/// Shared request handler for the `app://localhost` custom protocol and the loopback
/// HTTP bridge. Returns `None` for static asset paths so callers can serve them from disk.
///
/// WebKitGTK does not expose request bodies to custom URI scheme handlers, so every
/// body-bearing `$eipc_message$` / REST call is proxied over a loopback HTTP socket by
/// the injected bridge prelude (`window.fetch`, `XMLHttpRequest`, `EventSource`).
#[allow(clippy::too_many_arguments)]
fn handle_quixi_request(
    method: &str,
    path: &str,
    path_and_query: &str,
    body: &[u8],
    engine: &Arc<QuixiEngine>,
    router: &Arc<EipcRouter>,
    proxy: &tao::event_loop::EventLoopProxy<UserEvent>,
) -> Option<(u16, String, Vec<u8>)> {
    const JSON: &str = "application/json; charset=utf-8";

    if method == "OPTIONS" {
        return Some((204, "text/plain".to_string(), Vec::new()));
    }

    // 1. Native window actions, clipboard & diagnostics
    if path == "/__eipc__/diag" {
        if let Ok(val) = serde_json::from_slice::<Value>(body) {
            tracing::info!("[WebView Diag] {}", val);
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
        return Some((200, JSON.to_string(), b"{\"ok\":true}".to_vec()));
    }

    if path == "/__eipc__/clipboard" {
        if let Ok(body_val) = serde_json::from_slice::<Value>(body) {
            match body_val.get("action").and_then(|v| v.as_str()).unwrap_or("") {
                "write" => {
                    let text = body_val.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    #[cfg(target_os = "linux")]
                    let written = {
                        let (tx, rx) = std::sync::mpsc::channel();
                        proxy.send_event(UserEvent::WriteClipboard { text: text.to_string(), tx }).is_ok()
                            && matches!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(Ok(())))
                    };
                    #[cfg(not(target_os = "linux"))]
                    let written = quixi_platform::ComputerUse::write_clipboard(text).is_ok();
                    if !written {
                        return Some((500, JSON.to_string(), b"{\"ok\":false}".to_vec()));
                    }
                    return Some((200, JSON.to_string(), b"{\"ok\":true}".to_vec()));
                }
                "read" => {
                    let text = quixi_platform::ComputerUse::read_clipboard();
                    let resp = serde_json::json!({ "ok": true, "text": text });
                    return Some((
                        200,
                        JSON.to_string(),
                        serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec()),
                    ));
                }
                _ => {}
            }
        }
        return Some((200, JSON.to_string(), b"{\"ok\":true}".to_vec()));
    }

    if path == "/__eipc__/window-action" {
        if let Ok(body_val) = serde_json::from_slice::<Value>(body) {
            match body_val.get("action").and_then(|v| v.as_str()).unwrap_or("") {
                "drag" => {
                    let _ = proxy.send_event(UserEvent::DragWindow);
                }
                "toggle_maximize" => {
                    let _ = proxy.send_event(UserEvent::ToggleMaximize);
                }
                "minimize" => {
                    let _ = proxy.send_event(UserEvent::MinimizeWindow);
                }
                "close" => {
                    let _ = proxy.send_event(UserEvent::CloseWindow);
                }
                _ => {}
            }
        }
        return Some((200, JSON.to_string(), b"{\"ok\":true}".to_vec()));
    }

    // 2. `$eipc_message$` RPC & Sync Store Bridge
    if path.starts_with("/__eipc__/") {
        return match serde_json::from_slice::<EipcInvokePayload>(body) {
            Ok(payload) => {
                if payload.channel.contains("WindowControl_$_minimize") {
                    let _ = proxy.send_event(UserEvent::MinimizeWindow);
                } else if payload.channel.contains("WindowControl_$_maximize") {
                    let _ = proxy.send_event(UserEvent::ToggleMaximize);
                } else if payload.channel.contains("WindowControl_$_close") {
                    let _ = proxy.send_event(UserEvent::CloseWindow);
                }
                let reply = router.dispatch(&payload);
                if std::env::var_os("QUIXI_DEBUG_EIPC").is_some() {
                    tracing::info!(
                        "[eipc] {} -> err={} {}",
                        payload.channel,
                        reply.error.is_some(),
                        reply
                            .result
                            .as_ref()
                            .map(|v| serde_json::to_string(v)
                                .unwrap_or_default()
                                .chars()
                                .take(160)
                                .collect::<String>())
                            .unwrap_or_else(|| "null".to_string())
                    );
                }
                let bytes = serde_json::to_vec(&reply).unwrap_or_else(|_| b"{}".to_vec());
                Some((200, JSON.to_string(), bytes))
            }
            Err(err) => {
                let err_json = serde_json::json!({
                    "error": { "message": format!("Invalid EIPC payload: {}", err) }
                });
                Some((
                    400,
                    JSON.to_string(),
                    serde_json::to_vec(&err_json).unwrap_or_default(),
                ))
            }
        };
    }

    // 3. Local REST & SSE API
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
            engine.handle_api_request(method, path_and_query, body);
        tracing::info!("[API] {} {} -> {}", method, path_and_query, status);
        return Some((status, content_type.to_string(), body));
    }

    None
}

/// Starts the loopback HTTP bridge used for requests that carry bodies (WebKitGTK
/// custom URI schemes drop them). Binds an ephemeral loopback port and requires a
/// per-launch bearer token supplied by the injected prelude.
fn start_api_http_bridge(
    engine: Arc<QuixiEngine>,
    router: Arc<EipcRouter>,
    proxy: tao::event_loop::EventLoopProxy<UserEvent>,
) -> Option<(u16, String)> {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind(("127.0.0.1", 0)).ok()?;
    let port = listener.local_addr().ok()?.port();
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let token = format!("{:x}{:x}{:x}", seed, std::process::id(), port);

    let thread_token = token.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let engine = engine.clone();
            let router = router.clone();
            let proxy = proxy.clone();
            let expected_token = thread_token.clone();
            std::thread::spawn(move || {
                let mut stream = stream;
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));
                let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));

                let mut buf = Vec::with_capacity(8192);
                let mut temp = [0u8; 8192];
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
                            if buf.len() > 64 * 1024 * 1024 {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }

                let hdr_end = match header_end {
                    Some(h) => h,
                    None => return,
                };
                let req_header = String::from_utf8_lossy(&buf[..hdr_end]).to_string();
                let request_origin = req_header
                    .lines()
                    .find_map(|line| {
                        line.split_once(':').and_then(|(k, v)| {
                            if k.trim().eq_ignore_ascii_case("origin") {
                                Some(v.trim().to_string())
                            } else {
                                None
                            }
                        })
                    })
                    .unwrap_or_else(|| "*".to_string());
                let request_headers = req_header
                    .lines()
                    .find_map(|line| {
                        line.split_once(':').and_then(|(k, v)| {
                            if k.trim().eq_ignore_ascii_case("access-control-request-headers") {
                                Some(v.trim().to_string())
                            } else {
                                None
                            }
                        })
                    })
                    .unwrap_or_else(|| "*".to_string());

                let write_resp = |stream: &mut std::net::TcpStream,
                                  status: u16,
                                  ct: &str,
                                  body: &[u8]| {
                    let reason = match status {
                        200 => "OK",
                        204 => "No Content",
                        400 => "Bad Request",
                        403 => "Forbidden",
                        404 => "Not Found",
                        500 => "Internal Server Error",
                        _ => "OK",
                    };
                    // Credentialed fetches (the renderer API client uses `credentials:
                    // include`) reject `Access-Control-Allow-Origin: *`, so reflect the
                    // exact custom-scheme origin and allow credentials.
                    let hdr = format!(
                        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Credentials: true\r\nAccess-Control-Allow-Methods: GET, POST, PUT, PATCH, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: {}\r\nAccess-Control-Expose-Headers: *\r\nAccess-Control-Max-Age: 600\r\nVary: Origin\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
                        status,
                        reason,
                        ct,
                        body.len(),
                        request_origin,
                        request_headers
                    );
                    let _ = stream.write_all(hdr.as_bytes());
                    let _ = stream.write_all(body);
                    let _ = stream.flush();
                };

                let first_line = req_header.lines().next().unwrap_or("");
                let mut parts = first_line.split_whitespace();
                let method = parts.next().unwrap_or("GET").to_uppercase();
                let target = parts.next().unwrap_or("/");
                let body_bytes = if hdr_end <= buf.len() {
                    &buf[hdr_end..]
                } else {
                    &[][..]
                };

                // Token check: query parameter (EventSource) or header (fetch/XHR).
                let has_header_token = req_header.lines().any(|line| {
                    line.split_once(':')
                        .map(|(k, v)| {
                            k.trim().eq_ignore_ascii_case("x-quixi-bridge-token")
                                && v.trim() == expected_token
                        })
                        .unwrap_or(false)
                });
                let has_query_token = target
                    .split_once('?')
                    .map(|(_, q)| {
                        q.split('&').any(|kv| {
                            kv.split_once('=')
                                .map(|(k, v)| k == "__quixi_token" && v == expected_token)
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false);

                if !has_header_token && !has_query_token && method != "OPTIONS" {
                    write_resp(
                        &mut stream,
                        403,
                        "application/json",
                        b"{\"error\":\"invalid bridge token\"}",
                    );
                    return;
                }

                let (path, path_and_query) = match target.split_once('?') {
                    Some((p, q)) => (p, format!("{}?{}", p, q)),
                    None => (target, target.to_string()),
                };
                if std::env::var_os("QUIXI_DEBUG_BRIDGE").is_some() {
                    tracing::info!("[bridge] {} {}", method, target);
                }
                // Strip the private bridge token before handing the query to the app.
                let clean_path_and_query = match path_and_query.split_once("__quixi_token=") {
                    Some((head, tail)) => {
                        let rest = tail.split('&').skip(1).collect::<Vec<_>>().join("&");
                        let trimmed = head.trim_end_matches(['?', '&']);
                        if rest.is_empty() {
                            trimmed.to_string()
                        } else {
                            format!("{}?{}", trimmed, rest)
                        }
                    }
                    None => path_and_query.clone(),
                };

                match handle_quixi_request(
                    &method,
                    path,
                    &clean_path_and_query,
                    body_bytes,
                    &engine,
                    &router,
                    &proxy,
                ) {
                    Some((status, ct, body)) => write_resp(&mut stream, status, &ct, &body),
                    None => write_resp(
                        &mut stream,
                        404,
                        "application/json",
                        b"{\"error\":\"not found on api bridge\"}",
                    ),
                }
            });
        }
    });

    Some((port, token))
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
    let ion_dist = resources_dir.join("ion-dist");
    let renderer_dir = resources_dir.join("renderer");

    // 1. Direct static file in ion-dist
    if !clean_path.is_empty() {
        let candidate = ion_dist.join(clean_path);
        if candidate.is_file() {
            if std::env::var_os("QUIXI_DEBUG_ASSETS").is_some() {
                tracing::info!("[asset] {}", candidate.display());
            }
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

/// Linux compositor selection.
///
/// The WebView is embedded as a native GTK widget (`build_gtk`), so Wayland works
/// out of the box. Some drivers/compositors still render blank through the GTK
/// WebKit DMABUF path; `QUIXI_X11=1` forces the battle-tested XWayland path and
/// `WEBKIT_DISABLE_DMABUF_RENDERER=1` disables the DMABUF renderer directly.
#[cfg(target_os = "linux")]
fn configure_linux_display_backend() {
    if std::env::var_os("QUIXI_X11").is_none() {
        return;
    }
    if std::env::var_os("DISPLAY").is_some() {
        std::env::set_var("WINIT_UNIX_BACKEND", "x11");
        std::env::set_var("GDK_BACKEND", "x11");
    }
}

fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    configure_linux_display_backend();
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

    // WebKitGTK drops request bodies for `app://` custom URI schemes, so expose the
    // same API/EIPC surface over a token-guarded loopback HTTP socket as well.
    let http_bridge = start_api_http_bridge(engine.clone(), router.clone(), proxy.clone());
    if let Some((port, _)) = &http_bridge {
        tracing::info!("QuixiDesk loopback API bridge listening on http://127.0.0.1:{}", port);
    }

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

    // Read combined Electron-free preload bridge
    let preload_path = resources_dir.join("preload/combined_preload.js");
    let mut init_script = fs::read_to_string(&preload_path).unwrap_or_else(|e| {
        tracing::error!("Failed to read {:?}: {}", preload_path, e);
        String::new()
    });

    #[cfg(target_os = "linux")]
    init_script.push_str(
        r#"
;(function() {
  try {
    if (globalThis.process) { globalThis.process.platform = "linux"; }
  } catch (_) {}
})();
"#,
    );

    // Route body-bearing API/EIPC calls through the loopback HTTP bridge. This is required
    // on WebKitGTK (Linux), where custom scheme handlers never receive POST bodies, and is
    // harmless on macOS where the custom scheme handler also works.
    if let Some((port, token)) = &http_bridge {
        init_script.push_str(&format!(
            r#"
;(function() {{
  var base = "http://127.0.0.1:{port}";
  var token = "{token}";
  var apiPrefixes = ["/__eipc__/", "/api/", "/api", "/edge-api", "/plugins/", "/marketplaces/", "/v1/", "/v2/", "/dust"];
  window.__QUIXI_HTTP_BASE__ = base;
  window.__QUIXI_BRIDGE_TOKEN__ = token;

  function rewrite(raw) {{
    if (raw === null || raw === undefined) return null;
    var u = String(raw);
    var path = null;
    if (u.indexOf("app://localhost") === 0) path = u.slice("app://localhost".length);
    else if (u.charAt(0) === "/") path = u;
    else {{
      try {{
        var parsed = new URL(u, window.location.href);
        if (parsed.origin === "app://localhost" || parsed.protocol === "app:") path = parsed.pathname + parsed.search;
      }} catch (_) {{ return null; }}
    }}
    if (path === null) return null;
    var handled = apiPrefixes.some(function (p) {{
      return p.charAt(p.length - 1) === "/" ? path.indexOf(p) === 0 : (path === p || path.indexOf(p + "/") === 0 || path.indexOf(p + "?") === 0);
    }});
    if (!handled) return null;
    var sep = path.indexOf("?") === -1 ? "?" : "&";
    return base + path + sep + "__quixi_token=" + encodeURIComponent(token);
  }}

  function installFetchBridge() {{
    if (typeof window.fetch !== "function" || window.fetch.__quixi_bridge__) return;
    var nativeFetch = window.fetch.bind(window);
    var bridgeFetch = function (input, init) {{
      try {{
        if (typeof input === "string") {{
          var mapped = rewrite(input);
          if (mapped) return nativeFetch(mapped, init);
        }} else if (input && typeof input === "object" && typeof input.url === "string") {{
          var mappedReq = rewrite(input.url);
          if (mappedReq) {{
            try {{ input = new Request(mappedReq, input); }} catch (_) {{ return nativeFetch(mappedReq, init); }}
          }}
        }}
      }} catch (_) {{}}
      return nativeFetch(input, init);
    }};
    bridgeFetch.__quixi_bridge__ = true;
    try {{ window.fetch = bridgeFetch; }} catch (_) {{}}
  }}
  installFetchBridge();

  function installXhrBridge() {{
    if (typeof XMLHttpRequest === "undefined" || !XMLHttpRequest.prototype.open || XMLHttpRequest.prototype.open.__quixi_bridge__) return;
    var nativeOpen = XMLHttpRequest.prototype.open;
    var bridgeOpen = function (method, url) {{
      var args = Array.prototype.slice.call(arguments);
      try {{
        var mapped = rewrite(url);
        if (mapped) args[1] = mapped;
      }} catch (_) {{}}
      return nativeOpen.apply(this, args);
    }};
    bridgeOpen.__quixi_bridge__ = true;
    try {{ XMLHttpRequest.prototype.open = bridgeOpen; }} catch (_) {{}}
  }}
  installXhrBridge();

  // Telemetry SDKs (Datadog/Sentry) instrument fetch/XHR during bootstrap and would
  // otherwise replace the bridge wrapper. Re-install around their startup.
  [0, 100, 500, 1500, 4000].forEach(function (delay) {{
    setTimeout(function () {{ installFetchBridge(); installXhrBridge(); }}, delay);
  }});

  if (typeof EventSource === "function" && !EventSource.__quixi_bridge__) {{
    var NativeEventSource = EventSource;
    var BridgeEventSource = function (url, config) {{
      var mapped = null;
      try {{ mapped = rewrite(url); }} catch (_) {{}}
      return new NativeEventSource(mapped || url, config);
    }};
    BridgeEventSource.prototype = NativeEventSource.prototype;
    BridgeEventSource.CONNECTING = 0;
    BridgeEventSource.OPEN = 1;
    BridgeEventSource.CLOSED = 2;
    BridgeEventSource.__quixi_bridge__ = true;
    window.EventSource = BridgeEventSource;
  }}

  if (navigator && typeof navigator.sendBeacon === "function" && !navigator.sendBeacon.__quixi_bridge__) {{
    var nativeBeacon = navigator.sendBeacon.bind(navigator);
    var bridgeBeacon = function (url, data) {{
      var mapped = null;
      try {{ mapped = rewrite(url); }} catch (_) {{}}
      return nativeBeacon(mapped || url, data);
    }};
    bridgeBeacon.__quixi_bridge__ = true;
    navigator.sendBeacon = bridgeBeacon;
  }}

  setTimeout(function() {{
    try {{
      localStorage.setItem("__quixi_bridge_diag__", JSON.stringify({{
        fetch: !!(window.fetch && window.fetch.__quixi_bridge__),
        xhr: !!(XMLHttpRequest.prototype.open && XMLHttpRequest.prototype.open.__quixi_bridge__),
        es: !!(EventSource && EventSource.__quixi_bridge__),
        href: location.href,
        at: Date.now()
      }}));
    }} catch (_) {{}}
  }}, 1200);
}})();
"#
        ));
    }

    // Linux implementation of the native file preview surface. The renderer's
    // CoworkFilePreview.show()/hide() calls are answered by the EIPC router,
    // which emits QuixiPreviewHost onShow/onHide events; this helper renders
    // the previewed file into a fixed-position sandboxed surface anchored to
    // the preview pane and reports drawn/reset back through EIPC.
    #[cfg(target_os = "linux")]
    init_script.push_str(&format!(
        r#"
;(function() {{
  if (window.__QUIXI_PREVIEW_HELPER__) return;
  window.__QUIXI_PREVIEW_HELPER__ = true;
  var PRE = "$eipc_message$_{uuid}_$_claude.web_$_";

  function eipcCall(member, args) {{
    try {{
      return fetch("/__eipc__/invoke", {{
        method: "POST",
        headers: {{ "Content-Type": "application/json" }},
        body: JSON.stringify({{ channel: PRE + "QuixiPreviewHost_$_" + member, args: args }})
      }}).catch(function() {{}});
    }} catch (_) {{
      return Promise.resolve();
    }}
  }}

  function onChannel(member, fn) {{
    try {{
      var m = window.__QUIXI_LISTENERS__ || (window.__QUIXI_LISTENERS__ = new Map());
      var ch = PRE + "QuixiPreviewHost_$_" + member;
      var set = m.get(ch);
      if (!set) {{ set = new Set(); m.set(ch, set); }}
      set.add(function(ev) {{
        var args = Array.prototype.slice.call(arguments, 1);
        try {{ fn.apply(null, args); }} catch (err) {{ console.warn("[quixi-preview]", err); }}
      }});
    }} catch (_) {{}}
  }}

  var surface = null;

  function removeSurface(notifyReset) {{
    var s = surface;
    if (!s) return;
    surface = null;
    try {{ if (s.unbind) s.unbind(); }} catch (_) {{}}
    try {{ s.root.remove(); }} catch (_) {{}}
    if (notifyReset && s.path) eipcCall("reset", [s.path]);
  }}

  function findAnchor(rect) {{
    try {{
      var cx = Math.min(Math.max(rect.x + rect.width / 2, 1), window.innerWidth - 1);
      var cy = Math.min(Math.max(rect.y + rect.height / 2, 1), window.innerHeight - 1);
      var el = document.elementFromPoint(cx, cy);
      var best = null, bestErr = Infinity;
      for (var cur = el; cur; cur = cur.parentElement) {{
        var r = cur.getBoundingClientRect();
        var err = Math.abs(r.x - rect.x) + Math.abs(r.y - rect.y)
          + Math.abs(r.width - rect.width) + Math.abs(r.height - rect.height);
        if (err < bestErr) {{ bestErr = err; best = cur; }}
        if (err <= 2) break;
      }}
      if (best && bestErr <= 8) return best;
    }} catch (_) {{}}
    return null;
  }}

  function place(s) {{
    try {{
      var r = (s.anchor && s.anchor.isConnected)
        ? s.anchor.getBoundingClientRect()
        : s.rect;
      var hidden = !r || r.width < 4 || r.height < 4;
      s.root.style.opacity = hidden ? "0" : "1";
      s.root.style.pointerEvents = hidden ? "none" : "auto";
      if (!hidden) {{
        s.root.style.left = r.x + "px";
        s.root.style.top = r.y + "px";
        s.root.style.width = r.width + "px";
        s.root.style.height = r.height + "px";
      }}
    }} catch (_) {{}}
  }}

  function b64ToBlobUrl(b64, mime) {{
    try {{
      var bin = atob(b64);
      var arr = new Uint8Array(bin.length);
      for (var i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
      return URL.createObjectURL(new Blob([arr], {{ type: mime || "application/octet-stream" }}));
    }} catch (_) {{
      return null;
    }}
  }}

  function showPreview(payload) {{
    if (!payload || typeof payload !== "object") return;
    var path = String(payload.path || "");
    var sid = String(payload.sessionId || "");
    var rect = (payload.rect && typeof payload.rect === "object") ? payload.rect
      : {{ x: 0, y: 0, width: window.innerWidth, height: window.innerHeight }};
    removeSurface(false);

    var root = document.createElement("div");
    root.setAttribute("data-quixi-preview-surface", "1");
    root.style.cssText = "position:fixed;z-index:2147483000;background:#ffffff;"
      + "overflow:hidden;border-radius:10px;box-shadow:0 12px 48px rgba(0,0,0,0.28);";

    var extM = path.match(/\.([A-Za-z0-9]+)$/);
    var ext = extM ? extM[1].toLowerCase() : "";
    var isHtml = ext === "html" || ext === "htm";
    var isImage = ["png","jpg","jpeg","jfif","gif","webp","bmp","ico","avif","svg"].indexOf(ext) >= 0;

    var s = {{ root: root, path: path, rect: rect, anchor: findAnchor(rect), unbind: null, blobUrl: null }};
    surface = s;
    var ready = false;
    function markDrawn() {{
      if (surface === s && !ready) {{ ready = true; eipcCall("drawn", [path]); }}
    }}
    function markReset() {{
      if (surface === s) eipcCall("reset", [path]);
    }}

    fetch("/__eipc__/invoke", {{
      method: "POST",
      headers: {{ "Content-Type": "application/json" }},
      body: JSON.stringify({{
        channel: PRE + "FileSystem_$_readLocalFile",
        args: [sid, encodeURIComponent(path)]
      }})
    }}).then(function(r) {{ return r.ok ? r.json() : null; }})
      .then(function(d) {{ return (d && d.result) || null; }})
      .catch(function() {{ return null; }})
      .then(function(res) {{
        if (surface !== s) return;
        if (!res || typeof res.content !== "string") {{ markReset(); return; }}
        var mime = res.mimeType || "application/octet-stream";
        if (isHtml) {{
          var frame = document.createElement("iframe");
          frame.setAttribute("sandbox", "allow-scripts allow-forms allow-popups allow-modals");
          frame.setAttribute("referrerpolicy", "no-referrer");
          frame.style.cssText = "border:0;width:100%;height:100%;display:block;background:#ffffff;";
          root.appendChild(frame);
          frame.addEventListener("load", markDrawn);
          frame.addEventListener("error", markReset);
          frame.srcdoc = res.encoding === "base64" ? atob(res.content) : res.content;
        }} else if (isImage) {{
          var img = document.createElement("img");
          img.alt = "";
          img.style.cssText = "width:100%;height:100%;object-fit:contain;display:block;background:#141414;";
          root.appendChild(img);
          img.addEventListener("load", markDrawn);
          img.addEventListener("error", markReset);
          var url = null;
          if (res.encoding === "base64") url = b64ToBlobUrl(res.content, mime);
          else {{
            try {{ url = URL.createObjectURL(new Blob([res.content], {{ type: mime }})); }}
            catch (_) {{ url = null; }}
          }}
          if (url) {{ s.blobUrl = url; img.src = url; }} else {{ markReset(); }}
        }} else {{
          var pre = document.createElement("pre");
          pre.style.cssText = "margin:0;padding:16px;color:#e8e8e8;"
            + "font:12px/1.55 ui-monospace,SFMono-Regular,Menlo,monospace;"
            + "white-space:pre-wrap;word-break:break-word;";
          if (res.encoding === "base64") {{
            pre.textContent = "(binary file)";
            root.style.background = '#141414';
          }} else {{
            pre.textContent = res.content;
            root.style.background = '#141414';
          }}
          root.style.overflow = "auto";
          root.appendChild(pre);
          markDrawn();
        }}
        place(s);
      }});

    document.documentElement.appendChild(root);
    place(s);

    var raf = 0;
    var onMove = function() {{
      if (surface !== s || raf) return;
      raf = requestAnimationFrame(function() {{
        raf = 0;
        if (surface === s) place(s);
      }});
    }};
    window.addEventListener("resize", onMove, true);
    window.addEventListener("scroll", onMove, true);
    s.unbind = function() {{
      window.removeEventListener("resize", onMove, true);
      window.removeEventListener("scroll", onMove, true);
      if (raf) {{ try {{ cancelAnimationFrame(raf); }} catch (_) {{}} raf = 0; }}
      if (s.blobUrl) {{ try {{ URL.revokeObjectURL(s.blobUrl); }} catch (_) {{}} s.blobUrl = null; }}
    }};
  }}

  onChannel("onShow", showPreview);
  onChannel("onHide", function() {{ removeSurface(true); }});
}})();
"#,
        uuid = quixi_proto::EIPC_UUID_MAIN,
    ));

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

  function recordBootError(kind, data) {
    try {
      var key = '__quixi_boot_errors__';
      var list = [];
      try { list = JSON.parse(window.localStorage.getItem(key) || '[]'); } catch (_) { list = []; }
      list.push({ kind: kind, at: Date.now(), data: data });
      if (list.length > 30) list = list.slice(-30);
      window.localStorage.setItem(key, JSON.stringify(list));
      var arr = window.__quixi_boot_errors__ || [];
      arr.push({ kind: kind, at: Date.now(), data: data });
      window.__quixi_boot_errors__ = arr;
    } catch (_) {}
  }

  window.addEventListener('error', function(e) {
    var data = {
      message: String(e.message || ''),
      filename: String(e.filename || ''),
      lineno: e.lineno,
      colno: e.colno
    };
    recordBootError('window.error', data);
    reportToRust('window.error', data);
  });

  window.addEventListener('unhandledrejection', function(e) {
    var reason = e.reason;
    var data = {
      message: reason && reason.message ? String(reason.message) : String(reason),
      stack: reason && reason.stack ? String(reason.stack).slice(0, 800) : null
    };
    recordBootError('unhandledrejection', data);
    reportToRust('unhandledrejection', data);
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
      recordBootError('console.error', { args: args });
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
    init_script.push_str(include_str!("local_artifacts_ui.js"));
    init_script.push_str(include_str!("storage_ui.js"));

    // The E2E harness needs the live workspace root, not a baked-in developer path.
    init_script = init_script.replace(
        "__QUIXI_WORKSPACE__",
        &quixi_proto::paths::default_workspace_str().replace('\\', "\\\\").replace('\'', "\\'"),
    );
    // Keep IPC URLs and the reused preload's trusted-origin checks consistent
    // with the origin exposed by the platform webview.
    init_script = init_script.replace("app://localhost", APP_ORIGIN);

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

    let user_agent = if cfg!(target_os = "macos") {
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) QuixiDesk/3.6556.2 Claude/3.6556.2 Electron/44.9.3 Safari/605.1.15"
    } else {
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) QuixiDesk/3.6556.2 Claude/3.6556.2 Electron/44.9.3 Safari/605.1.15"
    };

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
            let is_ipc = request.uri().path().starts_with("/__eipc__/");
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

                // The same handler is used by the loopback HTTP bridge; see
                // `handle_quixi_request` for why body-bearing requests cannot rely on
                // WebKitGTK custom URI schemes.
                if let Some((status, content_type, body)) = handle_quixi_request(
                    &method,
                    path,
                    path_and_query,
                    request.body(),
                    &protocol_engine,
                    &protocol_router,
                    &protocol_proxy,
                ) {
                    return build_http_response(status, &content_type, body);
                }

                // 4. Static assets & SPA fallback (`ion-dist`, `renderer`, `fonts`)
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

    // On Linux, attaching the WebView to the window's X11 child handle leaves the page
    // unpainted under XWayland (blank window). Build it as a native GTK widget inside
    // tao's GTK window instead: this paints correctly and works on both Wayland and X11.
    #[cfg(target_os = "linux")]
    let webview = {
        let container = window
            .default_vbox()
            .ok_or_else(|| anyhow::anyhow!("tao window has no default GTK container"))?;
        webview_builder.build_gtk(container)?
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
                    let _ = webview.evaluate_script("location.hash='settings/general'; window.dispatchEvent(new PopStateEvent('popstate'));");
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
