export type Language = 'fr' | 'en' | 'auto';
export type TranscriptionProvider = 'local' | 'groq';
export interface LocalModelInfo { id: string; name: string; description: string; path: string; size_bytes: number; is_default: boolean }
export interface Microphone { id: string; name: string; is_default: boolean }
export interface RuntimeConfig { provider: TranscriptionProvider; model_path: string; microphone_id: string; language: Language }
export interface DictationSnapshot {
  revision: number;
  phase: 'unconfigured' | 'loading_engine' | 'ready' | 'recording' | 'transcribing' | 'inserting' | 'cancelling' | 'success' | 'error';
  engine_ready: boolean;
  delivery?: 'none' | 'inserted' | 'clipboard' | 'preview' | 'fallback';
  clipboard_owned?: boolean;
  auto_insert_enabled?: boolean;
  insertion_uncertain?: boolean;
  provider?: TranscriptionProvider;
  model_id?: string | null;
  session_id: number | null;
  microphone_id: string;
  language: Language;
  recording_ms: number;
  audio_duration_ms: number | null;
  text: string | null;
  error_code: string | null;
  reason?: 'recording_limit' | null;
  message: string;
}

export interface Settings {
  schema_version: number;
  provider: TranscriptionProvider;
  model_path: string;
  microphone_id: string;
  language: Language;
  shortcut: string;
  auto_insert_enabled: boolean;
  clipboard_auto_copy_consent: boolean;
  clipboard_fallback: boolean;
  max_recording_seconds: number;
}
export interface SettingsState { settings: Settings; error: string | null }
export interface HotkeyStatus {
  revision: number;
  shortcut: string | null;
  held: boolean;
  supported: boolean;
  error: string | null;
  press_count: number;
  release_count: number;
}
export interface Diagnostics {
  x11_supported: boolean;
  microphone_available: boolean;
  model_available: boolean;
  engine_ready: boolean;
  xdotool_available: boolean;
  xclip_available: boolean;
}
export const defaultSettings: Settings = {
  schema_version: 1, provider: 'local', model_path: '', microphone_id: 'default', language: 'fr',
  shortcut: 'Ctrl+Alt+Space', auto_insert_enabled: true, clipboard_auto_copy_consent: false,
  clipboard_fallback: true, max_recording_seconds: 60,
};
