use crate::protocol::ErrorCode;
use serde::{Deserialize, Serialize};
pub type SessionId = u64;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppError {
    pub code: ErrorCode,
}
/// Session IDs must be monotonic and never reused during process lifetime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DictationState {
    Initializing,
    Ready,
    Recording {
        session_id: SessionId,
    },
    Transcribing {
        session_id: SessionId,
    },
    Inserting {
        session_id: SessionId,
    },
    Success {
        session_id: SessionId,
    },
    Cancelling {
        session_id: SessionId,
    },
    Cancelled {
        session_id: SessionId,
    },
    Error {
        session_id: SessionId,
        code: ErrorCode,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Initialized,
    InitializationFailed {
        code: ErrorCode,
    },
    Press {
        session_id: SessionId,
    },
    Release {
        session_id: SessionId,
    },
    Result {
        session_id: SessionId,
    },
    Inserted {
        session_id: SessionId,
    },
    Cancel {
        session_id: SessionId,
    },
    Cancelled {
        session_id: SessionId,
    },
    CancelTimeout {
        session_id: SessionId,
    },
    Timeout {
        session_id: SessionId,
    },
    Failure {
        session_id: SessionId,
        code: ErrorCode,
    },
    Reset,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    StartRecording { session_id: SessionId },
    Transcribe { session_id: SessionId },
    Insert { session_id: SessionId },
    Cleanup { session_id: SessionId },
    CancelEngine { session_id: SessionId },
    RestartEngine,
}
impl DictationState {
    pub fn session_id(&self) -> Option<SessionId> {
        match self {
            Self::Initializing | Self::Ready => None,
            Self::Recording { session_id }
            | Self::Transcribing { session_id }
            | Self::Inserting { session_id }
            | Self::Success { session_id }
            | Self::Cancelling { session_id }
            | Self::Cancelled { session_id }
            | Self::Error { session_id, .. } => Some(*session_id),
        }
    }
}
impl Event {
    fn id(&self) -> Option<SessionId> {
        match self {
            Self::Initialized | Self::InitializationFailed { .. } | Self::Reset => None,
            Self::Press { session_id }
            | Self::Release { session_id }
            | Self::Result { session_id }
            | Self::Inserted { session_id }
            | Self::Cancel { session_id }
            | Self::Cancelled { session_id }
            | Self::CancelTimeout { session_id }
            | Self::Timeout { session_id }
            | Self::Failure { session_id, .. } => Some(*session_id),
        }
    }
}
/// Pure reducer: adapters execute returned effects once, in order. Result carries no text.
pub fn transition(
    state: DictationState,
    event: Event,
) -> Result<(DictationState, Vec<Effect>), AppError> {
    use DictationState as S;
    use Effect as F;
    use Event as E;
    // Identity-bound asynchronous events never affect another or completed session.
    if !matches!(event, E::Press { .. }) && event.id().is_some() && event.id() != state.session_id()
    {
        return Ok((state, vec![]));
    }
    let next = match (&state, event) {
        (S::Initializing, E::Initialized) => (S::Ready, vec![]),
        (S::Initializing, E::InitializationFailed { code }) => (
            S::Error {
                session_id: 0,
                code,
            },
            vec![],
        ),
        (S::Ready, E::Press { session_id })
            if session_id == 0 || session_id > 9_007_199_254_740_991 =>
        {
            return Err(AppError {
                code: ErrorCode::ProtocolError,
            })
        }
        (S::Ready, E::Press { session_id }) => (
            S::Recording { session_id },
            vec![F::StartRecording { session_id }],
        ),
        (S::Recording { session_id: active }, E::Press { session_id }) if *active == session_id => {
            (state.clone(), vec![])
        }
        (S::Recording { .. }, E::Release { session_id }) => (
            S::Transcribing { session_id },
            vec![F::Transcribe { session_id }],
        ),
        (S::Transcribing { .. }, E::Result { session_id }) => {
            (S::Inserting { session_id }, vec![F::Insert { session_id }])
        }
        (S::Inserting { .. }, E::Inserted { session_id }) => {
            (S::Success { session_id }, vec![F::Cleanup { session_id }])
        }
        (S::Recording { .. } | S::Inserting { .. }, E::Cancel { session_id }) => {
            (S::Cancelled { session_id }, vec![F::Cleanup { session_id }])
        }
        (S::Transcribing { .. }, E::Cancel { session_id }) => (
            S::Cancelling { session_id },
            vec![F::CancelEngine { session_id }, F::Cleanup { session_id }],
        ),
        (S::Cancelling { .. }, E::Cancelled { session_id }) => {
            (S::Cancelled { session_id }, vec![])
        }
        (S::Cancelling { .. }, E::CancelTimeout { session_id }) => {
            (S::Cancelled { session_id }, vec![F::RestartEngine])
        }
        (S::Transcribing { .. }, E::Timeout { session_id }) => (
            S::Error {
                session_id,
                code: ErrorCode::EngineTimeout,
            },
            vec![F::Cleanup { session_id }, F::RestartEngine],
        ),
        (
            S::Recording { .. } | S::Transcribing { .. } | S::Inserting { .. },
            E::Failure { session_id, code },
        ) => {
            let next = if code == ErrorCode::NoSpeech {
                S::Ready
            } else {
                S::Error { session_id, code }
            };
            (next, vec![F::Cleanup { session_id }])
        }
        (S::Success { .. } | S::Cancelled { .. } | S::Error { .. }, E::Reset) => (S::Ready, vec![]),
        // Duplicate or late engine events cannot schedule a second insertion.
        (
            _,
            E::Result { .. }
            | E::Cancelled { .. }
            | E::CancelTimeout { .. }
            | E::Timeout { .. }
            | E::Inserted { .. }
            | E::Failure { .. },
        ) => (state.clone(), vec![]),
        _ => {
            return Err(AppError {
                code: ErrorCode::EngineBusy,
            })
        }
    };
    Ok(next)
}

/// Recheck immediately before touching the target, under the same controller lock
/// that applies Cancel. A queued Insert effect alone is never authorization.
pub fn can_insert(state: &DictationState, session_id: SessionId) -> bool {
    matches!(state, DictationState::Inserting { session_id: active } if *active == session_id)
}

impl From<crate::protocol::ProtocolError> for AppError {
    fn from(_: crate::protocol::ProtocolError) -> Self {
        Self {
            code: ErrorCode::ProtocolError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn go(state: DictationState, event: Event) -> (DictationState, Vec<Effect>) {
        transition(state, event).unwrap()
    }
    #[test]
    fn single_session_ignores_stale_and_duplicate_results() {
        let (s, _) = go(DictationState::Ready, Event::Press { session_id: 1 });
        assert!(transition(s.clone(), Event::Press { session_id: 2 }).is_err());
        let (s, _) = go(s, Event::Release { session_id: 1 });
        let (s, e) = go(s, Event::Result { session_id: 2 });
        assert!(e.is_empty());
        let (s, e) = go(s, Event::Result { session_id: 1 });
        assert_eq!(e, vec![Effect::Insert { session_id: 1 }]);
        let (_, e) = go(s, Event::Result { session_id: 1 });
        assert!(e.is_empty());
    }
    #[test]
    fn cancel_invalidates_before_late_result_and_timeout() {
        let (s, _) = go(DictationState::Ready, Event::Press { session_id: 1 });
        let (s, _) = go(s, Event::Release { session_id: 1 });
        let (s, e) = go(s, Event::Cancel { session_id: 1 });
        assert!(e.contains(&Effect::CancelEngine { session_id: 1 }));
        let (s, e) = go(s, Event::Result { session_id: 1 });
        assert!(e.is_empty());
        let (s, e) = go(s, Event::CancelTimeout { session_id: 1 });
        assert_eq!(s, DictationState::Cancelled { session_id: 1 });
        assert!(e.contains(&Effect::RestartEngine));
    }
    #[test]
    fn cancellation_blocks_queued_insertion_and_repeated_press_is_ignored() {
        let (s, _) = go(DictationState::Ready, Event::Press { session_id: 7 });
        let (s, e) = go(s, Event::Press { session_id: 7 });
        assert!(e.is_empty());
        let (s, _) = go(s, Event::Release { session_id: 7 });
        let (s, _) = go(s, Event::Result { session_id: 7 });
        assert!(can_insert(&s, 7));
        assert!(!can_insert(&s, 8));
        let (s, e) = go(s, Event::Cancel { session_id: 7 });
        assert!(!can_insert(&s, 7));
        assert_eq!(e, vec![Effect::Cleanup { session_id: 7 }]);
        let (s, e) = go(s, Event::Inserted { session_id: 7 });
        assert!(e.is_empty());
        assert_eq!(s, DictationState::Cancelled { session_id: 7 });
    }
    #[test]
    fn initialization_failure_is_recoverable_and_reserved_id_rejected() {
        let (s, e) = go(
            DictationState::Initializing,
            Event::InitializationFailed {
                code: ErrorCode::ModelNotFound,
            },
        );
        assert!(matches!(
            s,
            DictationState::Error {
                session_id: 0,
                code: ErrorCode::ModelNotFound
            }
        ));
        assert!(e.is_empty());
        let (s, _) = go(s, Event::Reset);
        assert_eq!(s, DictationState::Ready);
        assert!(transition(s, Event::Press { session_id: 0 }).is_err());
    }
    #[test]
    fn silence_and_failure_clean_without_insertion() {
        let (s, _) = go(DictationState::Ready, Event::Press { session_id: 3 });
        let (s, _) = go(s, Event::Release { session_id: 3 });
        let (s, e) = go(
            s,
            Event::Failure {
                session_id: 3,
                code: ErrorCode::NoSpeech,
            },
        );
        assert_eq!(s, DictationState::Ready);
        assert_eq!(e, vec![Effect::Cleanup { session_id: 3 }]);
        assert!(transition(DictationState::Ready, Event::Reset).is_err());
    }
}
