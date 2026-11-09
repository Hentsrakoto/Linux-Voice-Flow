/** UI contract. Audio buffers, temporary paths and engine payloads stay in Rust. */
export type SessionId = number;
export type ErrorCode =
  | 'MIC_UNAVAILABLE' | 'NO_SPEECH' | 'MODEL_NOT_FOUND' | 'MODEL_LOAD_FAILED'
  | 'ENGINE_BUSY' | 'ENGINE_TIMEOUT' | 'ENGINE_CRASHED' | 'FOCUS_CHANGED'
  | 'INJECTION_FAILED' | 'CLIPBOARD_FAILED' | 'CANCELLED' | 'PROTOCOL_ERROR';
export interface AppError { code: ErrorCode }
export type DictationState =
  | { state: 'initializing' | 'ready' }
  | { state: 'recording' | 'transcribing' | 'inserting' | 'success' | 'cancelling' | 'cancelled'; session_id: SessionId }
  | { state: 'error'; session_id: SessionId; code: ErrorCode };
export interface DictationStateEvent { state: DictationState }
