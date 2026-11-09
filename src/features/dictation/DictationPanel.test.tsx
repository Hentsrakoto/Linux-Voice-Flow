import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import DictationPanel from './DictationPanel';
import type { DictationSnapshot } from '../../shared/ipc/runtime';
import type { VoiceFlow } from '../../shared/runtime/useVoiceFlow';
function runtime(override: Partial<DictationSnapshot> = {}): VoiceFlow {
  return {
    native: true, busy: false, pending: false, loading: false, notice: null,
    hotkey: { revision: 0, shortcut: 'Ctrl+Alt+Space', held: false, supported: true, error: null, press_count: 0, release_count: 0 },
    snapshot: { revision: 1, phase: 'ready', engine_ready: true, provider: 'local', session_id: null, microphone_id: 'default', language: 'fr', recording_ms: 0, audio_duration_ms: null, text: null, error_code: null, message: '', ...override },
    run: vi.fn().mockResolvedValue(undefined),
  } as unknown as VoiceFlow;
}
it.each([
  ['inserted', 'Texte transmis au champ ciblé.'],
  ['clipboard', 'Texte copié. Collez-le dans votre application avec Ctrl + V.'],
  ['preview', 'Texte disponible ici. Copiez-le pour le coller dans votre application.'],
  ['fallback', 'Insertion non confirmée. Vérifiez le champ cible avant de copier et coller pour éviter un doublon.'],
] as const)('distinguishes %s from successful insertion', (delivery, message) => {
  render(<DictationPanel flow={runtime({ phase: 'success', text: 'Bonjour', delivery })} openSettings={vi.fn()}/>);
  expect(screen.getByLabelText('Texte transcrit')).toHaveValue('Bonjour');
  expect(screen.getByRole('status')).toHaveTextContent(message);
});
it('maps focus and API failures without showing private runtime messages', () => {
  render(<DictationPanel flow={runtime({ phase: 'error', provider: 'groq', error_code: 'API_AUTH_ERROR', message: 'private-key-and-path' })} openSettings={vi.fn()}/>);
  expect(screen.getByRole('alert')).toHaveTextContent('Clé Groq refusée');
  expect(screen.queryByText('private-key-and-path')).not.toBeInTheDocument();
});
it('keeps cancellation available during insertion', async () => {
  const flow = runtime({ phase: 'inserting' });
  render(<DictationPanel flow={flow} openSettings={vi.fn()}/>);
  await userEvent.click(screen.getByRole('button', { name: 'Annuler' }));
  expect(flow.run).toHaveBeenCalledWith('cancel_dictation');
});
it('shows the native Escape notice once and clears only the owned clipboard', async () => {
  const flow = runtime({ clipboard_owned: true });
  flow.notice = 'Échap indisponible. Utilisez Annuler.';
  render(<DictationPanel flow={flow} openSettings={vi.fn()}/>);
  expect(screen.getAllByText(flow.notice)).toHaveLength(1);
  await userEvent.click(screen.getByRole('button', { name: 'Effacer ma copie du presse-papiers' }));
  expect(flow.run).toHaveBeenCalledWith('clear_dictation_clipboard');
});
it('never presents the default shortcut as active when registration is absent', () => {
  const flow = runtime();
  flow.hotkey = { ...flow.hotkey!, shortcut: null };
  render(<DictationPanel flow={flow} openSettings={vi.fn()}/>);
  expect(screen.getByText('Aucun raccourci actif')).toBeInTheDocument();
  expect(screen.queryByText('Espace')).not.toBeInTheDocument();
});
it('retains the partial insertion warning after fallback copying', () => {
  render(<DictationPanel flow={runtime({ phase: 'success', text: 'Bonjour', delivery: 'clipboard', error_code: 'CLIPBOARD_FAILED', insertion_uncertain: true, clipboard_owned: true })} openSettings={vi.fn()}/>);
  expect(screen.getByRole('status')).toHaveTextContent('Texte copié');
  expect(screen.getByRole('alert')).toHaveTextContent('Une partie du texte peut déjà être présente');
});
