use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    io::Read,
    process::{Command, Stdio},
    sync::{mpsc::SyncSender, Mutex, OnceLock},
};
use tiny_http::{Header, Method, Response, Server};

static STATE: OnceLock<Mutex<State>> = OnceLock::new();
struct State {
    snapshot: Value,
    events: VecDeque<Value>,
    sequence: u64,
}

pub fn publish(event: &Value) {
    let Some(state) = STATE.get() else { return };
    let mut state = state.lock().unwrap();
    for key in ["root", "config", "files", "active", "paused", "building", "success", "message"] {
        if let Some(value) = event.get(key) {
            state.snapshot[key] = value.clone();
        }
    }
    if event["type"] == "files" || event["type"] == "config" || event["type"] == "ready" {
        return;
    }
    state.sequence += 1;
    let mut event = event.clone();
    event["id"] = json!(state.sequence);
    state.events.push_back(event);
    if state.events.len() > 2000 {
        state.events.pop_front();
    }
}

pub fn start(port: u16, open: bool, tx: SyncSender<Value>) -> Result<(), String> {
    let server = Server::http(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    let address = server.server_addr().to_string();
    let mut random = [0u8; 32];
    std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut random)).map_err(|e| e.to_string())?;
    let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let url = format!("http://{address}/#{token}");
    STATE.set(Mutex::new(State { snapshot: json!({}), events: VecDeque::new(), sequence: 0 })).map_err(|_| "dashboard already started")?;
    eprintln!("TrpHot dashboard: {url}\nPress Ctrl+C to stop.");
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let host_ok = request.headers().iter().any(|h| h.field.equiv("Host") && h.value.as_str() == address);
            let authorized = request.headers().iter().any(|h| h.field.equiv("X-Trphot-Token") && h.value.as_str() == token);
            let path = request.url().split('?').next().unwrap_or("");
            let (status, mime, body) = if !host_ok {
                (403, "text/plain", "Invalid host".into())
            } else if request.method() == &Method::Get && path == "/" {
                (200, "text/html; charset=utf-8", include_str!("../web/index.html").into())
            } else if request.method() == &Method::Get && path == "/app.js" {
                (200, "text/javascript", include_str!("../web/app.js").into())
            } else if request.method() == &Method::Get && path == "/style.css" {
                (200, "text/css", include_str!("../web/style.css").into())
            } else if !authorized {
                (403, "text/plain", "Invalid dashboard token".into())
            } else if request.method() == &Method::Get && path == "/api/state" {
                let state = STATE.get().unwrap().lock().unwrap();
                (200, "application/json", json!({"state":state.snapshot,"events":state.events}).to_string())
            } else if request.method() == &Method::Post && path == "/api/command" {
                let mut body = String::new();
                let result = request.as_reader().take(65537).read_to_string(&mut body);
                if result.is_err() || body.len() > 65536 {
                    (400, "text/plain", "Invalid command body".into())
                } else {
                    match serde_json::from_str::<Value>(&body) {
                        Ok(value) if ["arm", "pause", "build", "stop", "config", "save"].contains(&value["cmd"].as_str().unwrap_or("")) => {
                            match tx.try_send(value) {
                                Ok(()) => (202, "application/json", "{}".into()),
                                Err(_) => (503, "text/plain", "Engine busy; try again".into()),
                            }
                        }
                        _ => (400, "text/plain", "Invalid command".into()),
                    }
                }
            } else {
                (404, "text/plain", "Not found".into())
            };
            let response = Response::from_string(body).with_status_code(status)
                .with_header(Header::from_bytes("Content-Type", mime).unwrap())
                .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
                .with_header(Header::from_bytes("X-Content-Type-Options", "nosniff").unwrap())
                .with_header(Header::from_bytes("Content-Security-Policy", "default-src 'self'; frame-ancestors 'none'; base-uri 'none'").unwrap());
            let _ = request.respond(response);
        }
    });
    if open {
        let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        match Command::new(opener).arg(url).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(mut child) => { std::thread::spawn(move || { let _ = child.wait(); }); }
            Err(_) => eprintln!("Open the dashboard URL in your browser."),
        }
    }
    Ok(())
}
