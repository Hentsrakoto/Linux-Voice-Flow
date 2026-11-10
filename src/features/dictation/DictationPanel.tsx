import { Icon } from '@iconify/react/offline';
import mic from '@iconify-icons/lucide/mic';
import clipboard from '@iconify-icons/lucide/clipboard';
import check from '@iconify-icons/lucide/check';
import trash from '@iconify-icons/lucide/trash-2';
import settings from '@iconify-icons/lucide/settings-2';
import fileText from '@iconify-icons/lucide/file-text';
import MicrophoneActivity from './MicrophoneActivity';
import ShortcutKeys from '../../shared/ui/ShortcutKeys';
import { errorMessage } from '../../shared/runtime/messages';
import type { VoiceFlow } from '../../shared/runtime/useVoiceFlow';
import type { DictationSnapshot } from '../../shared/ipc/runtime';
const titles: Record<DictationSnapshot['phase'], string> = {
  unconfigured: 'Configuration requise', loading_engine: 'Préparation du moteur…', ready: 'Prêt à dicter', recording: 'Enregistrement en cours', transcribing: 'Transcription en cours…', inserting: 'Insertion en cours…', cancelling: 'Annulation en cours…', success: 'Transcription terminée', error: 'Dictée interrompue',
};
export default function DictationPanel({ flow, openSettings }: { flow: VoiceFlow; openSettings: () => void }) {
  const { snapshot, native, hotkey, busy, pending, notice } = flow;
  const provider = snapshot?.provider ?? 'local';
  const active = snapshot && ['loading_engine', 'recording', 'transcribing', 'inserting', 'cancelling'].includes(snapshot.phase);
  const status = !native ? 'Aperçu navigateur' : snapshot ? titles[snapshot.phase] : 'Vérification de la dictée…';
  const shortcut = !native ? 'Ctrl+Alt+Space' : hotkey?.shortcut;
  const hasResult = snapshot?.text != null;
  const delivery = snapshot?.delivery ?? 'preview';
  const preparationHelp = hotkey?.press_count ? 'Raccourci reçu. Préparez le moteur dans Réglages pour démarrer la dictée.' : 'Préparez le moteur dans Réglages pour activer la dictée.';
  return <div className="dictation-workspace">
    <div className="page-heading dictation-heading"><h1>Dictée</h1><p className="page-description">Placez le curseur dans votre application, puis maintenez votre raccourci.</p></div>
    <section className="shortcut-panel" aria-labelledby="dictation-state">
      <div className="state-label"><span className={`state-dot${snapshot?.engine_ready ? ' is-ready' : ''}`} aria-hidden="true"/><h2 id="dictation-state" aria-live="polite">{status}</h2></div>
      <div className="dictation-trigger">
        <MicrophoneActivity native={native} phase={snapshot?.phase} shortcutHeld={!!hotkey?.held}/>
        {shortcut ? <ShortcutKeys shortcut={shortcut}/> : <p className="shortcut-unavailable">{flow.loading ? 'Vérification du raccourci…' : hotkey ? 'Aucun raccourci actif' : 'Raccourci non vérifié'}</p>}
      </div>
      {shortcut && <p className="shortcut-instruction">{native && !snapshot?.engine_ready && !active ? preparationHelp : <>Maintenez pour parler. Relâchez pour transcrire{snapshot?.auto_insert_enabled !== false ? ' et insérer' : ''}.</>}</p>}
      {!native && <p className="field-help">Ouvrez l’application desktop pour dicter. Aucun microphone ni raccourci global n’est actif dans cet aperçu.</p>}
      {native && hotkey && !hotkey.shortcut && !flow.loading && <p role="alert">Le raccourci n’est pas disponible. Vérifiez sa configuration dans les réglages.</p>}
      {snapshot?.engine_ready && <p className="service-indicator">{provider === 'groq' ? 'Groq · audio envoyé au service' : 'Local · audio traité sur cet ordinateur'}</p>}
      {snapshot?.phase === 'recording' && <p className="field-help">Microphone actif · {(snapshot.recording_ms / 1000).toFixed(1)} s / 60 s</p>}
      {snapshot?.phase === 'ready' && snapshot.error_code === 'NO_SPEECH' && <p role="status">{errorMessage('NO_SPEECH')}</p>}
      {snapshot?.phase === 'error' && <p role="alert" className="inline-message">{snapshot.reason === 'recording_limit' ? 'Limite de 60 secondes atteinte. Audio abandonné ; dictez un passage plus court.' : errorMessage(snapshot.error_code, provider)}</p>}
      {notice && <p role="status" className="inline-message">{notice}</p>}
      <div className="flex items-center gap-3 flex-wrap">
        {active && <button type="button" className="button button-primary" disabled={snapshot.phase === 'cancelling'} onClick={() => { void flow.run('cancel_dictation'); }}>Annuler</button>}
        {native && (!snapshot?.engine_ready || snapshot.phase === 'error') && !active && <button type="button" className="button button-primary" onClick={openSettings}><Icon icon={settings} width={17} aria-hidden="true"/>Ouvrir les réglages</button>}
      </div>
    </section>
    <section className="result-panel" aria-labelledby="result-title">
      <div className="result-heading"><h2 id="result-title"><Icon icon={fileText} width={18} aria-hidden="true"/>Votre transcription</h2>{hasResult && <span className="result-retention">Éphémère · 120 s</span>}</div>
      {hasResult ? <>
        <label htmlFor="transcribed-text" className="sr-only">Texte transcrit</label>
        <textarea className="resize-none transcription-text" id="transcribed-text" readOnly value={snapshot.text ?? ''} rows={6}/>
        <p className="delivery-message" role="status">{delivery === 'inserted' ? <><Icon icon={check} width={17} aria-hidden="true"/>Texte transmis au champ ciblé.</> : delivery === 'clipboard' ? 'Texte copié. Collez-le dans votre application avec Ctrl + V.' : delivery === 'fallback' ? 'Insertion non confirmée. Vérifiez le champ cible avant de copier et coller pour éviter un doublon.' : 'Texte disponible ici. Copiez-le pour le coller dans votre application.'}</p>
        {(snapshot.insertion_uncertain || ['FOCUS_CHANGED', 'INJECTION_FAILED'].includes(snapshot.error_code ?? '')) && <p role="alert" className="inline-message">L’insertion n’a pas été terminée. Une partie du texte peut déjà être présente dans le champ cible : vérifiez-le avant de coller pour éviter un doublon.</p>}
        <div className="result-actions flex items-center gap-2 flex-wrap">
          <button className="button button-primary" disabled={busy} onClick={() => { void flow.run('copy_dictation_result'); }}><Icon icon={clipboard} width={16} aria-hidden="true"/>Copier</button>
          <button className="button button-quiet" disabled={pending || busy} onClick={() => { void flow.run('clear_dictation_result'); }}><Icon icon={trash} width={16} aria-hidden="true"/>Effacer</button>
        </div>
        <p className="field-help">Le texte est effacé de cette vue après 120 secondes. Aucun message n’est envoyé automatiquement dans votre chat.</p>
      </> : <div className="result-empty"><Icon icon={mic} width={26} aria-hidden="true"/><p>Votre prochaine phrase apparaîtra ici.</p><span>La capture démarre uniquement avec le raccourci, une fois le moteur prêt.</span></div>}
      {snapshot?.clipboard_owned && <button className="button button-quiet" disabled={busy} onClick={() => { void flow.run('clear_dictation_clipboard'); }}>Effacer ma copie du presse-papiers</button>}
    </section>
  </div>;
}
