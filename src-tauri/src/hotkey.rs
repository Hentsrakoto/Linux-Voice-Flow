//! Desktop-independent shortcut policy. X11 registration lives in `desktop`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEvent {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyError {
    Invalid,
    Unavailable,
    Busy,
    RollbackFailed,
}

impl std::fmt::Display for HotkeyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Invalid => "Raccourci invalide. Utilisez par exemple Ctrl+Alt+Space.",
            Self::Unavailable => "Raccourci indisponible. L’ancien raccourci est conservé.",
            Self::Busy => "Relâchez le raccourci avant de le modifier.",
            Self::RollbackFailed => "Échec de restauration du raccourci. Relancez l’application.",
        })
    }
}

pub trait ShortcutBackend {
    fn register(&mut self, shortcut: &str) -> Result<(), HotkeyError>;
    fn unregister(&mut self, shortcut: &str) -> Result<(), HotkeyError>;
}

pub struct HotkeyManager<B: ShortcutBackend> {
    backend: B,
    current: Option<String>,
    held: bool,
    /// Total order for snapshots delivered over command responses and events.
    pub revision: u64,
    pub press_count: u32,
    pub release_count: u32,
}

impl<B: ShortcutBackend> HotkeyManager<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            current: None,
            held: false,
            revision: 0,
            press_count: 0,
            release_count: 0,
        }
    }

    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }
    pub fn held(&self) -> bool {
        self.held
    }

    /// The backend validates syntax; callers supply its canonical representation.
    pub fn replace(&mut self, shortcut: &str) -> Result<(), HotkeyError> {
        if self.held {
            return Err(HotkeyError::Busy);
        }
        if self.current() == Some(shortcut) {
            return Ok(());
        }
        self.backend.register(shortcut)?;
        if let Some(previous) = self.current.as_deref() {
            if let Err(error) = self.backend.unregister(previous) {
                if self.backend.unregister(shortcut).is_err() {
                    return Err(HotkeyError::RollbackFailed);
                }
                return Err(error);
            }
        }
        self.current = Some(shortcut.to_owned());
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    /// Remove our registration, used when a settings transaction must roll back
    /// an initially unregistered shortcut.
    pub fn clear(&mut self) -> Result<(), HotkeyError> {
        if self.held {
            return Err(HotkeyError::Busy);
        }
        if let Some(current) = self.current.as_deref() {
            self.backend.unregister(current)?;
            self.current = None;
            self.revision = self.revision.saturating_add(1);
        }
        Ok(())
    }

    /// Only a change in physical state emits an event; auto-repeat is ignored.
    pub fn handle(&mut self, shortcut: &str, event: HotkeyEvent) -> Option<HotkeyEvent> {
        if self.current() != Some(shortcut) {
            return None;
        }
        match (self.held, event) {
            (false, HotkeyEvent::Pressed) => {
                self.held = true;
                self.press_count = self.press_count.saturating_add(1);
            }
            (true, HotkeyEvent::Released) => {
                self.held = false;
                self.release_count = self.release_count.saturating_add(1);
            }
            _ => return None,
        }
        self.revision = self.revision.saturating_add(1);
        Some(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[derive(Default)]
    struct Backend {
        registered: HashSet<String>,
        reject: Option<String>,
        fail_remove: Option<String>,
    }

    impl ShortcutBackend for Backend {
        fn register(&mut self, shortcut: &str) -> Result<(), HotkeyError> {
            if self.reject.as_deref() == Some(shortcut) {
                return Err(HotkeyError::Unavailable);
            }
            self.registered.insert(shortcut.into());
            Ok(())
        }
        fn unregister(&mut self, shortcut: &str) -> Result<(), HotkeyError> {
            if self.fail_remove.as_deref() == Some(shortcut) {
                return Err(HotkeyError::Unavailable);
            }
            self.registered.remove(shortcut);
            Ok(())
        }
    }

    fn manager() -> HotkeyManager<Backend> {
        let mut manager = HotkeyManager::new(Backend::default());
        manager.replace("Ctrl+Alt+Space").unwrap();
        manager
    }

    #[test]
    fn held_key_emits_one_press_and_one_release() {
        let mut manager = manager();
        assert_eq!(
            manager.handle("Ctrl+Alt+Space", HotkeyEvent::Pressed),
            Some(HotkeyEvent::Pressed)
        );
        assert_eq!(manager.handle("Ctrl+Alt+Space", HotkeyEvent::Pressed), None);
        assert_eq!(
            manager.handle("Ctrl+Alt+Space", HotkeyEvent::Released),
            Some(HotkeyEvent::Released)
        );
        assert_eq!(
            manager.handle("Ctrl+Alt+Space", HotkeyEvent::Released),
            None
        );
    }

    #[test]
    fn failed_replace_preserves_previous_shortcut() {
        let mut manager = manager();
        manager.backend.reject = Some("Ctrl+Q".into());
        assert_eq!(manager.replace("Ctrl+Q"), Err(HotkeyError::Unavailable));
        assert_eq!(manager.current(), Some("Ctrl+Alt+Space"));
        assert!(manager.backend.registered.contains("Ctrl+Alt+Space"));
    }

    #[test]
    fn failed_unregister_rolls_back_new_registration() {
        let mut manager = manager();
        manager.backend.fail_remove = Some("Ctrl+Alt+Space".into());
        assert!(manager.replace("Ctrl+Q").is_err());
        assert_eq!(manager.current(), Some("Ctrl+Alt+Space"));
        assert!(!manager.backend.registered.contains("Ctrl+Q"));
    }

    #[test]
    fn cannot_replace_held_shortcut_and_ignores_unrelated_events() {
        let mut manager = manager();
        assert_eq!(manager.handle("Ctrl+Q", HotkeyEvent::Pressed), None);
        manager.handle("Ctrl+Alt+Space", HotkeyEvent::Pressed);
        assert_eq!(manager.replace("Ctrl+Q"), Err(HotkeyError::Busy));
        manager.handle("Ctrl+Alt+Space", HotkeyEvent::Released);
        assert!(manager.replace("Ctrl+Q").is_ok());
    }

    #[test]
    fn snapshot_revision_increases_only_when_state_changes() {
        let mut manager = manager();
        let registered = manager.revision;
        manager.handle("Ctrl+Alt+Space", HotkeyEvent::Pressed);
        assert!(manager.revision > registered);
        let pressed = manager.revision;
        manager.handle("Ctrl+Alt+Space", HotkeyEvent::Pressed);
        assert_eq!(manager.revision, pressed);
        manager.handle("Ctrl+Alt+Space", HotkeyEvent::Released);
        assert!(manager.revision > pressed);
        let released = manager.revision;
        manager.replace("Ctrl+Q").unwrap();
        assert!(manager.revision > released);
    }

    #[test]
    fn replacing_same_shortcut_is_a_noop() {
        let mut manager = manager();
        manager.backend.reject = Some("Ctrl+Alt+Space".into());
        assert!(manager.replace("Ctrl+Alt+Space").is_ok());
    }
    #[test]
    fn clearing_a_new_registration_supports_transaction_rollback() {
        let mut manager = manager();
        manager.handle("Ctrl+Alt+Space", HotkeyEvent::Pressed);
        assert_eq!(manager.clear(), Err(HotkeyError::Busy));
        manager.handle("Ctrl+Alt+Space", HotkeyEvent::Released);
        manager.clear().unwrap();
        assert!(manager.current().is_none());
        assert!(manager.backend.registered.is_empty());
    }
}
