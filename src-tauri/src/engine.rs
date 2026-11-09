//! Bounded, asynchronous supervisor for the local Python JSONL engine.
use crate::protocol::{
    self, ErrorCode, JsonlDecoder, Language, Request, Response, TranscriptionProvider,
};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::{mpsc, oneshot, watch},
    time::{self, Instant},
};
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub provider: TranscriptionProvider,
    pub python_path: PathBuf,
    pub working_dir: PathBuf,
    pub model_path: PathBuf,
    pub audio_root: PathBuf,
}
#[derive(Debug, Clone, Copy)]
pub struct EngineTimeouts {
    pub startup: Duration,
    pub transcription: Duration,
    pub cancel: Duration,
}
impl Default for EngineTimeouts {
    fn default() -> Self {
        Self {
            startup: Duration::from_secs(60),
            transcription: Duration::from_secs(120),
            cancel: Duration::from_secs(3),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    pub code: ErrorCode,
}
impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Moteur local indisponible ({:?})", self.code)
    }
}
impl std::error::Error for EngineError {}
fn error(code: ErrorCode) -> EngineError {
    EngineError { code }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    Ready,
    Result {
        request_id: String,
        text: String,
        language: Language,
        duration_ms: u64,
    },
    Error {
        request_id: Option<String>,
        code: ErrorCode,
    },
    Cancelled {
        request_id: String,
    },
    Stopped,
}
#[derive(Clone)]
pub struct EngineClient {
    tx: mpsc::Sender<Control>,
    done: watch::Receiver<bool>,
}
enum Control {
    Transcribe(Request, oneshot::Sender<Result<(), EngineError>>),
    Cancel(String, oneshot::Sender<Result<(), EngineError>>),
    Shutdown,
}
impl EngineClient {
    pub async fn start(
        config: EngineConfig,
        timeouts: EngineTimeouts,
    ) -> Result<(Self, mpsc::Receiver<EngineEvent>), EngineError> {
        let (ready, ready_rx) = oneshot::channel();
        let result = Self::spawn_engine(config, timeouts, Some(ready))?;
        ready_rx
            .await
            .map_err(|_| error(ErrorCode::EngineCrashed))??;
        Ok(result)
    }
    /// Launch immediately; readiness and failures arrive through the event channel.
    /// Requires an active Tokio runtime. The returned handle can shut down startup.
    pub fn launch(
        config: EngineConfig,
        timeouts: EngineTimeouts,
    ) -> Result<(Self, mpsc::Receiver<EngineEvent>), EngineError> {
        Self::spawn_engine(config, timeouts, None)
    }
    fn spawn_engine(
        config: EngineConfig,
        timeouts: EngineTimeouts,
        ready: Option<oneshot::Sender<Result<(), EngineError>>>,
    ) -> Result<(Self, mpsc::Receiver<EngineEvent>), EngineError> {
        for path in [&config.working_dir, &config.model_path, &config.audio_root] {
            if !path.is_absolute()
                || !path.is_dir()
                || path.canonicalize().ok().as_ref() != Some(path)
            {
                return Err(error(ErrorCode::ModelNotFound));
            }
        }
        if !config.python_path.is_absolute() {
            return Err(error(ErrorCode::EngineCrashed));
        }
        let mut command = Command::new(&config.python_path);
        command
            .args(["-u", "-m", "engine.main", "--model-path"])
            .arg(&config.model_path)
            .arg("--audio-root")
            .arg(&config.audio_root)
            .args(["--provider", config.provider.as_str()])
            .args(["--device", "cpu", "--compute-type", "int8"])
            .current_dir(&config.working_dir)
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1")
            .env_remove("PYTHONPATH")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let child = command
            .spawn()
            .map_err(|_| error(ErrorCode::EngineCrashed))?;
        let (tx, rx) = mpsc::channel(8);
        let (events, event_rx) = mpsc::channel(32);
        let (done, done_rx) = watch::channel(false);
        tokio::spawn(run(child, config, timeouts, rx, events, ready, done));
        Ok((Self { tx, done: done_rx }, event_rx))
    }
    pub async fn transcribe(&self, request: Request) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Control::Transcribe(request, tx))
            .await
            .map_err(|_| error(ErrorCode::EngineCrashed))?;
        rx.await.map_err(|_| error(ErrorCode::EngineCrashed))?
    }
    pub async fn cancel(&self, request_id: &str) -> Result<(), EngineError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Control::Cancel(request_id.into(), tx))
            .await
            .map_err(|_| error(ErrorCode::EngineCrashed))?;
        rx.await.map_err(|_| error(ErrorCode::EngineCrashed))?
    }
    pub async fn shutdown(&self) -> Result<(), EngineError> {
        let mut done = self.done.clone();
        if *done.borrow() {
            return Ok(());
        }
        let _ = self.tx.send(Control::Shutdown).await;
        while !*done.borrow_and_update() {
            if done.changed().await.is_err() {
                break;
            }
        }
        Ok(())
    }
}
struct Active {
    id: String,
    cancelling: bool,
}
async fn run(
    mut child: tokio::process::Child,
    config: EngineConfig,
    t: EngineTimeouts,
    mut controls: mpsc::Receiver<Control>,
    events: mpsc::Sender<EngineEvent>,
    ready: Option<oneshot::Sender<Result<(), EngineError>>>,
    done: watch::Sender<bool>,
) {
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let (lines_tx, mut lines) = mpsc::channel(8);
    let reader = tokio::spawn(async move {
        let mut decoder = JsonlDecoder::default();
        let mut chunk = [0; 8192];
        loop {
            match stdout.read(&mut chunk).await {
                Ok(0) => {
                    let code = if decoder.finish().is_ok() {
                        ErrorCode::EngineCrashed
                    } else {
                        ErrorCode::ProtocolError
                    };
                    let _ = lines_tx.send(Err(code)).await;
                    break;
                }
                Ok(n) => {
                    for line in decoder.push(&chunk[..n]) {
                        let result = line
                            .and_then(|b| protocol::parse_response(&b))
                            .map_err(|_| ErrorCode::ProtocolError);
                        if lines_tx.send(result).await.is_err() {
                            return;
                        }
                    }
                }
                Err(_) => {
                    let _ = lines_tx.send(Err(ErrorCode::EngineCrashed)).await;
                    break;
                }
            }
        }
    });
    let drain = tokio::spawn(async move {
        let mut chunk = [0; 8192];
        while let Ok(n) = stderr.read(&mut chunk).await {
            if n == 0 {
                break;
            }
        }
    });
    let mut ready = ready;
    let mut startup = true;
    let mut active: Option<Active> = None;
    let mut deadline = Instant::now() + t.startup;
    let mut failed: Option<ErrorCode> = None;
    loop {
        tokio::select! {
            biased;
            _ = async {
                if let Some(waiter) = ready.as_mut() {
                    waiter.closed().await
                } else {
                    std::future::pending::<()>().await
                }
            } => break,
            _ = time::sleep_until(deadline), if startup || active.is_some() => {
                failed = Some(ErrorCode::EngineTimeout);
                break;
            },
            command = controls.recv() => match command {
                None | Some(Control::Shutdown) => break,
                Some(Control::Transcribe(request, ack)) => {
                    let invalid = if startup || active.is_some() {
                        Some(ErrorCode::EngineBusy)
                    } else if !matches!(&request, Request::Transcribe { .. })
                        || request.validate_model_path(&config.model_path.to_string_lossy()).is_err()
                    {
                        Some(ErrorCode::ProtocolError)
                    } else {
                        None
                    };
                    if let Some(code) = invalid {
                        let _ = ack.send(Err(error(code)));
                        continue;
                    }
                    let raw = match protocol::encode_request(&request) {
                        Ok(raw) => raw,
                        Err(_) => {
                            let _ = ack.send(Err(error(ErrorCode::ProtocolError)));
                            continue;
                        }
                    };
                    match time::timeout(t.cancel, stdin.write_all(&raw)).await {
                        Ok(Ok(())) => {
                            active = Some(Active { id: request.request_id().into(), cancelling: false });
                            deadline = Instant::now() + t.transcription;
                            let _ = ack.send(Ok(()));
                        },
                        _ => {
                            let _ = ack.send(Err(error(ErrorCode::EngineCrashed)));
                            failed = Some(ErrorCode::EngineCrashed);
                            break;
                        }
                    }
                },
                Some(Control::Cancel(id, ack)) => {
                    if let Some(current) = active.as_mut().filter(|current| current.id == id) {
                        if !current.cancelling {
                            // Invalidate before the write: no late result can be delivered.
                            current.cancelling = true;
                            deadline = Instant::now() + t.cancel;
                            let request = Request::Cancel { protocol_version: 1, request_id: id };
                            let raw = protocol::encode_request(&request).unwrap();
                            if !matches!(time::timeout(t.cancel, stdin.write_all(&raw)).await, Ok(Ok(()))) {
                                let _ = ack.send(Err(error(ErrorCode::EngineTimeout)));
                                failed = Some(ErrorCode::EngineTimeout);
                                break;
                            }
                        }
                    }
                    let _ = ack.send(Ok(()));
                }
            },
            _ = events.closed() => break,
            line = lines.recv() => match line {
                Some(Ok(Response::Ready { .. })) if startup => {
                    startup = false;
                    let _ = events.try_send(EngineEvent::Ready);
                    if let Some(waiter) = ready.take() {
                        if waiter.send(Ok(())).is_err() { break; }
                    }
                },
                Some(Ok(Response::Ready { .. })) => {
                    failed = Some(ErrorCode::ProtocolError);
                    break;
                },
                Some(Ok(Response::Error { request_id, code, .. })) if startup && request_id == "startup" => {
                    failed = Some(code);
                    break;
                },
                Some(Ok(response)) => {
                    if startup {
                        failed = Some(ErrorCode::ProtocolError);
                        break;
                    }
                    if let Some(current) = active.as_ref().filter(|current| current.id == response.request_id()) {
                        let event = match response {
                            Response::Cancelled { request_id, .. } if current.cancelling =>
                                Some(EngineEvent::Cancelled { request_id }),
                            Response::Result { request_id, text, language, duration_ms, .. } if !current.cancelling =>
                                Some(EngineEvent::Result { request_id, text, language, duration_ms }),
                            Response::Error { request_id, code, .. } if !current.cancelling =>
                                Some(EngineEvent::Error { request_id: Some(request_id), code }),
                            _ => None
                        };
                        if let Some(event) = event {
                            active = None;
                            if events.try_send(event).is_err() { break; }
                        }
                    }
                },
                Some(Err(code)) => { failed = Some(code); break; },
                None => { failed = Some(ErrorCode::EngineCrashed); break; }
            }
        }
    }
    // kill + wait, not merely kill_on_drop: no zombies and no child outlives shutdown.
    drop(stdin);
    let _ = child.kill().await;
    let _ = child.wait().await;
    reader.abort();
    drain.abort();
    let _ = reader.await;
    let _ = drain.await;
    if let Some(code) = failed {
        if let Some(ready) = ready.take() {
            let _ = ready.send(Err(error(code)));
        } else {
            let _ = events.try_send(EngineEvent::Error {
                request_id: active.map(|a| a.id),
                code,
            });
        }
    }
    let _ = events.try_send(EngineEvent::Stopped);
    let _ = done.send(true);
}
