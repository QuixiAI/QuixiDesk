//! Wire-level regression coverage for steering versus queued agent messages.
use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(15);

fn user_texts(request: &Value) -> Vec<String> {
    request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["role"] == "user")
        .map(|message| match &message["content"] {
            Value::String(text) => text.clone(),
            Value::Array(blocks) => blocks
                .iter()
                .filter_map(|block| block["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            other => panic!("unexpected user content: {other}"),
        })
        .collect()
}

#[test]
fn active_agent_steering_precedes_queued_followup_on_openai_wire() {
    steering_wire(false);
}

#[test]
fn active_code_steering_precedes_queued_followup_on_openai_wire() {
    steering_wire(true);
}

fn steering_wire(code: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let (requests_tx, requests_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        for index in 0..4 {
            let deadline = Instant::now() + TIMEOUT;
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "request {index} never arrived");
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("accept: {error}"),
                }
            };
            stream.set_read_timeout(Some(TIMEOUT)).unwrap();
            stream.set_write_timeout(Some(TIMEOUT)).unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = None;
            loop {
                let mut line = String::new();
                assert_ne!(
                    reader.read_line(&mut line).unwrap(),
                    0,
                    "incomplete headers"
                );
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = Some(value.trim().parse::<usize>().unwrap());
                }
            }
            let mut bytes = vec![0; length.expect("request Content-Length")];
            reader.read_exact(&mut bytes).unwrap();
            requests_tx
                .send(serde_json::from_slice::<Value>(&bytes).unwrap())
                .unwrap();
            // No response bytes are sent until the test has inspected this request.
            release_rx
                .recv_timeout(TIMEOUT)
                .expect("response gate timed out");
            let chunk = json!({"choices":[{"delta":{"content":format!("fixture answer {index}")},"finish_reason":"stop"}]});
            let body = format!("data: {chunk}\n\ndata: [DONE]\n\n");
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        }
    });

    let root = std::env::temp_dir().join(format!("quixi-steering-wire-{}", Uuid::new_v4()));
    let engine = QuixiEngine::new_with_data_dir(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources"),
        root.clone(),
    );
    let saved = engine.provider_manager.save_provider(&json!({
        "providerId":"custom", "baseUrl":url, "apiFormat":"openai",
        "selectedModel":"fixture", "activate":true
    }));
    assert!(saved.get("error").is_none(), "{saved}");
    let sid = if code {engine.start_code_session(&json!({"model":"fixture"}))} else {engine.start_agent_session(&json!({"model":"fixture"}))};
    let running = || if code {engine.code_sessions.read()[&sid].turn_running} else {engine.agent_sessions.read()[&sid].is_running};
    engine
        .send_chat_message(&sid, &json!({"text":"initial request"}))
        .unwrap();
    let first = requests_rx.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(user_texts(&first), ["initial request"]);
    assert!(running());

    // Enqueue the followup first: steering must bypass it, not drain the FIFO.
    for (id, text, delivery) in [
        ("followup-id", "queued followup", "queue"),
        ("steering-id", "steering correction", "steer"),
        ("followup-2", "second followup", "queue"),
    ] {
        let accepted = engine
            .send_chat_message(
                &sid,
                &json!({
                    "text":text, "messageUuid":id, "delivery":delivery
                }),
            )
            .unwrap();
        assert_eq!(accepted["queued"], true);
        assert_eq!(accepted["delivery"], delivery);
        assert_eq!(accepted["messageUuid"], id);
    }
    assert_eq!(
        engine.get_pending_messages(&sid),
        json!([
            {"messageUuid":"followup-id","text":"queued followup","delivery":"queue"},
            {"messageUuid":"steering-id","text":"steering correction","delivery":"steer"},
            {"messageUuid":"followup-2","text":"second followup","delivery":"queue"}
        ])
    );
    release_tx.send(()).unwrap();

    let second = requests_rx.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(
        user_texts(&second),
        ["initial request", "steering correction"]
    );
    assert!(running());
    assert_eq!(
        engine.get_pending_messages(&sid),
        json!([
            {"messageUuid":"followup-id","text":"queued followup","delivery":"queue"},
            {"messageUuid":"followup-2","text":"second followup","delivery":"queue"}
        ])
    );
    release_tx.send(()).unwrap();

    let third = requests_rx.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(
        user_texts(&third),
        ["initial request", "steering correction", "queued followup"]
    );
    assert!(
        third["messages"].to_string().contains("fixture answer 1"),
        "followup must retain the completed steered turn's reply"
    );
    assert_eq!(engine.get_pending_messages(&sid).as_array().unwrap().len(), 1);
    release_tx.send(()).unwrap();
    let fourth = requests_rx.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(user_texts(&fourth), ["initial request", "steering correction", "queued followup", "second followup"]);
    assert_eq!(engine.get_pending_messages(&sid), json!([]));
    release_tx.send(()).unwrap();
    server.join().unwrap();

    let deadline = Instant::now() + TIMEOUT;
    while running() {
        if Instant::now() >= deadline {
            engine.cancel_turn(&sid);
            panic!("agent did not finish the queued followup");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if !code {assert!(engine.agent_sessions.read()[&sid].error.is_none());}
    drop(engine);
    fs::remove_dir_all(root).unwrap();
}
