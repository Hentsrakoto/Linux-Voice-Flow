//! Dictation orchestration, guarded X11 delivery and ephemeral results.
use crate::{
    audio::{AudioBuffer, AudioError, Microphone},
    engine::{EngineClient, EngineConfig, EngineEvent, EngineTimeouts},
    hotkey::HotkeyEvent,
    insertion::{DesktopOutput, FocusTarget, NoopOutput, OutputDiagnostics},
    protocol::{ErrorCode, Language, Request, TranscriptionProvider},
    temp_audio::{TempAudioHandle, TempAudioStore},
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot};
use zeroize::Zeroize;

pub trait AudioCapture: Send + 'static {
    fn list_devices(&self) -> Result<Vec<Microphone>, AudioError>;
    fn start(&mut self, device: &str) -> Result<(), AudioError>;
    fn stop(&mut self) -> Result<AudioBuffer, AudioError>;
    fn discard(&mut self);
    fn poll_error(&self) -> Option<AudioError>;
}
#[cfg(feature = "native-audio")]
impl AudioCapture for crate::audio::NativeAudioRecorder {
    fn list_devices(&self) -> Result<Vec<Microphone>, AudioError> {
        self.list_devices()
    }
    fn start(&mut self, device: &str) -> Result<(), AudioError> {
        self.start(device)
    }
    fn stop(&mut self) -> Result<AudioBuffer, AudioError> {
        self.stop()
    }
    fn discard(&mut self) {
        self.discard()
    }
    fn poll_error(&self) -> Option<AudioError> {
        self.poll_error()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    #[serde(default)]
    pub provider: TranscriptionProvider,
    pub model_path: String,
    pub microphone_id: String,
    pub language: Language,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Unconfigured,
    LoadingEngine,
    Ready,
    Recording,
    Transcribing,
    Cancelling,
    Inserting,
    Success,
    Error,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    None,
    Inserted,
    Clipboard,
    Preview,
    Fallback,
}
#[derive(Debug, Clone, Copy)]
pub struct OutputOptions {
    pub auto_insert_enabled: bool,
    pub clipboard_auto_copy_consent: bool,
    pub clipboard_fallback: bool,
}
impl Default for OutputOptions {
    fn default() -> Self {
        Self {
            auto_insert_enabled: false,
            clipboard_auto_copy_consent: false,
            clipboard_fallback: true,
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct DictationSnapshot {
    pub provider: TranscriptionProvider,
    pub delivery: Delivery,
    pub clipboard_owned: bool,
    pub insertion_uncertain: bool,
    pub auto_insert_enabled: bool,
    pub revision: u64,
    pub phase: Phase,
    pub engine_ready: bool,
    pub model_id: Option<String>,
    pub session_id: Option<u64>,
    pub microphone_id: String,
    pub language: Language,
    pub recording_ms: u64,
    pub audio_duration_ms: Option<u64>,
    pub text: Option<String>,
    pub error_code: Option<ErrorCode>,
    pub reason: Option<&'static str>,
    pub message: String,
}
impl Default for DictationSnapshot {
    fn default() -> Self {
        Self {
            provider: TranscriptionProvider::Local,
            delivery: Delivery::None,
            clipboard_owned: false,
            insertion_uncertain: false,
            auto_insert_enabled: false,
            revision: 0,
            phase: Phase::Unconfigured,
            engine_ready: false,
            model_id: None,
            session_id: None,
            microphone_id: "default".into(),
            language: Language::Fr,
            recording_ms: 0,
            audio_duration_ms: None,
            text: None,
            error_code: None,
            reason: None,
            message: "Choisissez un modèle local et un microphone pour préparer le moteur.".into(),
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeError {
    pub code: ErrorCode,
    pub message: String,
}
fn failure(code: ErrorCode) -> RuntimeError {
    let message = match code {
        ErrorCode::MicUnavailable => "Microphone indisponible. Vérifiez sa connexion et le périphérique choisi.",
        ErrorCode::NoSpeech => "Aucune parole détectée.",
        ErrorCode::ModelNotFound => "Modèle local introuvable. Choisissez son répertoire complet.",
        ErrorCode::ModelLoadFailed => "Modèle impossible à charger. Vérifiez le modèle et les dépendances Python.",
        ErrorCode::ApiConfigError => "Configuration Groq absente ou invalide. Vérifiez GROQ_API_KEY et GROQ_MODEL dans .env puis préparez le moteur.",
        ErrorCode::ApiAuthError => "Groq refuse l’authentification. Vérifiez votre clé dans .env puis préparez le moteur.",
        ErrorCode::ApiRateLimit => "Quota Groq atteint. Attendez avant de réessayer ou choisissez le mode local.",
        ErrorCode::ApiUnavailable => "Groq est indisponible. Vérifiez la connexion et le modèle configuré ou choisissez le mode local.",
        ErrorCode::EngineBusy => "Une opération est déjà en cours. Relâchez le raccourci ou annulez avant de recommencer.",
        ErrorCode::EngineTimeout => "Le moteur a dépassé le délai autorisé. Préparez-le à nouveau.",
        ErrorCode::EngineCrashed => "Le moteur local est arrêté. Vérifiez Python puis préparez-le à nouveau.",
        ErrorCode::Cancelled => "Dictée annulée.",
        _ => "Opération locale impossible. Vérifiez les prérequis puis réessayez.",
    };
    RuntimeError {
        code,
        message: message.into(),
    }
}
type Reply = oneshot::Sender<Result<DictationSnapshot, RuntimeError>>;
enum Control {
    Status(Reply),
    Configure(RuntimeConfig, Reply),
    Cancel(Reply),
    Clear(Reply),
    Microphones(oneshot::Sender<Result<Vec<Microphone>, RuntimeError>>),
    Copy(Reply),
    ClearClipboard(Reply),
    OutputDiagnostics(oneshot::Sender<OutputDiagnostics>),
    IdleAction(Box<dyn FnOnce() -> Result<(), RuntimeError> + Send>, Reply),
    Options(
        OutputOptions,
        Box<dyn FnOnce() -> Result<(), RuntimeError> + Send>,
        Reply,
    ),
    Hotkey(HotkeyEvent, u64),
}

pub struct DictationService {
    tx: mpsc::Sender<Control>,
    stopping: Arc<AtomicBool>,
    urgent_cancel: Arc<AtomicBool>,
    input_epoch: Arc<AtomicU64>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
    busy_guard: Arc<AtomicBool>,
}
impl DictationService {
    pub fn spawn(
        working_dir: PathBuf,
        python_path: PathBuf,
        audio_root: PathBuf,
        audio: Box<dyn AudioCapture>,
        emit: Arc<dyn Fn(DictationSnapshot) + Send + Sync>,
    ) -> Result<Self, RuntimeError> {
        Self::spawn_with_output(
            working_dir,
            python_path,
            audio_root,
            audio,
            emit,
            Box::new(NoopOutput),
            OutputOptions::default(),
        )
    }
    pub fn spawn_with_output(
        working_dir: PathBuf,
        python_path: PathBuf,
        audio_root: PathBuf,
        audio: Box<dyn AudioCapture>,
        emit: Arc<dyn Fn(DictationSnapshot) + Send + Sync>,
        output: Box<dyn DesktopOutput>,
        options: OutputOptions,
    ) -> Result<Self, RuntimeError> {
        let store =
            TempAudioStore::new(&audio_root).map_err(|_| failure(ErrorCode::ProtocolError))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| failure(ErrorCode::EngineCrashed))?;
        let (tx, rx) = mpsc::channel(32);
        let stopping = Arc::new(AtomicBool::new(false));
        let urgent_cancel = Arc::new(AtomicBool::new(false));
        let input_epoch = Arc::new(AtomicU64::new(0));
        let busy_guard = Arc::new(AtomicBool::new(false));
        let worker_busy = busy_guard.clone();
        let (stop, cancel, epoch) = (stopping.clone(), urgent_cancel.clone(), input_epoch.clone());
        let worker = thread::Builder::new()
            .name("dictation-controller".into())
            .spawn(move || {
                runtime.block_on(
                    Actor::new(working_dir, python_path, store, audio, emit)
                        .with_output(output, options)
                        .with_busy_guard(worker_busy)
                        .run(rx, stop, cancel, epoch),
                );
            })
            .map_err(|_| failure(ErrorCode::EngineCrashed))?;
        Ok(Self {
            tx,
            stopping,
            urgent_cancel,
            input_epoch,
            worker: Mutex::new(Some(worker)),
            busy_guard,
        })
    }
    async fn request(
        &self,
        command: impl FnOnce(Reply) -> Control,
    ) -> Result<DictationSnapshot, RuntimeError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(command(tx))
            .await
            .map_err(|_| failure(ErrorCode::EngineCrashed))?;
        rx.await.map_err(|_| failure(ErrorCode::EngineCrashed))?
    }
    pub fn ensure_idle(&self) -> Result<(), RuntimeError> {
        if self.busy_guard.load(Ordering::Acquire) {
            Err(failure(ErrorCode::EngineBusy))
        } else {
            Ok(())
        }
    }
    pub async fn status(&self) -> Result<DictationSnapshot, RuntimeError> {
        self.request(Control::Status).await
    }
    pub async fn configure(
        &self,
        config: RuntimeConfig,
    ) -> Result<DictationSnapshot, RuntimeError> {
        self.ensure_idle()?;
        self.request(|tx| Control::Configure(config, tx)).await
    }
    pub async fn cancel(&self) -> Result<DictationSnapshot, RuntimeError> {
        self.request_cancel();
        self.request(Control::Cancel).await
    }
    pub async fn clear(&self) -> Result<DictationSnapshot, RuntimeError> {
        self.ensure_idle()?;
        self.request(Control::Clear).await
    }
    pub async fn copy_result(&self) -> Result<DictationSnapshot, RuntimeError> {
        self.ensure_idle()?;
        self.request(Control::Copy).await
    }
    pub async fn clear_clipboard(&self) -> Result<DictationSnapshot, RuntimeError> {
        self.ensure_idle()?;
        self.request(Control::ClearClipboard).await
    }
    pub async fn output_diagnostics(&self) -> Result<OutputDiagnostics, RuntimeError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Control::OutputDiagnostics(tx))
            .await
            .map_err(|_| failure(ErrorCode::EngineCrashed))?;
        rx.await.map_err(|_| failure(ErrorCode::EngineCrashed))
    }
    /// Serialized with capture start, so a settings transaction cannot race a session.
    pub async fn apply_options(
        &self,
        options: OutputOptions,
        apply: impl FnOnce() -> Result<(), RuntimeError> + Send + 'static,
    ) -> Result<DictationSnapshot, RuntimeError> {
        self.ensure_idle()?;
        self.request(|reply| Control::Options(options, Box::new(apply), reply))
            .await
    }
    /// Recheck inactivity on the actor before applying a runtime-only change.
    pub async fn apply_when_idle(
        &self,
        apply: impl FnOnce() -> Result<(), RuntimeError> + Send + 'static,
    ) -> Result<DictationSnapshot, RuntimeError> {
        self.ensure_idle()?;
        self.request(|reply| Control::IdleAction(Box::new(apply), reply))
            .await
    }
    pub async fn microphones(&self) -> Result<Vec<Microphone>, RuntimeError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Control::Microphones(tx))
            .await
            .map_err(|_| failure(ErrorCode::EngineCrashed))?;
        rx.await.map_err(|_| failure(ErrorCode::EngineCrashed))?
    }
    /// Called only by the native relay, never on the X11 callback or UI thread.
    pub fn hotkey(&self, event: HotkeyEvent) {
        if event == HotkeyEvent::Pressed && self.busy_guard.load(Ordering::Acquire) {
            return;
        }
        let epoch = self.input_epoch.load(Ordering::Acquire);
        if self.tx.try_send(Control::Hotkey(event, epoch)).is_err() {
            // Never silently lose a release when the queue is saturated.
            self.request_cancel();
        }
    }
    pub fn request_cancel(&self) {
        self.input_epoch.fetch_add(1, Ordering::AcqRel);
        self.urgent_cancel.store(true, Ordering::Release);
    }
    pub fn shutdown(&self) {
        self.stopping.store(true, Ordering::Release);
        if let Some(worker) = self.worker.lock().ok().and_then(|mut h| h.take()) {
            let _ = worker.join();
        }
    }
}
impl Drop for DictationService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct Actor {
    working_dir: PathBuf,
    python_path: PathBuf,
    store: TempAudioStore,
    audio: Box<dyn AudioCapture>,
    emit: Arc<dyn Fn(DictationSnapshot) + Send + Sync>,
    output: Box<dyn DesktopOutput>,
    options: OutputOptions,
    target: Option<FocusTarget>,
    stop_guard: Arc<AtomicBool>,
    cancel_guard: Arc<AtomicBool>,
    epoch_guard: Arc<AtomicU64>,
    session_epoch: u64,
    busy_guard: Arc<AtomicBool>,
    snapshot: DictationSnapshot,
    engine: Option<EngineClient>,
    events: Option<mpsc::Receiver<EngineEvent>>,
    model_path: Option<PathBuf>,
    wav: Option<TempAudioHandle>,
    cancelling_id: Option<String>,
    next_id: u64,
    recording_since: Option<Instant>,
    result_until: Option<Instant>,
}
impl Actor {
    fn new(
        working_dir: PathBuf,
        python_path: PathBuf,
        store: TempAudioStore,
        audio: Box<dyn AudioCapture>,
        emit: Arc<dyn Fn(DictationSnapshot) + Send + Sync>,
    ) -> Self {
        Self {
            working_dir,
            python_path,
            store,
            audio,
            emit,
            output: Box::new(NoopOutput),
            options: OutputOptions::default(),
            target: None,
            stop_guard: Arc::new(AtomicBool::new(false)),
            cancel_guard: Arc::new(AtomicBool::new(false)),
            epoch_guard: Arc::new(AtomicU64::new(0)),
            session_epoch: 0,
            busy_guard: Arc::new(AtomicBool::new(false)),
            snapshot: DictationSnapshot::default(),
            engine: None,
            events: None,
            model_path: None,
            wav: None,
            cancelling_id: None,
            next_id: 0,
            recording_since: None,
            result_until: None,
        }
    }
    fn with_output(mut self, output: Box<dyn DesktopOutput>, options: OutputOptions) -> Self {
        self.output = output;
        self.options = options;
        self.snapshot.auto_insert_enabled = options.auto_insert_enabled;
        self
    }
    fn with_busy_guard(mut self, guard: Arc<AtomicBool>) -> Self {
        self.busy_guard = guard;
        self
    }
    fn busy(&self) -> bool {
        matches!(
            self.snapshot.phase,
            Phase::LoadingEngine
                | Phase::Recording
                | Phase::Transcribing
                | Phase::Inserting
                | Phase::Cancelling
        )
    }
    fn copy_result(&mut self) -> Result<DictationSnapshot, RuntimeError> {
        if self.busy() {
            return Err(failure(ErrorCode::EngineBusy));
        }
        if self
            .result_until
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.expire_result();
        }
        let text = self
            .snapshot
            .text
            .as_deref()
            .filter(|text| !text.is_empty())
            .ok_or_else(|| failure(ErrorCode::ProtocolError))?;
        self.output.copy(text).map_err(failure)?;
        self.snapshot.clipboard_owned = true;
        if self.snapshot.delivery != Delivery::Inserted {
            self.snapshot.delivery = Delivery::Clipboard;
        }
        self.snapshot.message = if self.snapshot.insertion_uncertain {
            "Texte copié. Vérifiez le champ cible avant collage : une partie peut déjà y apparaître."
        } else { "Texte copié. Collez-le dans l’application de votre choix." }.into();
        self.publish();
        Ok(self.snapshot.clone())
    }
    fn expire_result(&mut self) {
        self.clear_text();
        self.snapshot.session_id = None;
        self.snapshot.phase = if self.snapshot.engine_ready {
            Phase::Ready
        } else {
            Phase::Unconfigured
        };
        self.snapshot.message = "Résultat effacé après deux minutes.".into();
        self.publish();
    }
    fn update_options(
        &mut self,
        options: OutputOptions,
        apply: impl FnOnce() -> Result<(), RuntimeError>,
    ) -> Result<DictationSnapshot, RuntimeError> {
        self.apply_when_idle(apply)?;
        self.options = options;
        self.snapshot.auto_insert_enabled = options.auto_insert_enabled;
        self.publish();
        Ok(self.snapshot.clone())
    }
    fn apply_when_idle(
        &mut self,
        apply: impl FnOnce() -> Result<(), RuntimeError>,
    ) -> Result<DictationSnapshot, RuntimeError> {
        if self.busy() {
            return Err(failure(ErrorCode::EngineBusy));
        }
        apply()?;
        Ok(self.snapshot.clone())
    }
    fn clear_clipboard(&mut self) -> Result<DictationSnapshot, RuntimeError> {
        if self.busy() {
            return Err(failure(ErrorCode::EngineBusy));
        }
        self.output.clear_clipboard().map_err(failure)?;
        self.snapshot.clipboard_owned = false;
        self.publish();
        Ok(self.snapshot.clone())
    }
    fn deliver(&mut self) {
        let stop = self.stop_guard.clone();
        let cancel = self.cancel_guard.clone();
        let epoch = self.epoch_guard.clone();
        let expected = self.session_epoch;
        let cancelled = || {
            stop.load(Ordering::Acquire)
                || cancel.load(Ordering::Acquire)
                || epoch.load(Ordering::Acquire) != expected
        };
        if cancelled() {
            self.invalidate();
            self.snapshot.phase = Phase::Ready;
            self.snapshot.message = "Dictée annulée avant insertion.".into();
            self.publish();
            return;
        }
        self.snapshot.delivery = Delivery::Preview;
        self.snapshot.error_code = None;
        let mut fallback = false;
        if self.options.auto_insert_enabled {
            self.snapshot.phase = Phase::Inserting;
            self.snapshot.message = "Insertion dans la fenêtre cible…".into();
            self.publish();
            let result = match self.target.as_ref() {
                Some(target) => self.output.insert(
                    target,
                    self.snapshot.text.as_deref().unwrap_or_default(),
                    &cancelled,
                ),
                None => Err(ErrorCode::FocusChanged),
            };
            if cancelled() || matches!(result, Err(ErrorCode::Cancelled)) {
                self.invalidate();
                self.snapshot.phase = Phase::Ready;
                self.snapshot.message = "Dictée annulée. Aucune nouvelle insertion.".into();
                self.publish();
                return;
            }
            match result {
                Ok(()) => self.snapshot.delivery = Delivery::Inserted,
                Err(code) => {
                    self.snapshot.delivery = Delivery::Fallback;
                    self.snapshot.insertion_uncertain = true;
                    self.snapshot.error_code = Some(code);
                    fallback = true;
                }
            }
        }
        if self.snapshot.delivery != Delivery::Inserted
            && self.options.clipboard_auto_copy_consent
            && self.options.clipboard_fallback
            && !cancelled()
        {
            match self
                .output
                .copy(self.snapshot.text.as_deref().unwrap_or_default())
            {
                Ok(()) => {
                    self.snapshot.clipboard_owned = true;
                    self.snapshot.delivery = Delivery::Clipboard;
                }
                Err(code) => {
                    self.snapshot.error_code = Some(code);
                    self.snapshot.delivery = Delivery::Fallback;
                }
            }
        }
        if cancelled() {
            self.invalidate();
            self.snapshot.phase = Phase::Ready;
            self.snapshot.message = "Dictée annulée. Aucun texte conservé.".into();
            self.publish();
            return;
        }
        self.snapshot.phase = Phase::Success;
        self.snapshot.message = match self.snapshot.delivery {
            Delivery::Inserted => "Texte transmis à la fenêtre cible.",
            Delivery::Clipboard if self.snapshot.insertion_uncertain => "Texte copié. Vérifiez le champ cible avant collage : une partie peut déjà y apparaître.",
            Delivery::Clipboard => "Texte copié. Collez-le dans le champ souhaité.",
            Delivery::Fallback if fallback => "Insertion interrompue ou incertaine. Vérifiez le champ cible avant de copier le texte ; une partie peut déjà y apparaître.",
            _ => "Texte transcrit. Vous pouvez le copier ou le sélectionner.",
        }.into();
        self.publish();
    }
    fn publish(&mut self) {
        self.busy_guard.store(self.busy(), Ordering::Release);
        self.snapshot.revision = self.snapshot.revision.saturating_add(1);
        (self.emit)(self.snapshot.clone());
    }
    fn clear_text(&mut self) {
        if let Some(mut text) = self.snapshot.text.take() {
            text.zeroize();
        }
        self.result_until = None;
        self.snapshot.delivery = Delivery::None;
        self.snapshot.insertion_uncertain = false;
        if self.snapshot.clipboard_owned && self.output.clear_clipboard().is_ok() {
            self.snapshot.clipboard_owned = false;
        }
    }
    fn invalidate(&mut self) {
        self.audio.discard();
        self.snapshot.session_id = None;
        self.snapshot.reason = None;
        self.recording_since = None;
        self.wav.take();
        self.target = None;
        self.clear_text();
    }
    fn set_error(&mut self, code: ErrorCode) {
        let error = failure(code);
        self.snapshot.phase = Phase::Error;
        self.snapshot.error_code = Some(code);
        self.snapshot.reason = None;
        self.snapshot.message = error.message;
        self.publish();
    }
    async fn stop_engine(&mut self) {
        if let Some(engine) = self.engine.take() {
            let _ = engine.shutdown().await;
        }
        self.events = None;
        self.snapshot.engine_ready = false;
    }
    async fn configure(
        &mut self,
        config: RuntimeConfig,
    ) -> Result<DictationSnapshot, RuntimeError> {
        if matches!(
            self.snapshot.phase,
            Phase::Recording
                | Phase::Transcribing
                | Phase::Inserting
                | Phase::Cancelling
                | Phase::LoadingEngine
        ) {
            return Err(failure(ErrorCode::EngineBusy));
        }
        if config.model_path.len() > 4096
            || config.microphone_id.is_empty()
            || config.microphone_id.len() > 1024
        {
            return Err(failure(ErrorCode::ProtocolError));
        }
        let model = match config.provider {
            TranscriptionProvider::Local => {
                let supplied = PathBuf::from(&config.model_path);
                if !supplied.is_absolute() {
                    return Err(failure(ErrorCode::ModelNotFound));
                }
                let canonical = supplied
                    .canonicalize()
                    .map_err(|_| failure(ErrorCode::ModelNotFound))?;
                if supplied != canonical || !canonical.is_dir() {
                    return Err(failure(ErrorCode::ModelNotFound));
                }
                canonical
            }
            // The protocol keeps a session identity; no local weights are required.
            TranscriptionProvider::Groq => self.working_dir.clone(),
        };
        self.stop_engine().await;
        self.audio.discard();
        self.invalidate();
        self.snapshot.microphone_id = config.microphone_id;
        self.snapshot.language = config.language;
        self.snapshot.provider = config.provider;
        self.snapshot.phase = Phase::LoadingEngine;
        self.snapshot.error_code = None;
        self.snapshot.message = match config.provider {
            TranscriptionProvider::Local => {
                "Chargement du modèle local… Aucun enregistrement en cours."
            }
            TranscriptionProvider::Groq => "Préparation de Groq… Aucun enregistrement en cours.",
        }
        .into();
        self.snapshot.model_id = match config.provider {
            TranscriptionProvider::Local => {
                crate::models::installed_model_id(&self.working_dir, &model)
            }
            TranscriptionProvider::Groq => None,
        };
        self.model_path = Some(model.clone());
        self.publish();
        match EngineClient::launch(
            EngineConfig {
                provider: config.provider,
                python_path: self.python_path.clone(),
                working_dir: self.working_dir.clone(),
                model_path: model,
                audio_root: self.store.root().to_owned(),
            },
            EngineTimeouts::default(),
        ) {
            Ok((engine, events)) => {
                self.engine = Some(engine);
                self.events = Some(events);
            }
            Err(error) => self.set_error(error.code),
        }
        Ok(self.snapshot.clone())
    }
    async fn cancel(&mut self) {
        let previous = self.snapshot.phase;
        if previous == Phase::Success {
            self.invalidate();
            self.snapshot.phase = if self.snapshot.engine_ready {
                Phase::Ready
            } else {
                Phase::Unconfigured
            };
            self.snapshot.message = "Résultat annulé et effacé.".into();
            self.publish();
            return;
        }
        if previous == Phase::Cancelling {
            return;
        }
        if !matches!(
            previous,
            Phase::LoadingEngine
                | Phase::Recording
                | Phase::Transcribing
                | Phase::Inserting
                | Phase::Cancelling
        ) {
            return;
        }
        self.audio.discard();
        self.recording_since = None;
        self.clear_text();
        self.wav.take();
        let id = self.snapshot.session_id.take(); // Invalidate before any engine response can be accepted.
        self.cancelling_id = id.map(|id| format!("d-{id}"));
        self.snapshot.phase = Phase::Cancelling;
        self.snapshot.message = "Annulation en cours…".into();
        self.publish();
        if previous == Phase::LoadingEngine {
            self.stop_engine().await;
        } else if previous == Phase::Transcribing {
            if let (Some(engine), Some(id)) = (&self.engine, &self.cancelling_id) {
                let _ = engine.cancel(id).await;
                // Acknowledgement or forced shutdown will finish the cancellation.
                return;
            }
        }
        self.cancelling_id = None;
        self.snapshot.phase = if self.snapshot.engine_ready {
            Phase::Ready
        } else {
            Phase::Unconfigured
        };
        self.snapshot.error_code = None;
        self.snapshot.message = "Dictée annulée. Aucun texte conservé.".into();
        self.publish();
    }
    async fn hotkey(&mut self, event: HotkeyEvent) {
        match event {
            HotkeyEvent::Pressed => {
                if !self.snapshot.engine_ready || self.engine.is_none() {
                    return;
                }
                if !matches!(
                    self.snapshot.phase,
                    Phase::Ready | Phase::Success | Phase::Error
                ) {
                    self.snapshot.message =
                        "Traitement déjà en cours. Attendez ou annulez avant une nouvelle dictée."
                            .into();
                    self.publish();
                    return;
                }
                self.invalidate();
                self.session_epoch = self.epoch_guard.load(Ordering::Acquire);
                self.target = if self.options.auto_insert_enabled {
                    self.output.capture_target().ok()
                } else {
                    None
                };
                // Focus lookup invokes a bounded helper; cancellation may arrive while it runs.
                if self.stop_guard.load(Ordering::Acquire)
                    || self.cancel_guard.load(Ordering::Acquire)
                    || self.epoch_guard.load(Ordering::Acquire) != self.session_epoch
                {
                    self.invalidate();
                    self.snapshot.phase = Phase::Ready;
                    self.snapshot.message = "Dictée annulée avant l’enregistrement.".into();
                    self.publish();
                    return;
                }
                self.next_id += 1;
                if self.next_id > 9_007_199_254_740_991 {
                    self.set_error(ErrorCode::ProtocolError);
                    return;
                }
                match self.audio.start(&self.snapshot.microphone_id) {
                    Ok(()) => {
                        self.snapshot.session_id = Some(self.next_id);
                        self.snapshot.phase = Phase::Recording;
                        self.snapshot.recording_ms = 0;
                        self.snapshot.audio_duration_ms = None;
                        self.snapshot.error_code = None;
                        self.recording_since = Some(Instant::now());
                        self.snapshot.message =
                            "Enregistrement en cours. Relâchez le raccourci pour transcrire."
                                .into();
                        self.publish();
                    }
                    Err(_) => {
                        self.audio.discard();
                        self.set_error(ErrorCode::MicUnavailable);
                    }
                }
            }
            HotkeyEvent::Released => {
                if self.snapshot.phase != Phase::Recording {
                    return;
                }
                self.recording_since = None;
                self.snapshot.phase = Phase::Transcribing;
                self.snapshot.message = "Préparation locale de l’audio…".into();
                self.publish();
                let buffer = match self.audio.stop() {
                    Ok(buffer) => buffer,
                    Err(error) => {
                        self.invalidate();
                        self.audio.discard();
                        self.audio_error(error);
                        return;
                    }
                };
                let id = self
                    .snapshot
                    .session_id
                    .expect("recording session identity");
                self.snapshot.audio_duration_ms =
                    Some(buffer.samples.len() as u64 * 1000 / buffer.sample_rate as u64);
                let handle = match self.store.write_wav(id, &buffer) {
                    Ok(handle) => handle,
                    Err(_) => {
                        self.invalidate();
                        self.set_error(ErrorCode::ProtocolError);
                        return;
                    }
                };
                drop(buffer);
                let request = Request::Transcribe {
                    protocol_version: 1,
                    request_id: format!("d-{id}"),
                    audio_path: handle.path().to_string_lossy().into_owned(),
                    language: self.snapshot.language.clone(),
                    model_path: self
                        .model_path
                        .as_ref()
                        .expect("configured model")
                        .to_string_lossy()
                        .into_owned(),
                    vad: true,
                };
                self.wav = Some(handle);
                self.snapshot.phase = Phase::Transcribing;
                self.snapshot.message = "Transcription en cours…".into();
                self.publish();
                let outcome = self
                    .engine
                    .as_ref()
                    .expect("ready engine")
                    .transcribe(request)
                    .await;
                if let Err(error) = outcome {
                    self.invalidate();
                    self.snapshot.engine_ready = false;
                    self.stop_engine().await;
                    self.set_error(error.code);
                }
            }
        }
    }
    fn audio_error(&mut self, error: AudioError) {
        if error.code == "audio_limit" {
            self.snapshot.reason = Some("recording_limit");
            self.snapshot.phase = Phase::Error;
            self.snapshot.error_code = None;
            self.snapshot.message =
                "Limite de 60 secondes atteinte. Audio abandonné, aucune transcription lancée."
                    .into();
            self.publish();
        } else {
            self.set_error(ErrorCode::MicUnavailable);
        }
    }
    fn accepts(&self, request_id: &str) -> bool {
        self.snapshot.phase == Phase::Transcribing
            && self
                .snapshot
                .session_id
                .is_some_and(|id| request_id == format!("d-{id}"))
    }
    async fn engine_event(&mut self, event: Option<EngineEvent>) {
        match event {
            Some(EngineEvent::Ready) => {
                if self.snapshot.phase == Phase::LoadingEngine {
                    self.snapshot.engine_ready = true;
                    self.snapshot.phase = Phase::Ready;
                    self.snapshot.message =
                        "Prêt. Maintenez le raccourci pour parler, puis relâchez-le.".into();
                    self.publish();
                }
            }
            Some(EngineEvent::Result {
                request_id,
                mut text,
                duration_ms,
                ..
            }) => {
                if self.accepts(&request_id) {
                    self.wav.take();
                    self.snapshot.phase = Phase::Success;
                    self.snapshot.text = Some(text);
                    self.snapshot.audio_duration_ms = Some(duration_ms);
                    self.result_until = Some(Instant::now() + Duration::from_secs(120));
                    self.deliver();
                } else {
                    text.zeroize();
                }
            }
            Some(EngineEvent::Cancelled { request_id }) => {
                if self.snapshot.phase == Phase::Cancelling
                    && self.cancelling_id.as_deref() == Some(&request_id)
                {
                    self.cancelling_id = None;
                    self.snapshot.phase = Phase::Ready;
                    self.snapshot.message = "Dictée annulée. Aucun texte conservé.".into();
                    self.publish();
                }
            }
            Some(EngineEvent::Error { request_id, code }) => {
                if request_id.as_deref().is_some_and(|id| {
                    !self.accepts(id) && self.cancelling_id.as_deref() != Some(id)
                }) {
                    return;
                }
                self.invalidate();
                if code == ErrorCode::NoSpeech {
                    self.snapshot.phase = Phase::Ready;
                    self.snapshot.error_code = Some(code);
                    self.snapshot.message = failure(code).message;
                    self.publish();
                } else {
                    self.snapshot.engine_ready = false;
                    self.stop_engine().await;
                    self.set_error(code);
                }
            }
            Some(EngineEvent::Stopped) | None => {
                self.invalidate();
                self.snapshot.engine_ready = false;
                self.stop_engine().await;
                if self.snapshot.phase == Phase::Cancelling {
                    self.snapshot.phase = Phase::Unconfigured;
                    self.snapshot.message =
                        "Moteur arrêté après annulation. Préparez-le pour recommencer.".into();
                    self.publish();
                } else {
                    self.set_error(ErrorCode::EngineCrashed);
                }
            }
        }
    }
    async fn run(
        mut self,
        mut rx: mpsc::Receiver<Control>,
        stop: Arc<AtomicBool>,
        urgent_cancel: Arc<AtomicBool>,
        input_epoch: Arc<AtomicU64>,
    ) {
        self.stop_guard = stop.clone();
        self.cancel_guard = urgent_cancel.clone();
        self.epoch_guard = input_epoch.clone();
        let mut timer = tokio::time::interval(Duration::from_millis(20));
        loop {
            if stop.load(Ordering::Acquire) {
                break;
            }
            if urgent_cancel.swap(false, Ordering::AcqRel) {
                self.cancel().await;
            }
            tokio::select! {
                biased;
                _ = timer.tick() => {
                    if let Some(since) = self.recording_since {
                        if let Some(error) = self.audio.poll_error() { self.audio.discard(); self.invalidate(); self.audio_error(error); }
                        else { let ms = since.elapsed().as_millis() as u64;
                            if ms >= 60_000 { self.audio.discard(); self.invalidate(); self.audio_error(AudioError::new("audio_limit", "")); }
                            else if ms.saturating_sub(self.snapshot.recording_ms) >= 100 { self.snapshot.recording_ms = ms; self.publish(); }
                        }
                    }
                    if self.result_until.is_some_and(|deadline| Instant::now() >= deadline) { self.expire_result(); }
                }
                event = async { match &mut self.events { Some(events) => events.recv().await, None => std::future::pending().await } } => {
                    if urgent_cancel.swap(false, Ordering::AcqRel) { self.cancel().await; }
                    self.engine_event(event).await;
                },
                command = rx.recv() => match command {
                    Some(Control::Status(reply)) => { let _ = reply.send(Ok(self.snapshot.clone())); }
                    Some(Control::Configure(config, reply)) => { let result = self.configure(config).await; let _ = reply.send(result); }
                    Some(Control::Cancel(reply)) => { if urgent_cancel.swap(false, Ordering::AcqRel) { self.cancel().await; } let _ = reply.send(Ok(self.snapshot.clone())); }
                    Some(Control::Copy(reply)) => { let result = self.copy_result(); let _ = reply.send(result); }
                    Some(Control::ClearClipboard(reply)) => { let result = self.clear_clipboard(); let _ = reply.send(result); }
                    Some(Control::OutputDiagnostics(reply)) => { let _ = reply.send(self.output.diagnostics()); }
                    Some(Control::IdleAction(apply, reply)) => {
                        let result = self.apply_when_idle(apply);
                        let _ = reply.send(result);
                    }
                    Some(Control::Options(options, apply, reply)) => {
                        let result = self.update_options(options, apply);
                        let _ = reply.send(result);
                    }
                    Some(Control::Clear(reply)) => { if self.busy() { let _ = reply.send(Err(failure(ErrorCode::EngineBusy))); continue; } self.clear_text(); if self.snapshot.phase == Phase::Success { self.snapshot.phase = if self.snapshot.engine_ready { Phase::Ready } else { Phase::Unconfigured }; self.snapshot.session_id = None; self.snapshot.message = "Résultat effacé.".into(); } self.publish(); let _ = reply.send(Ok(self.snapshot.clone())); }
                    Some(Control::Microphones(reply)) => { let result = self.audio.list_devices().map_err(|_| failure(ErrorCode::MicUnavailable)); let _ = reply.send(result); }
                    Some(Control::Hotkey(event, epoch)) => {
                        if epoch == input_epoch.load(Ordering::Acquire) { self.hotkey(event).await; }
                    },
                    None => break,
                }
            }
        }
        self.audio.discard();
        self.invalidate();
        self.stop_engine().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct FakeOutput {
        inserts: Arc<std::sync::atomic::AtomicUsize>,
        copies: Arc<std::sync::atomic::AtomicUsize>,
        failure: Option<ErrorCode>,
        cancel: Option<Arc<AtomicBool>>,
    }
    impl DesktopOutput for FakeOutput {
        fn capture_target(&mut self) -> Result<FocusTarget, ErrorCode> {
            Ok(FocusTarget {
                active_window: 10,
                focused_window: 11,
            })
        }
        fn insert(
            &mut self,
            _: &FocusTarget,
            _: &str,
            cancelled: &dyn Fn() -> bool,
        ) -> Result<(), ErrorCode> {
            if let Some(cancel) = &self.cancel {
                cancel.store(true, Ordering::Release);
            }
            if cancelled() {
                return Err(ErrorCode::Cancelled);
            }
            self.inserts.fetch_add(1, Ordering::SeqCst);
            self.failure.map_or(Ok(()), Err)
        }
        fn copy(&mut self, _: &str) -> Result<(), ErrorCode> {
            self.copies.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn clear_clipboard(&mut self) -> Result<(), ErrorCode> {
            Ok(())
        }
        fn diagnostics(&self) -> OutputDiagnostics {
            OutputDiagnostics {
                x11: true,
                xdotool_available: true,
                xclip_available: true,
            }
        }
    }
    fn delivering_actor(
        failure: Option<ErrorCode>,
        consent: bool,
    ) -> (
        Actor,
        tempfile::TempDir,
        Arc<std::sync::atomic::AtomicUsize>,
        Arc<std::sync::atomic::AtomicUsize>,
    ) {
        let (mut actor, root) = actor();
        let inserts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let copies = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        actor.output = Box::new(FakeOutput {
            inserts: inserts.clone(),
            copies: copies.clone(),
            failure,
            cancel: None,
        });
        actor.options = OutputOptions {
            auto_insert_enabled: true,
            clipboard_auto_copy_consent: consent,
            clipboard_fallback: true,
        };
        actor.target = Some(FocusTarget {
            active_window: 10,
            focused_window: 11,
        });
        actor.snapshot.phase = Phase::Transcribing;
        actor.snapshot.engine_ready = true;
        actor.snapshot.session_id = Some(4);
        (actor, root, inserts, copies)
    }
    fn result_event() -> Option<EngineEvent> {
        Some(EngineEvent::Result {
            request_id: "d-4".into(),
            text: "Bonjour".into(),
            duration_ms: 500,
            language: Language::Fr,
        })
    }
    #[tokio::test]
    async fn matching_result_inserts_once_and_duplicate_is_ignored() {
        let (mut actor, _root, inserts, copies) = delivering_actor(None, false);
        actor.engine_event(result_event()).await;
        assert_eq!(actor.snapshot.delivery, Delivery::Inserted);
        assert_eq!(inserts.load(Ordering::SeqCst), 1);
        actor.engine_event(result_event()).await;
        assert_eq!(inserts.load(Ordering::SeqCst), 1);
        assert_eq!(copies.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn failed_insertion_requires_consent_before_clipboard_changes() {
        for consent in [false, true] {
            let (mut actor, _root, _, copies) =
                delivering_actor(Some(ErrorCode::FocusChanged), consent);
            actor.engine_event(result_event()).await;
            assert_eq!(
                actor.snapshot.delivery,
                if consent {
                    Delivery::Clipboard
                } else {
                    Delivery::Fallback
                }
            );
            assert_eq!(copies.load(Ordering::SeqCst), usize::from(consent));
            assert!(actor.snapshot.insertion_uncertain);
            actor.copy_result().unwrap();
            assert!(actor.snapshot.insertion_uncertain);
            assert!(actor.snapshot.message.contains("une partie"));
            assert_eq!(actor.snapshot.text.as_deref(), Some("Bonjour"));
        }
    }
    #[tokio::test]
    async fn urgent_cancel_before_or_during_delivery_never_copies_or_keeps_result() {
        for during in [false, true] {
            let (mut actor, _root, inserts, copies) = delivering_actor(None, true);
            if during {
                actor.output = Box::new(FakeOutput {
                    inserts: inserts.clone(),
                    copies: copies.clone(),
                    failure: None,
                    cancel: Some(actor.cancel_guard.clone()),
                });
            } else {
                actor.cancel_guard.store(true, Ordering::Release);
            }
            actor.engine_event(result_event()).await;
            assert!(actor.snapshot.text.is_none());
            assert_eq!(inserts.load(Ordering::SeqCst), 0);
            assert_eq!(copies.load(Ordering::SeqCst), 0);
        }
    }
    #[tokio::test]
    async fn explicit_copy_uses_only_live_result_and_never_accepts_expired_text() {
        let (mut actor, _root, _, copies) = delivering_actor(None, false);
        assert_eq!(actor.copy_result().unwrap_err().code, ErrorCode::EngineBusy);
        actor.options.auto_insert_enabled = false;
        actor.engine_event(result_event()).await;
        assert_eq!(actor.snapshot.delivery, Delivery::Preview);
        actor.copy_result().unwrap();
        assert_eq!(copies.load(Ordering::SeqCst), 1);
        actor.result_until = Some(Instant::now() - Duration::from_secs(1));
        let revision = actor.snapshot.revision;
        assert!(actor.copy_result().is_err());
        assert!(actor.snapshot.revision > revision);
        assert_eq!(actor.snapshot.phase, Phase::Ready);
        assert_eq!(copies.load(Ordering::SeqCst), 1);
        assert!(actor.snapshot.text.is_none());
    }
    #[test]
    fn settings_application_is_gated_and_failure_preserves_previous_options() {
        let (mut actor, _root) = actor();
        let changed = OutputOptions {
            auto_insert_enabled: true,
            ..OutputOptions::default()
        };
        for phase in [
            Phase::Recording,
            Phase::Transcribing,
            Phase::Inserting,
            Phase::LoadingEngine,
            Phase::Cancelling,
        ] {
            actor.snapshot.phase = phase;
            assert_eq!(
                actor
                    .apply_when_idle(|| panic!("must not replace shortcut during session"))
                    .unwrap_err()
                    .code,
                ErrorCode::EngineBusy
            );
            assert_eq!(
                actor
                    .update_options(changed, || panic!("must not apply settings during session"))
                    .unwrap_err()
                    .code,
                ErrorCode::EngineBusy
            );
        }
        actor.snapshot.phase = Phase::Ready;
        assert!(actor
            .update_options(changed, || Err(failure(ErrorCode::ProtocolError)))
            .is_err());
        assert!(!actor.options.auto_insert_enabled);
        actor.update_options(changed, || Ok(())).unwrap();
        assert!(actor.snapshot.auto_insert_enabled);
    }
    #[tokio::test]
    async fn idle_action_rechecks_state_after_queueing_before_changing_shortcut() {
        let (mut actor, _root) = actor();
        actor.snapshot.phase = Phase::Ready;
        actor.publish();
        let (tx, mut rx) = mpsc::channel(1);
        let service = DictationService {
            tx,
            stopping: Arc::default(),
            urgent_cancel: Arc::default(),
            input_epoch: Arc::default(),
            worker: Mutex::new(None),
            busy_guard: actor.busy_guard.clone(),
        };
        let applied = Arc::new(AtomicBool::new(false));
        let callback_applied = applied.clone();
        let pending = tokio::spawn(async move {
            service
                .apply_when_idle(move || {
                    callback_applied.store(true, Ordering::SeqCst);
                    Ok(())
                })
                .await
        });
        // The caller observed Ready and queued its change. A dictation starts
        // before the actor executes that change: the second check must reject it.
        let command = rx.recv().await.unwrap();
        actor.snapshot.phase = Phase::Recording;
        actor.publish();
        match command {
            Control::IdleAction(apply, reply) => {
                let _ = reply.send(actor.apply_when_idle(apply));
            }
            _ => panic!("expected serialized idle action"),
        }
        assert_eq!(
            pending.await.unwrap().unwrap_err().code,
            ErrorCode::EngineBusy
        );
        assert!(!applied.load(Ordering::SeqCst));
        actor.snapshot.phase = Phase::Ready;
        actor
            .apply_when_idle(|| {
                applied.store(true, Ordering::SeqCst);
                Ok(())
            })
            .unwrap();
        assert!(applied.load(Ordering::SeqCst));
        assert!(!actor.options.auto_insert_enabled);
    }
    #[tokio::test]
    async fn commands_received_while_inserting_are_rejected_without_queueing() {
        let (mut actor, _root) = actor();
        actor.snapshot.phase = Phase::Inserting;
        actor.publish();
        let (tx, mut rx) = mpsc::channel(1);
        let service = DictationService {
            tx,
            stopping: Arc::default(),
            urgent_cancel: Arc::default(),
            input_epoch: Arc::default(),
            worker: Mutex::new(None),
            busy_guard: actor.busy_guard.clone(),
        };
        assert_eq!(
            service.copy_result().await.unwrap_err().code,
            ErrorCode::EngineBusy
        );
        assert_eq!(
            service.clear().await.unwrap_err().code,
            ErrorCode::EngineBusy
        );
        assert_eq!(
            service
                .apply_when_idle(|| panic!("busy shortcut change must not be queued"))
                .await
                .unwrap_err()
                .code,
            ErrorCode::EngineBusy
        );
        assert_eq!(
            service
                .apply_options(OutputOptions::default(), || panic!(
                    "queued settings must never be applied"
                ))
                .await
                .unwrap_err()
                .code,
            ErrorCode::EngineBusy
        );
        assert!(matches!(
            rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
        service.hotkey(HotkeyEvent::Pressed);
        assert!(matches!(
            rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
        actor.snapshot.phase = Phase::Success;
        actor.publish();
        assert!(service.ensure_idle().is_ok());
    }
    struct FakeAudio;
    impl AudioCapture for FakeAudio {
        fn list_devices(&self) -> Result<Vec<Microphone>, AudioError> {
            Ok(vec![])
        }
        fn start(&mut self, _: &str) -> Result<(), AudioError> {
            panic!("no microphone without ready engine")
        }
        fn stop(&mut self) -> Result<AudioBuffer, AudioError> {
            unreachable!()
        }
        fn discard(&mut self) {}
        fn poll_error(&self) -> Option<AudioError> {
            None
        }
    }
    fn actor() -> (Actor, tempfile::TempDir) {
        let root = tempfile::tempdir().unwrap();
        let store = TempAudioStore::new(&root.path().join("audio")).unwrap();
        (
            Actor::new(
                root.path().into(),
                PathBuf::from("/usr/bin/python3"),
                store,
                Box::new(FakeAudio),
                Arc::new(|_| {}),
            ),
            root,
        )
    }
    #[tokio::test]
    async fn unconfigured_press_never_starts_microphone() {
        let (mut actor, _root) = actor();
        actor.hotkey(HotkeyEvent::Pressed).await;
        assert_eq!(actor.snapshot.phase, Phase::Unconfigured);
    }
    #[tokio::test]
    async fn stale_result_after_cancel_never_becomes_preview() {
        let (mut actor, _root) = actor();
        actor.snapshot.phase = Phase::Transcribing;
        actor.snapshot.session_id = Some(7);
        actor.cancel().await;
        actor
            .engine_event(Some(EngineEvent::Result {
                request_id: "d-7".into(),
                text: "sensitive".into(),
                language: Language::Fr,
                duration_ms: 10,
            }))
            .await;
        assert!(actor.snapshot.text.is_none());
        assert!(actor.snapshot.session_id.is_none());
    }
    #[tokio::test]
    async fn result_requires_matching_transcribing_session() {
        let (mut actor, _root) = actor();
        actor.snapshot.phase = Phase::Transcribing;
        actor.snapshot.session_id = Some(4);
        actor
            .engine_event(Some(EngineEvent::Result {
                request_id: "d-3".into(),
                text: "old".into(),
                language: Language::En,
                duration_ms: 5,
            }))
            .await;
        assert!(actor.snapshot.text.is_none());
        actor
            .engine_event(Some(EngineEvent::Result {
                request_id: "d-4".into(),
                text: "Hello".into(),
                language: Language::En,
                duration_ms: 5,
            }))
            .await;
        assert_eq!(actor.snapshot.text.as_deref(), Some("Hello"));
        assert_eq!(actor.snapshot.phase, Phase::Success);
        actor
            .engine_event(Some(EngineEvent::Result {
                request_id: "d-4".into(),
                text: "duplicate".into(),
                language: Language::En,
                duration_ms: 5,
            }))
            .await;
        assert_eq!(actor.snapshot.text.as_deref(), Some("Hello"));
    }
    #[tokio::test]
    async fn cancellation_after_completion_removes_result() {
        let (mut actor, _root) = actor();
        actor.snapshot.phase = Phase::Success;
        actor.snapshot.text = Some("private result".into());
        actor.cancel().await;
        assert!(actor.snapshot.text.is_none());
        assert_ne!(actor.snapshot.phase, Phase::Success);
    }
    #[tokio::test]
    async fn unrelated_cancellation_ack_cannot_finish_active_cancel() {
        let (mut actor, _root) = actor();
        actor.snapshot.phase = Phase::Cancelling;
        actor
            .engine_event(Some(EngineEvent::Cancelled {
                request_id: "old-id".into(),
            }))
            .await;
        assert_eq!(actor.snapshot.phase, Phase::Cancelling);
    }
    struct CountAudio {
        starts: Arc<std::sync::atomic::AtomicUsize>,
        discards: Arc<std::sync::atomic::AtomicUsize>,
    }
    impl AudioCapture for CountAudio {
        fn list_devices(&self) -> Result<Vec<Microphone>, AudioError> {
            Ok(vec![])
        }
        fn start(&mut self, _: &str) -> Result<(), AudioError> {
            self.starts.fetch_add(1, Ordering::SeqCst);
            Err(AudioError::new("audio_test", "synthetic source"))
        }
        fn stop(&mut self) -> Result<AudioBuffer, AudioError> {
            unreachable!()
        }
        fn discard(&mut self) {
            self.discards.fetch_add(1, Ordering::SeqCst);
        }
        fn poll_error(&self) -> Option<AudioError> {
            None
        }
    }
    #[tokio::test]
    async fn fatal_engine_failure_stops_microphone_immediately() {
        let (mut actor, _root) = actor();
        let discards = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        actor.audio = Box::new(CountAudio {
            starts: Arc::default(),
            discards: discards.clone(),
        });
        actor.snapshot.phase = Phase::Recording;
        actor.recording_since = Some(Instant::now());
        actor
            .engine_event(Some(EngineEvent::Error {
                request_id: None,
                code: ErrorCode::EngineCrashed,
            }))
            .await;
        assert!(discards.load(Ordering::SeqCst) > 0);
        assert!(actor.recording_since.is_none());
    }
    #[tokio::test]
    async fn saturated_release_invalidates_queued_press_without_starting_audio() {
        let (mut actor, root) = actor();
        std::fs::create_dir(root.path().join("engine")).unwrap();
        std::fs::create_dir(root.path().join("model")).unwrap();
        std::fs::write(root.path().join("engine/__init__.py"), "").unwrap();
        std::fs::write(
            root.path().join("engine/main.py"),
            include_str!("../tests/fixtures/fake_engine.py"),
        )
        .unwrap();
        std::fs::write(root.path().join("mode"), "result").unwrap();
        let (client, events) = EngineClient::start(
            EngineConfig {
                provider: TranscriptionProvider::Local,
                python_path: "/usr/bin/python3".into(),
                working_dir: root.path().into(),
                model_path: root.path().join("model"),
                audio_root: actor.store.root().into(),
            },
            EngineTimeouts::default(),
        )
        .await
        .unwrap();
        actor.engine = Some(client);
        actor.events = Some(events);
        actor.snapshot.phase = Phase::Ready;
        actor.snapshot.engine_ready = true;
        let starts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        actor.audio = Box::new(CountAudio {
            starts: starts.clone(),
            discards: Arc::default(),
        });
        let (tx, rx) = mpsc::channel(1);
        let stopping = Arc::new(AtomicBool::new(false));
        let urgent_cancel = Arc::new(AtomicBool::new(false));
        let service = DictationService {
            tx,
            stopping: stopping.clone(),
            urgent_cancel: urgent_cancel.clone(),
            input_epoch: Arc::new(AtomicU64::new(0)),
            worker: Mutex::new(None),
            busy_guard: Arc::new(AtomicBool::new(false)),
        };
        service.hotkey(HotkeyEvent::Pressed);
        service.hotkey(HotkeyEvent::Released); // Queue full: fail closed, not a delayed recording.
        let task = tokio::spawn(actor.run(
            rx,
            stopping.clone(),
            urgent_cancel,
            service.input_epoch.clone(),
        ));
        let _ = service.status().await.unwrap();
        stopping.store(true, Ordering::Release);
        task.await.unwrap();
        assert_eq!(starts.load(Ordering::SeqCst), 0);
    }
    struct WorkingAudio(Arc<AtomicBool>);
    impl AudioCapture for WorkingAudio {
        fn list_devices(&self) -> Result<Vec<Microphone>, AudioError> {
            Ok(vec![])
        }
        fn start(&mut self, _: &str) -> Result<(), AudioError> {
            assert!(!self.0.swap(true, Ordering::SeqCst));
            Ok(())
        }
        fn stop(&mut self) -> Result<AudioBuffer, AudioError> {
            assert!(self.0.swap(false, Ordering::SeqCst));
            Ok(AudioBuffer {
                samples: vec![0; 320],
                sample_rate: 16000,
            })
        }
        fn discard(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
        fn poll_error(&self) -> Option<AudioError> {
            None
        }
    }
    async fn until(service: &DictationService, phase: Phase) -> DictationSnapshot {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let status = service.status().await.unwrap();
                if status.phase == phase {
                    return status;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("controller reached expected phase")
    }
    #[tokio::test]
    async fn installed_selection_identity_survives_prepare_and_switch() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("engine")).unwrap();
        std::fs::write(root.path().join("engine/__init__.py"), "").unwrap();
        std::fs::write(
            root.path().join("engine/main.py"),
            include_str!("../tests/fixtures/fake_engine.py"),
        )
        .unwrap();
        std::fs::write(root.path().join("mode"), "result").unwrap();
        let service = DictationService::spawn(
            root.path().into(),
            "/usr/bin/python3".into(),
            root.path().join("audio"),
            Box::new(FakeAudio),
            Arc::new(|_| {}),
        )
        .unwrap();
        assert_eq!(service.status().await.unwrap().model_id, None);
        for id in ["tiny", "base"] {
            let model = root.path().join("models").join(id);
            std::fs::create_dir_all(&model).unwrap();
            for asset in [
                "model.bin",
                "config.json",
                "tokenizer.json",
                "vocabulary.txt",
            ] {
                std::fs::write(model.join(asset), "fake").unwrap();
            }
            let loading = service
                .configure(RuntimeConfig {
                    provider: TranscriptionProvider::Local,
                    model_path: model.to_string_lossy().into_owned(),
                    microphone_id: "default".into(),
                    language: Language::Fr,
                })
                .await
                .unwrap();
            assert_eq!(loading.model_id.as_deref(), Some(id));
            assert!(!loading.engine_ready);
            let prepared = until(&service, Phase::Ready).await;
            assert_eq!(prepared.model_id.as_deref(), Some(id));
            assert!(prepared.engine_ready);
        }
        service.shutdown();
    }
    #[test]
    fn provider_configuration_defaults_local_and_rejects_unknown_values() {
        let legacy =
            serde_json::json!({"model_path":"/model","microphone_id":"default","language":"fr"});
        assert!(serde_json::from_value::<RuntimeConfig>(legacy.clone()).is_ok());
        assert_eq!(
            serde_json::to_value(DictationSnapshot::default()).unwrap()["provider"],
            "local"
        );
        let mut groq = legacy.clone();
        groq["provider"] = "groq".into();
        assert!(serde_json::from_value::<RuntimeConfig>(groq).is_ok());
        let mut unknown = legacy;
        unknown["provider"] = "unknown".into();
        assert!(serde_json::from_value::<RuntimeConfig>(unknown).is_err());
    }
    #[tokio::test]
    async fn groq_uses_project_identity_without_local_weights_then_switches_local() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("engine")).unwrap();
        std::fs::write(root.path().join("engine/__init__.py"), "").unwrap();
        std::fs::write(
            root.path().join("engine/main.py"),
            include_str!("../tests/fixtures/fake_engine.py"),
        )
        .unwrap();
        std::fs::write(root.path().join("mode"), "provider_check").unwrap();
        let service = DictationService::spawn(
            root.path().into(),
            "/usr/bin/python3".into(),
            root.path().join("audio"),
            Box::new(FakeAudio),
            Arc::new(|_| {}),
        )
        .unwrap();
        let config: RuntimeConfig = serde_json::from_value(serde_json::json!({"provider":"groq","model_path":"/nonexistent/local/weights","microphone_id":"default","language":"fr"})).unwrap();
        let loading = service.configure(config).await.unwrap();
        assert_eq!(serde_json::to_value(&loading).unwrap()["provider"], "groq");
        assert!(loading.model_id.is_none());
        let ready = until(&service, Phase::Ready).await;
        assert_eq!(serde_json::to_value(&ready).unwrap()["provider"], "groq");
        assert!(ready.engine_ready);
        let invalid_local: RuntimeConfig = serde_json::from_value(serde_json::json!({"provider":"local","model_path":"/nonexistent/local/weights","microphone_id":"default","language":"fr"})).unwrap();
        assert_eq!(
            service.configure(invalid_local).await.unwrap_err().code,
            ErrorCode::ModelNotFound
        );
        assert_eq!(
            serde_json::to_value(service.status().await.unwrap()).unwrap()["provider"],
            "groq"
        );
        std::fs::create_dir(root.path().join("model")).unwrap();
        std::fs::write(root.path().join("mode"), "result").unwrap();
        let local: RuntimeConfig = serde_json::from_value(serde_json::json!({"provider":"local","model_path":root.path().join("model"),"microphone_id":"default","language":"fr"})).unwrap();
        service.configure(local).await.unwrap();
        let ready = until(&service, Phase::Ready).await;
        assert_eq!(serde_json::to_value(ready).unwrap()["provider"], "local");
        service.shutdown();
    }
    #[tokio::test]
    async fn synthetic_capture_python_preview_cleanup_and_shutdown_pipeline() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("engine")).unwrap();
        std::fs::create_dir(root.path().join("model")).unwrap();
        std::fs::write(root.path().join("engine/__init__.py"), "").unwrap();
        std::fs::write(
            root.path().join("engine/main.py"),
            include_str!("../tests/fixtures/fake_engine.py"),
        )
        .unwrap();
        std::fs::write(root.path().join("mode"), "result").unwrap();
        let recording = Arc::new(AtomicBool::new(false));
        let service = DictationService::spawn(
            root.path().into(),
            "/usr/bin/python3".into(),
            root.path().join("audio"),
            Box::new(WorkingAudio(recording.clone())),
            Arc::new(|_| {}),
        )
        .unwrap();
        service
            .configure(RuntimeConfig {
                provider: TranscriptionProvider::Local,
                model_path: root.path().join("model").to_string_lossy().into_owned(),
                microphone_id: "default".into(),
                language: Language::Fr,
            })
            .await
            .unwrap();
        until(&service, Phase::Ready).await;
        assert!(!recording.load(Ordering::SeqCst));
        service.hotkey(HotkeyEvent::Pressed);
        until(&service, Phase::Recording).await;
        assert!(recording.load(Ordering::SeqCst));
        service.hotkey(HotkeyEvent::Released);
        let result = until(&service, Phase::Success).await;
        assert_eq!(result.text.as_deref(), Some("bonjour"));
        assert!(!recording.load(Ordering::SeqCst));
        service.clear().await.unwrap();
        assert!(service.status().await.unwrap().text.is_none());
        let pid = std::fs::read_to_string(root.path().join("pid")).unwrap();
        service.shutdown();
        assert!(!PathBuf::from(format!("/proc/{pid}")).exists());
        assert_eq!(
            std::fs::read_dir(root.path().join("audio"))
                .unwrap()
                .count(),
            0
        );
    }
    #[test]
    fn recording_limit_has_safe_structured_ui_reason() {
        let (mut actor, _root) = actor();
        actor.audio_error(AudioError::new("audio_limit", "untrusted audio diagnostic"));
        assert_eq!(actor.snapshot.reason, Some("recording_limit"));
        assert_eq!(actor.snapshot.phase, Phase::Error);
        assert!(!actor.snapshot.message.contains("untrusted"));
    }
}
