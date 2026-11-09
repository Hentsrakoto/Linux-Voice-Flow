use linux_voice_flow::{
    engine::{EngineClient, EngineConfig, EngineEvent, EngineTimeouts},
    protocol::{ErrorCode, Language, Request, TranscriptionProvider},
};
use std::{path::PathBuf, time::Duration};
fn setup(mode: &str) -> (tempfile::TempDir, EngineConfig, EngineTimeouts) {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    std::fs::create_dir(p.join("engine")).unwrap();
    std::fs::write(p.join("engine/__init__.py"), "").unwrap();
    std::fs::write(
        p.join("engine/main.py"),
        include_str!("fixtures/fake_engine.py"),
    )
    .unwrap();
    std::fs::write(p.join("mode"), mode).unwrap();
    std::fs::create_dir(p.join("model")).unwrap();
    std::fs::create_dir(p.join("audio")).unwrap();
    let c = EngineConfig {
        provider: TranscriptionProvider::Local,
        python_path: PathBuf::from("/usr/bin/python3"),
        working_dir: p.into(),
        model_path: p.join("model"),
        audio_root: p.join("audio"),
    };
    (
        d,
        c,
        EngineTimeouts {
            startup: Duration::from_millis(500),
            transcription: Duration::from_millis(180),
            cancel: Duration::from_millis(80),
        },
    )
}
fn request(c: &EngineConfig) -> Request {
    Request::Transcribe {
        protocol_version: 1,
        request_id: "r1".into(),
        audio_path: c.audio_root.join("a.wav").to_string_lossy().into(),
        model_path: c.model_path.to_string_lossy().into(),
        language: Language::Fr,
        vad: true,
    }
}
async fn event(rx: &mut tokio::sync::mpsc::Receiver<EngineEvent>) -> EngineEvent {
    tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap()
}
fn reaped(d: &tempfile::TempDir) {
    let pid = std::fs::read_to_string(d.path().join("pid")).unwrap();
    assert!(!PathBuf::from(format!("/proc/{pid}")).exists());
}
#[tokio::test]
async fn result_and_shutdown_reap() {
    let (d, c, t) = setup("result");
    let req = request(&c);
    let (client, mut rx) = EngineClient::start(c, t).await.unwrap();
    assert_eq!(event(&mut rx).await, EngineEvent::Ready);
    client.transcribe(req).await.unwrap();
    assert!(matches!(event(&mut rx).await,EngineEvent::Result{text,..} if text=="bonjour"));
    client.shutdown().await.unwrap();
    assert_eq!(event(&mut rx).await, EngineEvent::Stopped);
    reaped(&d);
    client.shutdown().await.unwrap();
}
#[tokio::test]
async fn busy_cancel_suppresses_late_result() {
    let (d, c, t) = setup("cancel");
    let req = request(&c);
    let (client, mut rx) = EngineClient::start(c, t).await.unwrap();
    event(&mut rx).await;
    client.transcribe(req.clone()).await.unwrap();
    assert_eq!(
        client.transcribe(req).await.unwrap_err().code,
        ErrorCode::EngineBusy
    );
    client.cancel("r1").await.unwrap();
    assert_eq!(
        event(&mut rx).await,
        EngineEvent::Cancelled {
            request_id: "r1".into()
        }
    );
    client.shutdown().await.unwrap();
    assert_eq!(event(&mut rx).await, EngineEvent::Stopped);
    reaped(&d);
}
#[tokio::test]
async fn timeouts_and_bad_streams_reap() {
    for mode in ["timeout", "cancel_hang", "eof", "malformed", "oversize"] {
        let (d, c, t) = setup(mode);
        let req = request(&c);
        let (client, mut rx) = EngineClient::start(c, t).await.unwrap();
        assert_eq!(event(&mut rx).await, EngineEvent::Ready);
        if mode == "timeout" || mode == "cancel_hang" {
            client.transcribe(req).await.unwrap();
            if mode == "cancel_hang" {
                client.cancel("r1").await.unwrap();
            }
        }
        assert!(matches!(event(&mut rx).await, EngineEvent::Error { .. }));
        assert_eq!(event(&mut rx).await, EngineEvent::Stopped);
        client.shutdown().await.unwrap();
        reaped(&d);
    }
}
#[tokio::test]
async fn startup_failures_reap() {
    for (mode, code) in [
        ("startup_timeout", ErrorCode::EngineTimeout),
        ("startup_error", ErrorCode::ModelLoadFailed),
    ] {
        let (d, c, t) = setup(mode);
        assert_eq!(EngineClient::start(c, t).await.err().unwrap().code, code);
        reaped(&d);
    }
}
#[tokio::test]
async fn dropping_last_client_reaps() {
    let (d, c, t) = setup("timeout");
    let req = request(&c);
    let (client, mut rx) = EngineClient::start(c, t).await.unwrap();
    event(&mut rx).await;
    client.transcribe(req).await.unwrap();
    drop(client);
    assert_eq!(event(&mut rx).await, EngineEvent::Stopped);
    reaped(&d);
}
#[tokio::test]
async fn cancelling_start_future_reaps() {
    let (d, c, mut t) = setup("startup_timeout");
    t.startup = Duration::from_secs(30);
    let task = tokio::spawn(EngineClient::start(c, t));
    tokio::time::timeout(Duration::from_secs(2), async {
        while !d.path().join("pid").exists() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    let _ = task.await;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let pid = std::fs::read_to_string(d.path().join("pid")).unwrap();
            if !PathBuf::from(format!("/proc/{pid}")).exists() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn closed_event_receiver_reaps() {
    let (d, c, t) = setup("timeout");
    let (client, rx) = EngineClient::start(c, t).await.unwrap();
    drop(rx);
    client.shutdown().await.unwrap();
    reaped(&d);
}
#[tokio::test]
async fn invalid_config_rejected() {
    let (_d, mut c, t) = setup("result");
    c.model_path = PathBuf::from("relative");
    assert_eq!(
        EngineClient::start(c, t).await.err().unwrap().code,
        ErrorCode::ModelNotFound
    );
}
#[tokio::test]
async fn failed_engine_can_be_replaced_explicitly() {
    let (d, c, t) = setup("malformed");
    let (client, mut rx) = EngineClient::start(c.clone(), t).await.unwrap();
    assert_eq!(event(&mut rx).await, EngineEvent::Ready);
    assert!(matches!(
        event(&mut rx).await,
        EngineEvent::Error {
            code: ErrorCode::ProtocolError,
            ..
        }
    ));
    assert_eq!(event(&mut rx).await, EngineEvent::Stopped);
    client.shutdown().await.unwrap();
    reaped(&d);
    std::fs::write(d.path().join("mode"), "result").unwrap();
    let req = request(&c);
    let (client, mut rx) = EngineClient::start(c, t).await.unwrap();
    assert_eq!(event(&mut rx).await, EngineEvent::Ready);
    client.transcribe(req).await.unwrap();
    assert!(matches!(event(&mut rx).await, EngineEvent::Result { .. }));
    client.shutdown().await.unwrap();
    reaped(&d);
}
#[tokio::test]
async fn shutdown_with_result_backlog_is_bounded() {
    let (d, c, t) = setup("result");
    let req = request(&c);
    let (client, mut rx) = EngineClient::start(c, t).await.unwrap();
    event(&mut rx).await;
    // Consumer intentionally stops draining. Admission stays bounded and shutdown cannot wait on event capacity.
    for _ in 0..80 {
        let _ = client.transcribe(req.clone()).await;
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(
        client.transcribe(req).await.unwrap_err().code,
        ErrorCode::EngineCrashed
    );
    tokio::time::timeout(Duration::from_secs(2), client.shutdown())
        .await
        .unwrap()
        .unwrap();
    reaped(&d);
}
#[tokio::test]
async fn blocked_stdin_cannot_prevent_cleanup() {
    let (d, c, t) = setup("pipe_hang");
    let mut req = request(&c);
    if let Request::Transcribe { audio_path, .. } = &mut req {
        *audio_path = format!("/{}", "x".repeat(200_000));
    }
    let (client, mut rx) = EngineClient::start(c, t).await.unwrap();
    event(&mut rx).await;
    let result = tokio::time::timeout(Duration::from_secs(2), client.transcribe(req))
        .await
        .unwrap();
    assert!(result.is_err());
    client.shutdown().await.unwrap();
    reaped(&d);
}
#[tokio::test]
async fn launch_then_shutdown_before_ready_no_orphan() {
    let (d, c, mut t) = setup("startup_timeout");
    t.startup = Duration::from_secs(30);
    let (client, _rx) = EngineClient::launch(c, t).unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while !d.path().join("pid").exists() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(2), client.shutdown())
        .await
        .unwrap()
        .unwrap();
    reaped(&d);
}
