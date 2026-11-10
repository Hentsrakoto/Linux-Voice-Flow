import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import App from './App';
import type { DictationSnapshot } from '../shared/ipc/runtime';
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
const defaults = { schema_version: 1, provider: 'local', model_path: '/models/tiny', microphone_id: 'default', language: 'fr', shortcut: 'Ctrl+Alt+Space', auto_insert_enabled: true, clipboard_auto_copy_consent: false, clipboard_fallback: true, max_recording_seconds: 60 };
const state = (revision = 0, phase: DictationSnapshot['phase'] = 'ready'): DictationSnapshot => ({ revision, phase, engine_ready: true, provider: 'local', model_id: 'tiny', session_id: null, microphone_id: 'default', language: 'fr', recording_ms: 0, audio_duration_ms: null, text: null, error_code: null, message: '' });
const callbacks: Record<string, (event: { payload: unknown }) => void> = {};
const stop = vi.fn();
function mockCommands(overrides: Record<string, unknown> = {}) {
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command in overrides) return overrides[command];
    if (command === 'get_app_info') return { name: 'Linux Voice Flow', version: '0.1.0' };
    if (command === 'get_settings') return { settings: defaults, error: null };
    if (command === 'save_settings') return { settings: (args as { settings: unknown }).settings, error: null };
    if (command === 'list_local_models') return [{ id: 'tiny', name: 'Whisper Tiny', path: '/models/tiny', description: 'Léger', size_bytes: 100, is_default: true }];
    if (command === 'list_microphones') return [{ id: 'default', name: 'Microphone par défaut', is_default: true }];
    if (command === 'get_hotkey_status') return { revision: 1, shortcut: 'Ctrl+Alt+Space', held: false, supported: true, error: null, press_count: 0, release_count: 0 };
    if (command === 'get_diagnostics') return { x11_supported: true, microphone_available: true, model_available: true, engine_ready: true, xdotool_available: true, xclip_available: true };
    return state();
  });
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(listen).mockImplementation(async (name, callback) => { callbacks[name] = callback as (event: { payload: unknown }) => void; return stop; });
  mockCommands();
});
it('keeps configuration off the dictation screen and shows the actual shortcut', async () => {
  render(<App />);
  expect(await screen.findByText('Prêt à dicter')).toBeInTheDocument();
  expect(screen.getByText('Espace')).toBeInTheDocument();
  expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
  expect(screen.queryByLabelText('Raccourci de dictée')).not.toBeInTheDocument();
  act(() => callbacks['hotkey://event']({ payload: { revision: 2, shortcut: 'Ctrl+Shift+K', supported: true, held: false } }));
  expect(screen.getByText('K')).toBeInTheDocument();
  expect(screen.queryByText('Espace')).not.toBeInTheDocument();
});
it('restores settings and saves before preparing without putting settings on home', async () => {
  render(<App />);
  await screen.findByText('Prêt à dicter');
  await userEvent.click(screen.getByRole('button', { name: 'Réglages' }));
  expect(await screen.findByLabelText('Langue')).toHaveValue('fr');
  expect(screen.getByLabelText('Raccourci de dictée')).toHaveValue('Ctrl+Alt+Space');
  expect(screen.getByLabelText(/Copier automatiquement/)).not.toBeChecked();
  await userEvent.selectOptions(screen.getByLabelText('Langue'), 'en');
  await userEvent.click(screen.getByRole('button', { name: 'Enregistrer et préparer' }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('configure_dictation', { config: { provider: 'local', model_path: '/models/tiny', microphone_id: 'default', language: 'en' } }));
  const calls = vi.mocked(invoke).mock.calls;
  expect(calls.findIndex(([command]) => command === 'save_settings')).toBeLessThan(calls.findIndex(([command]) => command === 'configure_dictation'));
  await userEvent.click(screen.getByRole('button', { name: 'Dictée' }));
  expect(screen.queryByLabelText('Langue')).not.toBeInTheDocument();
});
it('allows Groq without local models and never sends a key through IPC', async () => {
  mockCommands({ list_local_models: [] });
  render(<App />);
  await screen.findByText('Prêt à dicter');
  await userEvent.click(screen.getByRole('button', { name: 'Réglages' }));
  await userEvent.selectOptions(screen.getByLabelText('Service de transcription'), 'groq');
  expect(screen.getByText(/L’audio sera envoyé à Groq/)).toBeInTheDocument();
  expect(screen.queryByLabelText('Modèle de transcription')).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Enregistrer et préparer' }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith('configure_dictation', { config: { provider: 'groq', model_path: '', microphone_id: 'default', language: 'fr' } }));
  expect(JSON.stringify(vi.mocked(invoke).mock.calls)).not.toContain('api_key');
});
it('preserves unsaved settings across navigation and focuses the active view', async () => {
  render(<App />);
  await screen.findByText('Prêt à dicter');
  await userEvent.click(screen.getByRole('button', { name: 'Réglages' }));
  expect(screen.getByRole('main')).toHaveFocus();
  await userEvent.selectOptions(screen.getByLabelText('Langue'), 'en');
  await userEvent.selectOptions(screen.getByLabelText('Service de transcription'), 'groq');
  await userEvent.click(screen.getByRole('button', { name: 'Dictée' }));
  expect(screen.getByRole('main')).toHaveFocus();
  expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Réglages' }));
  expect(screen.getByLabelText('Langue')).toHaveValue('en');
  expect(screen.getByLabelText('Service de transcription')).toHaveValue('groq');
  expect(invoke).not.toHaveBeenCalledWith('save_settings', expect.anything());
});
it('keeps diagnostics in settings and explains unavailable prerequisites', async () => {
  mockCommands({ get_diagnostics: { x11_supported: false, microphone_available: false, model_available: false, engine_ready: false, xdotool_available: false, xclip_available: false } });
  render(<App />);
  await screen.findByText('Prêt à dicter');
  expect(screen.queryByText('Diagnostic de l’application')).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Réglages' }));
  await userEvent.click(screen.getByText('Diagnostic de l’application'));
  expect(screen.getByText(/Installez xdotool/)).toBeVisible();
  expect(screen.getByText(/Installez xclip/)).toBeVisible();
  expect(screen.getByText(/Branchez le microphone sélectionné/)).toBeVisible();
});
it('keeps edits after failed save and does not prepare a different engine', async () => {
  const base = vi.mocked(invoke).getMockImplementation()!;
  vi.mocked(invoke).mockImplementation((command, args) => command === 'save_settings' ? Promise.reject({ message: 'secret-sentinel', code: 'PROTOCOL_ERROR' }) : base(command, args));
  render(<App />);
  await screen.findByText('Prêt à dicter');
  await userEvent.click(screen.getByRole('button', { name: 'Réglages' }));
  await userEvent.selectOptions(screen.getByLabelText('Langue'), 'en');
  await userEvent.click(screen.getByRole('button', { name: 'Enregistrer et préparer' }));
  expect(await screen.findByRole('alert')).not.toHaveTextContent('secret-sentinel');
  expect(screen.getByLabelText('Langue')).toHaveValue('en');
  expect(invoke).not.toHaveBeenCalledWith('configure_dictation', expect.anything());
});
it('keeps the runtime subscription across navigation, ignores old events, and blocks active settings', async () => {
  render(<App />);
  await screen.findByText('Prêt à dicter');
  act(() => callbacks['dictation://state']({ payload: { ...state(5, 'recording'), recording_ms: 500 } }));
  act(() => callbacks['dictation://state']({ payload: state(4) }));
  expect(screen.getByText('Enregistrement en cours')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Réglages' }));
  expect(screen.getByLabelText('Langue')).toBeDisabled();
  expect(screen.getByRole('button', { name: 'Enregistrer et préparer' })).toBeDisabled();
  await userEvent.click(screen.getByRole('button', { name: 'Dictée' }));
  expect(screen.getByRole('button', { name: 'Annuler' })).toBeEnabled();
  expect(vi.mocked(listen).mock.calls.filter(([name]) => name === 'dictation://state')).toHaveLength(1);
});
it('copies the backend result without sending text and removes it after a purge event', async () => {
  render(<App />);
  await screen.findByText('Prêt à dicter');
  act(() => callbacks['dictation://state']({ payload: { ...state(2, 'success'), text: 'Bonjour privé', delivery: 'fallback' } }));
  expect(screen.getByLabelText('Texte transcrit')).toHaveValue('Bonjour privé');
  await userEvent.click(screen.getByRole('button', { name: 'Copier' }));
  expect(invoke).toHaveBeenCalledWith('copy_dictation_result');
  expect(JSON.stringify(vi.mocked(invoke).mock.calls)).not.toContain('Bonjour privé');
  act(() => callbacks['dictation://state']({ payload: state(4) }));
  expect(screen.queryByLabelText('Texte transcrit')).not.toBeInTheDocument();
});
it('shows corruption without pretending defaults were restored successfully', async () => {
  mockCommands({ get_settings: { settings: defaults, error: 'private-config-path' } });
  render(<App />);
  await screen.findByText('Prêt à dicter');
  await userEvent.click(screen.getByRole('button', { name: 'Réglages' }));
  expect(screen.getByRole('alert')).toHaveTextContent('réglages enregistrés');
  expect(screen.getByRole('alert')).not.toHaveTextContent('private-config-path');
});
it('offers an honest browser preview with disabled settings and no native calls', async () => {
  vi.mocked(isTauri).mockReturnValue(false);
  render(<App />);
  expect(screen.getByText('Aperçu navigateur')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Réglages' }));
  expect(screen.getByLabelText('Langue')).toBeDisabled();
  expect(invoke).not.toHaveBeenCalled();
});
it('cleans up all runtime listeners on unmount', async () => {
  const view = render(<App />);
  await screen.findByText('Prêt à dicter');
  view.unmount();
  expect(stop).toHaveBeenCalledTimes(3);
});
it('shows asynchronous preparation failures inside settings without exposing raw details', async () => {
  render(<App />);
  await screen.findByText('Prêt à dicter');
  await userEvent.click(screen.getByRole('button', { name: 'Réglages' }));
  act(() => callbacks['dictation://state']({ payload: { ...state(3, 'error'), provider: 'groq', engine_ready: false, error_code: 'API_CONFIG_ERROR', message: 'private-key' } }));
  expect(screen.getByRole('alert')).toHaveTextContent('Configuration Groq absente');
  expect(screen.queryByText('private-key')).not.toBeInTheDocument();
});
it('cleans up listeners that resolve after unmount without fetching native state', async () => {
  const resolves: ((value: () => void) => void)[] = [];
  vi.mocked(listen).mockImplementation(() => new Promise(resolve => resolves.push(resolve)));
  const view = render(<App />);
  view.unmount();
  await act(async () => resolves.forEach(resolve => resolve(stop)));
  expect(stop).toHaveBeenCalledTimes(3);
  expect(invoke).not.toHaveBeenCalled();
});
it('can retry incomplete bootstrap even when a runtime event already arrived', async () => {
  const base = vi.mocked(invoke).getMockImplementation()!;
  let fail = true;
  vi.mocked(invoke).mockImplementation((command, args) => {
    if (command === 'get_settings' && fail) { fail = false; return Promise.reject(new Error('private-path')); }
    return base(command, args);
  });
  render(<App />);
  await screen.findByRole('alert');
  act(() => callbacks['dictation://state']({ payload: state(4) }));
  await userEvent.click(screen.getByRole('button', { name: 'Réessayer' }));
  await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
  await userEvent.click(screen.getByRole('button', { name: 'Réglages' }));
  expect(screen.getByLabelText('Langue')).toBeEnabled();
});
