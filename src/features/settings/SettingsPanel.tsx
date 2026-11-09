import { useState } from 'react';
import type { KeyboardEvent } from 'react';
import { Icon } from '@iconify/react/offline';
import keyboard from '@iconify-icons/lucide/keyboard';
import refresh from '@iconify-icons/lucide/refresh-cw';
import check from '@iconify-icons/lucide/check';
import type { Settings } from '../../shared/ipc/runtime';
import { defaultSettings } from '../../shared/ipc/runtime';
import type { VoiceFlow } from '../../shared/runtime/useVoiceFlow';
import { errorMessage } from '../../shared/runtime/messages';

export default function SettingsPanel({ flow }: { flow: VoiceFlow }) {
  const [editedSettings, setDraft] = useState<Settings | null>(null);
  const draft = editedSettings ?? flow.settingsState?.settings ?? defaultSettings;
  const [custom, setCustom] = useState(false);
  const [saved, setSaved] = useState(false);
  const [validation, setValidation] = useState<string | null>(null);
  const [capturing, setCapturing] = useState(false);
  const disabled = !flow.native || flow.busy || flow.loading || !flow.settingsState;
  const installed = flow.models.some(model => model.path === draft.model_path);
  const modelChoice = custom || !!draft.model_path && !installed ? 'custom' : draft.model_path;
  function change<K extends keyof Settings>(key: K, value: Settings[K]) { setSaved(false); setValidation(null); setDraft(current => ({ ...(current ?? flow.settingsState?.settings ?? defaultSettings), [key]: value })); }
  function capture(event: KeyboardEvent<HTMLButtonElement>) {
    if (!capturing) return;
    event.preventDefault();
    if (event.key === 'Escape') { setCapturing(false); return; }
    if (['Control', 'Alt', 'Shift', 'Meta'].includes(event.key)) return;
    if (!event.ctrlKey && !event.altKey && !event.metaKey) { setValidation('Associez une touche à Ctrl, Alt ou Super.'); return; }
    const keys = [event.ctrlKey && 'Ctrl', event.altKey && 'Alt', event.shiftKey && 'Shift', event.metaKey && 'Super', event.code.replace(/^Key|^Digit/, '')].filter(Boolean);
    change('shortcut', keys.join('+')); setCapturing(false);
  }
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (disabled) return;
    if (draft.provider === 'local' && !draft.model_path.trim().startsWith('/')) { setValidation('Choisissez un modèle installé ou saisissez son chemin absolu.'); return; }
    if (!draft.shortcut.trim()) { setValidation('Saisissez un raccourci.'); return; }
    const result = await flow.save({ ...draft, model_path: draft.model_path.trim(), shortcut: draft.shortcut.trim() });
    if (result) { setDraft(null); setSaved(true); }
  }
  return <div className="settings-workspace">
    <div className="page-heading"><p className="eyebrow">À votre façon</p><h1>Réglages</h1><p className="page-description">Un seul endroit pour préparer votre dictée. Vos choix sont conservés au redémarrage.</p></div>
    {!flow.native && <p className="inline-message">Aperçu des réglages. Ouvrez l’application desktop pour les modifier.</p>}
    {flow.settingsState?.error && <p role="alert" className="inline-message">Les réglages enregistrés ne peuvent pas être lus. Vérifiez les valeurs ci-dessous : Enregistrer et préparer remplacera ce fichier par vos nouveaux réglages.</p>}
    {flow.busy && <p role="status" className="inline-message">Une opération est en cours. Les réglages pourront être modifiés après sa fin.</p>}
    {flow.snapshot && ['loading_engine', 'recording', 'transcribing', 'inserting', 'cancelling'].includes(flow.snapshot.phase) && <button type="button" className="button" disabled={flow.snapshot.phase === 'cancelling'} onClick={() => { void flow.run('cancel_dictation'); }}>Annuler</button>}
    {flow.snapshot?.phase === 'error' && <p role="alert" className="inline-message">{errorMessage(flow.snapshot.error_code, flow.snapshot.provider)}</p>}
    <form onSubmit={event => { void submit(event); }} noValidate aria-busy={flow.pending}>
      <fieldset className="settings-section" disabled={disabled}><legend>Transcription</legend>
        <label htmlFor="provider">Service de transcription</label>
        <select id="provider" value={draft.provider} onChange={event => change('provider', event.target.value as Settings['provider'])}><option value="local">Local — sur cet ordinateur</option><option value="groq">Groq — API</option></select>
        {draft.provider === 'groq' ? <div className="service-notice"><p>L’audio sera envoyé à Groq pour transcription. Une connexion Internet et votre clé API sont nécessaires.</p><p className="field-help">Renseignez GROQ_API_KEY et GROQ_MODEL dans .env à la racine du projet. Votre clé reste dans le moteur et n’est jamais affichée ici.</p></div> : <>
          <label htmlFor="model-choice">Modèle de transcription</label>
          <select id="model-choice" value={modelChoice} onChange={event => { if (event.target.value === 'custom') { setCustom(true); setSaved(false); } else { setCustom(false); change('model_path', event.target.value); } }}>
            <option value="">Choisir un modèle installé</option>{flow.models.map(model => <option key={model.id} value={model.path}>{model.name}</option>)}<option value="custom">Dossier personnalisé</option>
          </select>
          {modelChoice === 'custom' && <><label htmlFor="model-path">Dossier du modèle local</label><input id="model-path" value={draft.model_path} onChange={event => change('model_path', event.target.value)} autoComplete="off" spellCheck={false}/><p className="field-help">Chemin absolu du modèle déjà présent sur cet ordinateur.</p></>}
          {flow.models.find(model => model.path === draft.model_path)?.description && <p className="field-help">{flow.models.find(model => model.path === draft.model_path)?.description}</p>}
          {flow.models.length === 0 && <p className="field-help">Aucun modèle installé détecté. Utilisez l’installateur du projet ou un dossier personnalisé.</p>}
        </>}
        <div className="settings-grid"><div><label htmlFor="language">Langue</label><select id="language" value={draft.language} onChange={event => change('language', event.target.value as Settings['language'])}><option value="fr">Français</option><option value="en">Anglais</option><option value="auto">Détection automatique</option></select></div><div><label htmlFor="microphone">Microphone</label><select id="microphone" value={draft.microphone_id} onChange={event => change('microphone_id', event.target.value)}>{!flow.microphones.some(device => device.id === draft.microphone_id) && <option value={draft.microphone_id}>{draft.microphone_id === 'default' ? 'Microphone par défaut' : 'Microphone enregistré indisponible'}</option>}{flow.microphones.map(device => <option key={device.id} value={device.id}>{device.name}</option>)}</select></div></div>
        <button className="button button-quiet" type="button" onClick={() => { void flow.refreshLists(); }}><Icon icon={refresh} width={15} aria-hidden="true"/>Actualiser les appareils et modèles</button>
        {flow.listError && <p role="alert">Une liste n’a pas pu être chargée. Actualisez les appareils et modèles pour réessayer.</p>}
      </fieldset>
      <fieldset className="settings-section" disabled={disabled}><legend>Raccourci</legend>
        <label htmlFor="shortcut">Raccourci de dictée</label><div className="shortcut-editor"><input id="shortcut" value={draft.shortcut} onChange={event => change('shortcut', event.target.value)} spellCheck={false} autoComplete="off"/><button type="button" className="button" aria-pressed={capturing} onClick={() => setCapturing(value => !value)} onKeyDown={capture} onBlur={() => setCapturing(false)}><Icon icon={keyboard} width={17} aria-hidden="true"/>{capturing ? 'Appuyez sur les touches…' : 'Saisir au clavier'}</button></div>
        <p className="field-help">Maintenir pour parler, relâcher pour transcrire. Associez une touche à Ctrl, Alt ou Super. Échap est réservé à l’annulation.</p>
        {flow.hotkey?.shortcut && <p className="field-help">Raccourci actif : <code>{flow.hotkey.shortcut}</code></p>}
      </fieldset>
      <fieldset className="settings-section" disabled={disabled}><legend>Insertion et confidentialité</legend>
        <label className="check-row"><input type="checkbox" checked={draft.auto_insert_enabled} onChange={event => change('auto_insert_enabled', event.target.checked)}/><span>Insérer automatiquement dans le champ ciblé<small>Seulement si la fenêtre et le focus n’ont pas changé. Aucun message de chat n’est envoyé.</small></span></label>
        <label className="check-row"><input type="checkbox" checked={draft.clipboard_auto_copy_consent} onChange={event => change('clipboard_auto_copy_consent', event.target.checked)}/><span>Copier automatiquement si l’insertion échoue<small>J’autorise le remplacement du contenu du presse-papiers. Sinon, le bouton Copier reste une action explicite.</small></span></label>
        <p className="field-help">X11 ne détecte pas toujours un changement de curseur dans la même fenêtre. Désactivez l’insertion automatique si vous préférez coller vous-même.</p>
        <p className="field-help">Durée maximale : 60 secondes. Texte affiché pendant 120 secondes, sans historique.</p>
      </fieldset>
      {validation && <p role="alert" className="inline-message">{validation}</p>}
      {saved && <p role="status" className="saved-message"><Icon icon={check} width={17} aria-hidden="true"/>Réglages enregistrés.{flow.snapshot?.engine_ready ? ' Le moteur est prêt.' : ''}</p>}
      <div className="settings-save"><button type="submit" className="button button-primary" disabled={disabled}>{flow.pending ? 'Enregistrement…' : 'Enregistrer et préparer'}</button><p className="field-help">Le nouveau service est appliqué après préparation. Aucun microphone ne démarre pendant l’enregistrement des réglages.</p></div>
    </form>
    <details className="diagnostics"><summary>Diagnostic de l’application</summary><div className="diagnostics-content">
      {flow.diagnostics ? <dl>{([['x11_supported', 'Session X11'], ['microphone_available', 'Microphone'], ['model_available', 'Modèle ou configuration'], ['engine_ready', 'Moteur prêt'], ['xdotool_available', 'Insertion X11'], ['xclip_available', 'Presse-papiers']] as const).map(([key, label]) => <div key={key}><dt>{label}</dt><dd>{flow.diagnostics?.[key] ? 'Disponible' : 'À vérifier'}</dd></div>)}</dl> : <p>{flow.native ? 'Diagnostic en cours de vérification…' : 'Diagnostic disponible dans l’application desktop.'}</p>}
      {flow.diagnosticError && <p role="alert">Le diagnostic n’a pas pu être chargé. Réessayez.</p>}
      <button className="button button-quiet" disabled={!flow.native || flow.pending} onClick={() => { void flow.refreshDiagnostics(); }}>Actualiser le diagnostic</button>
    </div></details>
  </div>;
}
