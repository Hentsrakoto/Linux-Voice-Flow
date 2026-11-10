import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AppInfo } from '../ipc/app-info';
import type { Diagnostics, DictationSnapshot, HotkeyStatus, LocalModelInfo, Microphone, Settings, SettingsState } from '../ipc/runtime';
import { commandError } from './messages';

export function useVoiceFlow(native: boolean) {
  const [snapshot, setSnapshot] = useState<DictationSnapshot | null>(null);
  const [hotkey, setHotkey] = useState<HotkeyStatus | null>(null);
  const [settingsState, setSettingsState] = useState<SettingsState | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [models, setModels] = useState<LocalModelInfo[]>([]);
  const [microphones, setMicrophones] = useState<Microphone[]>([]);
  const [diagnostics, setDiagnostics] = useState<Diagnostics | null>(null);
  const [listError, setListError] = useState(false);
  const [diagnosticError, setDiagnosticError] = useState(false);
  const [loading, setLoading] = useState(native);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const alive = useRef(false);
  const listRequest = useRef(0);
  const accept = useCallback((candidate: DictationSnapshot) => {
    if (alive.current && candidate) setSnapshot(current => !current || candidate.revision >= current.revision ? candidate : current);
  }, []);
  const acceptHotkey = useCallback((candidate: HotkeyStatus) => {
    if (alive.current && candidate) setHotkey(current => !current || candidate.revision >= current.revision ? candidate : current);
  }, []);
  const refreshLists = useCallback(async () => {
    const request = ++listRequest.current;
    const result = await Promise.allSettled([invoke<LocalModelInfo[]>('list_local_models'), invoke<Microphone[]>('list_microphones')]);
    if (!alive.current || request !== listRequest.current) return;
    if (result[0].status === 'fulfilled') setModels(result[0].value);
    if (result[1].status === 'fulfilled') setMicrophones(result[1].value);
    setListError(result.some(item => item.status === 'rejected'));
  }, []);
  const refreshDiagnostics = useCallback(async () => {
    setDiagnosticError(false);
    try { const value = await invoke<Diagnostics>('get_diagnostics'); if (alive.current) setDiagnostics(value); }
    catch { if (alive.current) setDiagnosticError(true); }
  }, []);
  useEffect(() => {
    if (!native) return;
    alive.current = true;
    let active = true;
    const subscriptions: (() => void)[] = [];
    async function subscribe<T>(name: string, onValue: (value: T) => void) {
      const stop = await listen<T>(name, event => { if (active) onValue(event.payload); });
      if (!active) { stop(); return; }
      subscriptions.push(stop);
    }
    async function connect() {
      setLoading(true);
      try {
        await Promise.all([
          subscribe<DictationSnapshot>('dictation://state', accept),
          subscribe<HotkeyStatus>('hotkey://event', acceptHotkey),
          subscribe<string>('dictation://notice', value => setNotice(value || null)),
        ]);
        if (!active) return;
        const result = await Promise.all([
          invoke<AppInfo>('get_app_info'), invoke<SettingsState>('get_settings'),
          invoke<DictationSnapshot>('get_dictation_status'), invoke<HotkeyStatus>('get_hotkey_status'),
        ]);
        if (!active) return;
        setInfo(result[0]); setSettingsState(result[1]); accept(result[2]); acceptHotkey(result[3]);
        void refreshLists();
      } catch { if (active) setError('Impossible de vérifier l’application. Réessayez ou relancez la fenêtre.'); }
      finally { if (active) setLoading(false); }
    }
    void connect();
    return () => { active = false; alive.current = false; subscriptions.forEach(stop => stop()); };
  }, [native, attempt, accept, acceptHotkey, refreshLists, refreshDiagnostics]);
  const phase = snapshot?.phase;
  const provider = snapshot?.provider;
  useEffect(() => {
    if (native && !loading && phase && !['loading_engine', 'recording', 'transcribing', 'inserting', 'cancelling'].includes(phase)) {
      void refreshDiagnostics();
    }
  }, [native, loading, phase, provider, refreshDiagnostics]);
  const busy = pending || !!snapshot && ['loading_engine', 'recording', 'transcribing', 'inserting', 'cancelling'].includes(snapshot.phase);
  async function run(command: string) {
    setPending(true); setError(null);
    try { accept(await invoke<DictationSnapshot>(command)); }
    catch (reason) { if (alive.current) setError(commandError(reason, snapshot?.provider)); }
    finally { if (alive.current) setPending(false); }
  }
  async function save(settings: Settings) {
    setPending(true); setError(null);
    try {
      const saved = await invoke<SettingsState>('save_settings', { settings });
      if (!alive.current) return false;
      setSettingsState(saved);
      if (saved.error) { setError('Les réglages enregistrés ne peuvent pas être appliqués. Vérifiez leur fichier avant de réessayer.'); return false; }
      try { acceptHotkey(await invoke<HotkeyStatus>('get_hotkey_status')); }
      catch { if (alive.current) setHotkey(null); }
      const config = { provider: saved.settings.provider, model_path: saved.settings.provider === 'groq' ? '' : saved.settings.model_path, microphone_id: saved.settings.microphone_id, language: saved.settings.language };
      accept(await invoke<DictationSnapshot>('configure_dictation', { config }));
      void refreshDiagnostics();
      return true;
    } catch (reason) { if (alive.current) setError(commandError(reason, settings.provider)); return false; }
    finally { if (alive.current) setPending(false); }
  }
  return { native, snapshot, hotkey, settingsState, info, models, microphones, diagnostics, listError, diagnosticError, loading, pending, busy, error, notice, run, save, refreshLists, refreshDiagnostics,
    retry: () => { setError(null); setAttempt(value => value + 1); } };
}
export type VoiceFlow = ReturnType<typeof useVoiceFlow>;
