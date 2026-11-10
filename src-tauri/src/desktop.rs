//! Tauri adapters for local push-to-talk capture and transcription.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex, TryLockError,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::dictation::{
    DictationService, DictationSnapshot, OutputOptions, Phase, RuntimeConfig, RuntimeError,
};
use crate::hotkey::{HotkeyError, HotkeyEvent, HotkeyManager, ShortcutBackend};
use crate::hotkey_relay::{spawn_relay_worker, ShortcutMessage, ShortcutRelay};
use crate::protocol::ErrorCode;
use crate::settings::{Settings, SettingsState, SettingsStore};

struct TauriShortcutBackend(AppHandle);

impl ShortcutBackend for TauriShortcutBackend {
    fn register(&mut self, shortcut: &str) -> Result<(), HotkeyError> {
        self.0
            .global_shortcut()
            .register(shortcut)
            .map_err(|_| HotkeyError::Unavailable)
    }

    fn unregister(&mut self, shortcut: &str) -> Result<(), HotkeyError> {
        self.0
            .global_shortcut()
            .unregister(shortcut)
            .map_err(|_| HotkeyError::Unavailable)
    }
}

struct HotkeyService {
    manager: Mutex<HotkeyManager<TauriShortcutBackend>>,
    supported: bool,
    cancellation: Arc<Mutex<CancellationShortcuts>>,
    error: Mutex<Option<String>>,
}

#[derive(Clone, Serialize)]
struct HotkeyStatus {
    revision: u64,
    shortcut: Option<String>,
    held: bool,
    supported: bool,
    error: Option<String>,
    press_count: u32,
    release_count: u32,
}

impl HotkeyService {
    fn snapshot(&self) -> Result<HotkeyStatus, String> {
        let manager = self
            .manager
            .lock()
            .map_err(|_| "Gestionnaire du raccourci indisponible.")?;
        self.snapshot_locked(&manager)
    }

    fn snapshot_locked(
        &self,
        manager: &HotkeyManager<TauriShortcutBackend>,
    ) -> Result<HotkeyStatus, String> {
        let error = self
            .error
            .lock()
            .map_err(|_| "Diagnostic du raccourci indisponible.")?;
        Ok(HotkeyStatus {
            revision: manager.revision,
            shortcut: manager.current().map(str::to_owned),
            held: manager.held(),
            supported: self.supported,
            error: error.clone(),
            press_count: manager.press_count,
            release_count: manager.release_count,
        })
    }

    fn handle_relayed(
        &self,
        message: &ShortcutMessage,
        stopping: &AtomicBool,
    ) -> Option<HotkeyStatus> {
        loop {
            if stopping.load(Ordering::Acquire) {
                return None;
            }
            match self.manager.try_lock() {
                Ok(mut manager) => {
                    manager.handle(&message.0, message.1)?;
                    return self.snapshot_locked(&manager).ok();
                }
                Err(TryLockError::WouldBlock) => {
                    // Keep this event until registration finishes. Checking stop
                    // here also lets shutdown join us if UI dispatch has ended.
                    thread::sleep(Duration::from_millis(2));
                }
                Err(TryLockError::Poisoned(_)) => return None,
            }
        }
    }
}

// Plugin registration dispatches to the UI thread and waits. Commands must not
// occupy that thread while a worker is waiting for registration to complete.
#[tauri::command]
async fn get_hotkey_status(app: AppHandle) -> Result<HotkeyStatus, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<HotkeyService>().snapshot())
        .await
        .map_err(|_| "Diagnostic du raccourci indisponible.".to_owned())?
}

fn parse_shortcut(shortcut: &str) -> Result<Shortcut, String> {
    if shortcut.len() > 128 {
        return Err(HotkeyError::Invalid.to_string());
    }
    let parsed: Shortcut = shortcut
        .parse()
        .map_err(|_| HotkeyError::Invalid.to_string())?;
    // Bare keys would consume normal typing; require an explicit modifier.
    if parsed.key == Code::Escape {
        return Err(
            "Échap est réservé à l’annulation. Choisissez une autre touche pour la dictée.".into(),
        );
    }
    if parsed.mods.is_empty() {
        return Err(HotkeyError::Invalid.to_string());
    }
    Ok(parsed)
}
#[tauri::command]
async fn replace_hotkey(app: AppHandle, shortcut: String) -> Result<HotkeyStatus, String> {
    let runtime = runtime(&app).map_err(|error| error.message)?;
    let parsed = parse_shortcut(&shortcut)?;
    let modifiers = parsed.mods;
    let shortcut = parsed.into_string();
    let owner = app.clone();
    runtime
        .apply_when_idle(move || {
            let service = owner.state::<HotkeyService>();
            if !service.supported {
                return Err(RuntimeError {
                    code: ErrorCode::ProtocolError,
                    message: "Ce prototype nécessite une session Cinnamon/X11.".into(),
                });
            }
            let outcome = service
                .manager
                .lock()
                .map_err(|_| runtime_error(ErrorCode::ProtocolError))?
                .replace(&shortcut);
            *service
                .error
                .lock()
                .map_err(|_| runtime_error(ErrorCode::ProtocolError))? =
                outcome.as_ref().err().map(ToString::to_string);
            outcome.map_err(|error| RuntimeError {
                code: ErrorCode::ProtocolError,
                message: error.to_string(),
            })?;
            schedule_cancellation(&owner, None, Some(modifiers));
            if let Ok(status) = service.snapshot() {
                let _ = owner.emit("hotkey://event", status);
            }
            Ok(())
        })
        .await
        .map_err(|error| error.message)?;
    get_hotkey_status(app).await
}

pub fn run() {
    let (sender, receiver) = mpsc::channel();
    let relay = ShortcutRelay(sender);
    let stopping = Arc::new(AtomicBool::new(false));
    let worker = Arc::new(Mutex::new(None::<JoinHandle<()>>));
    let setup_stopping = Arc::clone(&stopping);
    let setup_worker = Arc::clone(&worker);
    let runtime_cleanup = Arc::new(Mutex::new(None::<Arc<DictationService>>));
    let setup_runtime = Arc::clone(&runtime_cleanup);

    let outcome = tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |_app, shortcut, event| {
                    let event = match event.state {
                        ShortcutState::Pressed => HotkeyEvent::Pressed,
                        ShortcutState::Released => HotkeyEvent::Released,
                    };
                    relay.send(shortcut.into_string(), event);
                })
                .build(),
        )
        .setup(move |app| {
            let supported = std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("x11")
                && std::env::var_os("DISPLAY").is_some();
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let mut settings_store = SettingsStore::load(settings_path);
            let restored = settings_store.state.settings.clone();
            let restored_shortcut = match parse_shortcut(&restored.shortcut) {
                Ok(shortcut) => shortcut,
                Err(error) => {
                    settings_store.state.error = Some(error);
                    "Ctrl+Alt+Space".parse()?
                }
            };
            let restored_modifiers = restored_shortcut.mods;
            app.manage(SettingsService(Arc::new(Mutex::new(settings_store))));
            let mut manager = HotkeyManager::new(TauriShortcutBackend(app.handle().clone()));
            let initial_error = if supported {
                let shortcut = restored_shortcut;
                manager
                    .replace(&shortcut.into_string())
                    .err()
                    .map(|error| error.to_string())
            } else {
                Some("Ce prototype nécessite une session Cinnamon/X11.".to_owned())
            };
            app.manage(HotkeyService {
                manager: Mutex::new(manager),
                supported,
                cancellation: Arc::new(Mutex::new(CancellationShortcuts {
                    modifiers: restored_modifiers,
                    ..CancellationShortcuts::default()
                })),
                error: Mutex::new(initial_error),
            });
            let runtime_handle = app.handle().clone();
            let runtime_result = initialize_runtime(&runtime_handle);
            if let Ok(runtime) = &runtime_result {
                *setup_runtime.lock().expect("runtime cleanup lock poisoned") =
                    Some(runtime.clone());
            }
            if let Ok(runtime) = &runtime_result {
                if supported
                    && (restored.provider == crate::protocol::TranscriptionProvider::Groq
                        || !restored.model_path.is_empty())
                {
                    let runtime = runtime.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = runtime
                            .configure(RuntimeConfig {
                                provider: restored.provider,
                                model_path: restored.model_path,
                                microphone_id: restored.microphone_id,
                                language: restored.language,
                            })
                            .await;
                    });
                }
            }
            app.manage(RuntimeService(runtime_result));
            let app_handle = app.handle().clone();
            let handle = spawn_relay_worker(receiver, setup_stopping, move |message, stopping| {
                let service = app_handle.state::<HotkeyService>();
                let cancellation = service
                    .cancellation
                    .lock()
                    .ok()
                    .is_some_and(|state| state.active && state.registered.contains(&message.0));
                if cancellation {
                    if message.1 == HotkeyEvent::Pressed {
                        if let Ok(runtime) = &app_handle.state::<RuntimeService>().0 {
                            runtime.request_cancel();
                        }
                    }
                    return;
                }
                if let Some(status) = service.handle_relayed(&message, stopping) {
                    let _ = app_handle.emit("hotkey://event", status);
                    if let Ok(runtime) = &app_handle.state::<RuntimeService>().0 {
                        runtime.hotkey(message.1);
                    }
                }
            });
            *setup_worker
                .lock()
                .expect("shortcut relay worker lock poisoned") = Some(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::commands::get_app_info,
            get_hotkey_status,
            replace_hotkey,
            get_dictation_status,
            list_microphones,
            list_local_models,
            configure_dictation,
            cancel_dictation,
            clear_dictation_result,
            copy_dictation_result,
            clear_dictation_clipboard,
            get_settings,
            save_settings,
            get_diagnostics
        ])
        .build(tauri::generate_context!())
        .map(|app| {
            let exit_stopping = Arc::clone(&stopping);
            let exit_runtime = Arc::clone(&runtime_cleanup);
            app.run_return(move |_app_handle, event| {
                if matches!(event, tauri::RunEvent::Exit) {
                    exit_stopping.store(true, Ordering::Release);
                    if let Ok(runtime) = exit_runtime.lock() {
                        if let Some(runtime) = runtime.as_ref() {
                            runtime.request_cancel();
                        }
                    }
                }
            })
        });

    // Stop before reporting a startup error as well as after ordinary shutdown.
    // Worker lock acquisition retries check this flag, so joining cannot depend
    // on the now-stopped UI event loop.
    stopping.store(true, Ordering::Release);
    if let Some(handle) = worker
        .lock()
        .expect("shortcut relay worker lock poisoned")
        .take()
    {
        let _ = handle.join();
    }
    if let Some(runtime) = runtime_cleanup
        .lock()
        .expect("runtime cleanup lock poisoned")
        .take()
    {
        runtime.shutdown();
    }
    let exit_code = outcome.expect("Linux Voice Flow could not start");
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

struct RuntimeService(Result<Arc<DictationService>, RuntimeError>);
fn runtime_error(code: ErrorCode) -> RuntimeError {
    RuntimeError { code, message: "Moteur local indisponible. Vérifiez Python, le microphone et les répertoires de l’application.".into() }
}
fn project_root() -> Result<std::path::PathBuf, RuntimeError> {
    let workdir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.canonicalize().ok())
        .ok_or_else(|| runtime_error(ErrorCode::EngineCrashed))?;
    Ok(workdir)
}
#[tauri::command]
async fn list_local_models() -> Result<Vec<crate::models::LocalModelInfo>, RuntimeError> {
    crate::models::installed_models(&project_root()?).map_err(|_| RuntimeError {
        code: ErrorCode::ModelNotFound,
        message: "Catalogue des modèles locaux indisponible. Vérifiez l’installation locale."
            .into(),
    })
}
fn initialize_runtime(app: &AppHandle) -> Result<Arc<DictationService>, RuntimeError> {
    let workdir = project_root()?;
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|_| runtime_error(ErrorCode::ProtocolError))?;
    std::fs::create_dir_all(&cache).map_err(|_| runtime_error(ErrorCode::ProtocolError))?;
    let cache = cache
        .canonicalize()
        .map_err(|_| runtime_error(ErrorCode::ProtocolError))?;
    let audio = crate::audio::NativeAudioRecorder::new()
        .map_err(|_| runtime_error(ErrorCode::MicUnavailable))?;
    let handle = app.clone();
    let initial = app
        .state::<SettingsService>()
        .0
        .lock()
        .map_err(|_| runtime_error(ErrorCode::ProtocolError))?
        .state
        .settings
        .clone();
    let runtime = DictationService::spawn_with_output(
        workdir.clone(),
        workdir.join(".venv/bin/python"),
        cache.join("audio"),
        Box::new(audio),
        Arc::new(move |snapshot| {
            let active = cancellation_active(snapshot.phase);
            schedule_cancellation(&handle, Some(active), None);
            let _ = handle.emit("dictation://state", snapshot);
        }),
        Box::new(crate::insertion::X11Output::new(workdir)),
        output_options(&initial),
    )?;
    Ok(Arc::new(runtime))
}
fn runtime(app: &AppHandle) -> Result<Arc<DictationService>, RuntimeError> {
    app.state::<RuntimeService>().0.clone()
}
#[tauri::command]
async fn get_dictation_status(app: AppHandle) -> Result<DictationSnapshot, RuntimeError> {
    runtime(&app)?.status().await
}
#[tauri::command]
async fn list_microphones(app: AppHandle) -> Result<Vec<crate::audio::Microphone>, RuntimeError> {
    runtime(&app)?.microphones().await
}
#[tauri::command]
async fn configure_dictation(
    app: AppHandle,
    config: RuntimeConfig,
) -> Result<DictationSnapshot, RuntimeError> {
    if !app.state::<HotkeyService>().supported {
        return Err(RuntimeError {
            code: ErrorCode::EngineCrashed,
            message: "La dictée globale nécessite une session Cinnamon/X11.".into(),
        });
    }
    runtime(&app)?.configure(config).await
}
#[tauri::command]
async fn cancel_dictation(app: AppHandle) -> Result<DictationSnapshot, RuntimeError> {
    runtime(&app)?.cancel().await
}
#[tauri::command]
async fn clear_dictation_result(app: AppHandle) -> Result<DictationSnapshot, RuntimeError> {
    runtime(&app)?.clear().await
}

struct SettingsService(Arc<Mutex<SettingsStore>>);
fn output_options(settings: &Settings) -> OutputOptions {
    OutputOptions {
        auto_insert_enabled: settings.auto_insert_enabled,
        clipboard_auto_copy_consent: settings.clipboard_auto_copy_consent,
        clipboard_fallback: settings.clipboard_fallback,
    }
}
#[tauri::command]
async fn get_settings(app: AppHandle) -> Result<SettingsState, String> {
    app.state::<SettingsService>()
        .0
        .lock()
        .map(|store| store.state.clone())
        .map_err(|_| "Réglages indisponibles.".into())
}
#[tauri::command]
async fn save_settings(
    app: AppHandle,
    mut settings: Settings,
) -> Result<SettingsState, RuntimeError> {
    let parsed = parse_shortcut(&settings.shortcut).map_err(|message| RuntimeError {
        code: ErrorCode::ProtocolError,
        message,
    })?;
    settings.shortcut = parsed.into_string();
    settings.validate().map_err(|message| RuntimeError {
        code: ErrorCode::ProtocolError,
        message,
    })?;
    let runtime = runtime(&app)?;
    runtime.ensure_idle()?;
    if settings.microphone_id != "default"
        && !runtime
            .microphones()
            .await?
            .iter()
            .any(|device| device.id == settings.microphone_id)
    {
        return Err(runtime_error(ErrorCode::MicUnavailable));
    }
    let options = output_options(&settings);
    let owner = app.clone();
    runtime
        .apply_options(options, move || {
            let state = owner.state::<SettingsService>();
            let hotkey = owner.state::<HotkeyService>();
            let mut store = state
                .0
                .lock()
                .map_err(|_| runtime_error(ErrorCode::ProtocolError))?;
            let previous = hotkey
                .manager
                .lock()
                .map_err(|_| runtime_error(ErrorCode::ProtocolError))?
                .current()
                .map(str::to_owned);
            let result = store.commit(
                settings.clone(),
                || {
                    if hotkey.supported {
                        hotkey
                            .manager
                            .lock()
                            .map_err(|_| "Raccourci indisponible.".to_owned())?
                            .replace(&settings.shortcut)
                            .map_err(|e| e.to_string())?;
                    }
                    Ok(())
                },
                || {
                    if hotkey.supported {
                        let mut manager = hotkey
                            .manager
                            .lock()
                            .map_err(|_| "Raccourci indisponible.".to_owned())?;
                        match previous {
                            Some(previous) => manager.replace(&previous),
                            None => manager.clear(),
                        }
                        .map_err(|e| e.to_string())?;
                    }
                    Ok(())
                },
            );
            result.map_err(|message| RuntimeError {
                code: ErrorCode::ProtocolError,
                message,
            })?;
            if let Ok(mut error) = hotkey.error.lock() {
                *error = None;
            }
            schedule_cancellation(&owner, None, Some(parsed.mods));
            if let Ok(status) = hotkey.snapshot() {
                let _ = owner.emit("hotkey://event", status);
            }
            Ok(())
        })
        .await?;
    get_settings(app).await.map_err(|message| RuntimeError {
        code: ErrorCode::ProtocolError,
        message,
    })
}
#[tauri::command]
async fn copy_dictation_result(app: AppHandle) -> Result<DictationSnapshot, RuntimeError> {
    runtime(&app)?.copy_result().await
}
#[tauri::command]
async fn clear_dictation_clipboard(app: AppHandle) -> Result<DictationSnapshot, RuntimeError> {
    runtime(&app)?.clear_clipboard().await
}
#[derive(Serialize)]
struct Diagnostics {
    x11_supported: bool,
    microphone_available: bool,
    model_available: bool,
    engine_ready: bool,
    xdotool_available: bool,
    xclip_available: bool,
}
#[tauri::command]
async fn get_diagnostics(app: AppHandle) -> Result<Diagnostics, RuntimeError> {
    let runtime = runtime(&app)?;
    let snapshot = runtime.status().await?;
    let output = runtime.output_diagnostics().await?;
    let settings = get_settings(app.clone())
        .await
        .map_err(|_| runtime_error(ErrorCode::ProtocolError))?
        .settings;
    let microphone_available = runtime
        .microphones()
        .await
        .is_ok_and(|devices| crate::audio::microphone_available(&devices, &settings.microphone_id));
    let model_available = if settings.provider == crate::protocol::TranscriptionProvider::Groq {
        snapshot.engine_ready && snapshot.provider == settings.provider
    } else {
        !settings.model_path.is_empty() && std::path::Path::new(&settings.model_path).is_dir()
    };
    Ok(Diagnostics {
        x11_supported: app.state::<HotkeyService>().supported && output.x11,
        microphone_available,
        model_available,
        engine_ready: snapshot.engine_ready,
        xdotool_available: output.xdotool_available,
        xclip_available: output.xclip_available,
    })
}

fn cancellation_active(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::LoadingEngine
            | Phase::Recording
            | Phase::Transcribing
            | Phase::Inserting
            | Phase::Cancelling
    )
}
struct CancellationShortcuts {
    active: bool,
    modifiers: Modifiers,
    registered: Vec<String>,
}
impl Default for CancellationShortcuts {
    fn default() -> Self {
        Self {
            active: false,
            modifiers: Modifiers::CONTROL | Modifiers::ALT,
            registered: Vec::new(),
        }
    }
}
fn cancellation_shortcuts(modifiers: Modifiers) -> Vec<String> {
    let mut shortcuts = vec![Shortcut::new(None, Code::Escape).into_string()];
    if !modifiers.is_empty() {
        shortcuts.push(Shortcut::new(Some(modifiers), Code::Escape).into_string());
    }
    shortcuts
}
fn schedule_cancellation(app: &AppHandle, active: Option<bool>, modifiers: Option<Modifiers>) {
    let handle = app.clone();
    // Plugin operations run inline on the UI thread. This state is separate from
    // HotkeyManager, so registration never waits for a worker holding its lock.
    let _ = app.run_on_main_thread(move || {
        let service = handle.state::<HotkeyService>();
        let Ok(mut state) = service.cancellation.lock() else { return; };
        if active.is_none_or(|value| value == state.active)
            && modifiers.is_none_or(|value| value == state.modifiers) {
            return;
        }
        if let Some(active) = active { state.active = active; }
        if let Some(modifiers) = modifiers { state.modifiers = modifiers; }
        let desired = if state.active { cancellation_shortcuts(state.modifiers) } else { Vec::new() };
        let mut failed = false;
        let previous = std::mem::take(&mut state.registered);
        for shortcut in previous {
            if desired.contains(&shortcut) {
                state.registered.push(shortcut);
            } else if handle.global_shortcut().unregister(shortcut.as_str()).is_err() {
                failed = true;
                // Retain it so the next state update retries unregistering.
                state.registered.push(shortcut);
            }
        }
        for shortcut in desired {
            if !state.registered.contains(&shortcut) {
                if handle.global_shortcut().register(shortcut.as_str()).is_ok() {
                    state.registered.push(shortcut);
                } else { failed = true; }
            }
        }
        if failed {
            let message = if state.active {
                "Échap n’a pas pu être activé complètement. Utilisez le bouton Annuler pour interrompre la dictée."
            } else {
                "Le raccourci Échap n’a pas pu être libéré. Relancez l’application avant de continuer."
            };
            let _ = handle.emit("dictation://notice", message);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escape_is_scoped_to_active_phases() {
        for phase in [
            Phase::Unconfigured,
            Phase::Ready,
            Phase::Success,
            Phase::Error,
        ] {
            assert!(!cancellation_active(phase));
        }
        for phase in [
            Phase::LoadingEngine,
            Phase::Recording,
            Phase::Transcribing,
            Phase::Inserting,
            Phase::Cancelling,
        ] {
            assert!(cancellation_active(phase));
        }
    }
    #[test]
    fn escape_includes_current_push_to_talk_modifiers() {
        let keys = cancellation_shortcuts(Modifiers::CONTROL | Modifiers::SHIFT);
        assert_eq!(keys.len(), 2);
        let modified: Shortcut = keys[1].parse().unwrap();
        assert_eq!(modified.key, Code::Escape);
        assert_eq!(modified.mods, Modifiers::CONTROL | Modifiers::SHIFT);
        assert_eq!(cancellation_shortcuts(Modifiers::empty()).len(), 1);
    }
}
