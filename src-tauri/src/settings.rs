//! Versioned private settings; never contains transcript, audio or API credentials.
use crate::protocol::{Language, TranscriptionProvider};
use serde::{Deserialize, Serialize};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub schema_version: u32,
    pub provider: TranscriptionProvider,
    pub model_path: String,
    pub microphone_id: String,
    pub language: Language,
    pub shortcut: String,
    pub auto_insert_enabled: bool,
    pub clipboard_auto_copy_consent: bool,
    pub clipboard_fallback: bool,
    pub max_recording_seconds: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            provider: TranscriptionProvider::Local,
            model_path: String::new(),
            microphone_id: "default".into(),
            language: Language::Fr,
            shortcut: "Ctrl+Alt+Space".into(),
            auto_insert_enabled: true,
            clipboard_auto_copy_consent: false,
            clipboard_fallback: true,
            max_recording_seconds: 60,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.max_recording_seconds != 60
            || self.microphone_id.is_empty()
            || self.microphone_id.len() > 1024
            || self.microphone_id.contains('\0')
            || self.model_path.len() > 4096
            || self.model_path.contains('\0')
            || self.shortcut.len() > 128
            || !self.shortcut.contains('+')
            || self.shortcut.chars().any(char::is_control)
        {
            return Err("Réglages invalides. Vérifiez le raccourci, le microphone et la limite de 60 secondes.".into());
        }
        if self.provider == TranscriptionProvider::Local && !self.model_path.is_empty() {
            let path = Path::new(&self.model_path);
            if !path.is_absolute()
                || path.canonicalize().ok().as_deref() != Some(path)
                || !path.is_dir()
                || path.ancestors().any(|part| {
                    fs::symlink_metadata(part).is_ok_and(|m| m.file_type().is_symlink())
                })
            {
                return Err(
                    "Modèle local introuvable. Choisissez un dossier absolu accessible.".into(),
                );
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct SettingsState {
    pub settings: Settings,
    pub error: Option<String>,
}
pub struct SettingsStore {
    pub state: SettingsState,
    path: PathBuf,
}
fn real_ancestors(path: &Path) -> Result<(), String> {
    for part in path.ancestors() {
        match fs::symlink_metadata(part) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err("Répertoire de réglages non sûr.".into())
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err("Répertoire de réglages inaccessible.".into())
            }
            _ => {}
        }
    }
    Ok(())
}
impl SettingsStore {
    pub fn load(path: PathBuf) -> Self {
        let loaded = (|| -> Result<Settings, String> {
            real_ancestors(&path)?;
            let source = fs::OpenOptions::new()
                .read(true)
                .custom_flags(
                    rustix::fs::OFlags::NOFOLLOW.bits() as i32
                        | rustix::fs::OFlags::NONBLOCK.bits() as i32,
                )
                .open(&path);
            let mut source = match source {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Settings::default())
                }
                Err(_) => {
                    return Err(
                        "Impossible de lire les réglages. Le fichier existant est conservé.".into(),
                    )
                }
                Ok(source) => source,
            };
            let metadata = source.metadata().map_err(|_| "Réglages inaccessibles.")?;
            if !metadata.is_file()
                || metadata.uid() != rustix::process::getuid().as_raw()
                || metadata.permissions().mode() & 0o777 != 0o600
            {
                return Err(
                    "Fichier de réglages non privé. Protégez-le avec les permissions 0600.".into(),
                );
            }
            let mut bytes = Vec::new();
            Read::by_ref(&mut source)
                .take(65537)
                .read_to_end(&mut bytes)
                .map_err(|_| "Impossible de lire les réglages.")?;
            if bytes.len() > 65536 {
                return Err("Fichier de réglages trop volumineux.".into());
            }
            let settings: Settings = serde_json::from_slice(&bytes).map_err(|_| "Réglages corrompus ou version inconnue. Corrigez le fichier ou enregistrez explicitement de nouveaux réglages.")?;
            settings.validate()?;
            Ok(settings)
        })();
        let state = match loaded {
            Ok(settings) => SettingsState {
                settings,
                error: None,
            },
            Err(error) => SettingsState {
                settings: Settings::default(),
                error: Some(error),
            },
        };
        Self { state, path }
    }
    /// Stage first, then apply the runtime change; roll it back if publication fails.
    pub fn commit(
        &mut self,
        settings: Settings,
        apply: impl FnOnce() -> Result<(), String>,
        rollback: impl FnOnce() -> Result<(), String>,
    ) -> Result<SettingsState, String> {
        settings.validate()?;
        real_ancestors(&self.path)?;
        let parent = self
            .path
            .parent()
            .ok_or("Répertoire de réglages invalide.")?;
        fs::create_dir_all(parent)
            .map_err(|_| "Impossible de créer le répertoire des réglages.")?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Impossible de protéger les réglages.")?;
        let mut staged = tempfile::NamedTempFile::new_in(parent)
            .map_err(|_| "Impossible de préparer la sauvegarde des réglages.")?;
        staged
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| "Impossible de protéger les réglages.")?;
        serde_json::to_writer_pretty(&mut staged, &settings).map_err(|_| "Réglages invalides.")?;
        staged
            .write_all(b"\n")
            .map_err(|_| "Impossible d’écrire les réglages.")?;
        staged
            .as_file()
            .sync_all()
            .map_err(|_| "Impossible de synchroniser les réglages.")?;
        apply()?;
        if staged.persist(&self.path).is_err() {
            rollback().map_err(|_| {
                "Restauration du raccourci impossible. Relancez l’application.".to_owned()
            })?;
            return Err("Sauvegarde impossible. Les anciens réglages sont conservés.".into());
        }
        // Rename has committed. A directory sync failure must not report a false rollback.
        if let Ok(directory) = fs::File::open(parent) {
            let _ = directory.sync_all();
        }
        self.state = SettingsState {
            settings,
            error: None,
        };
        Ok(self.state.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    #[test]
    fn missing_defaults_save_and_reload_private() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("settings/settings.json");
        let mut store = SettingsStore::load(path.clone());
        assert!(store.state.error.is_none());
        assert!(!store.state.settings.clipboard_auto_copy_consent);
        assert!(store.state.settings.auto_insert_enabled);
        let mut changed = store.state.settings.clone();
        changed.language = Language::En;
        store.commit(changed, || Ok(()), || Ok(())).unwrap();
        assert_eq!(
            SettingsStore::load(path.clone()).state.settings.language,
            Language::En
        );
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    #[test]
    fn corrupt_or_unknown_version_is_preserved() {
        let root = tempfile::tempdir().unwrap();
        let mut unknown = serde_json::to_value(Settings::default()).unwrap();
        unknown["schema_version"] = 99.into();
        let unknown = serde_json::to_vec(&unknown).unwrap();
        for bytes in [b"not-json".as_slice(), unknown.as_slice()] {
            let path = root.path().join("settings.json");
            std::fs::write(&path, bytes).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            let store = SettingsStore::load(path.clone());
            assert!(store.state.error.is_some());
            assert_eq!(std::fs::read(path).unwrap(), bytes);
        }
    }
    #[test]
    fn failed_application_keeps_disk_and_state() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("settings.json");
        let mut store = SettingsStore::load(path.clone());
        store
            .commit(Settings::default(), || Ok(()), || Ok(()))
            .unwrap();
        let original = std::fs::read(&path).unwrap();
        let mut changed = store.state.settings.clone();
        changed.shortcut = "Ctrl+Shift+Space".into();
        assert!(store
            .commit(changed, || Err("Raccourci indisponible.".into()), || Ok(()))
            .is_err());
        assert_eq!(store.state.settings.shortcut, Settings::default().shortcut);
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
    #[test]
    fn failed_publish_rolls_back_application() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("settings.json");
        let mut store = SettingsStore::load(path.clone());
        let rolled = AtomicBool::new(false);
        assert!(store
            .commit(
                Settings::default(),
                || {
                    std::fs::create_dir(&path).unwrap();
                    Ok(())
                },
                || {
                    rolled.store(true, Ordering::SeqCst);
                    Ok(())
                }
            )
            .is_err());
        assert!(rolled.load(Ordering::SeqCst));
    }
    #[test]
    fn validation_rejects_unknown_fields_and_unsafe_values() {
        let mut settings = Settings {
            max_recording_seconds: 0,
            ..Settings::default()
        };
        assert!(settings.validate().is_err());
        settings.max_recording_seconds = 61;
        assert!(settings.validate().is_err());
        settings.max_recording_seconds = 60;
        settings.microphone_id.clear();
        assert!(settings.validate().is_err());
        let mut value = serde_json::to_value(Settings::default()).unwrap();
        value["api_key"] = "not accepted".into();
        assert!(serde_json::from_value::<Settings>(value).is_err());
    }
}
