import { useEffect, useRef, useState } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { Icon } from '@iconify/react/offline';
import mic from '@iconify-icons/lucide/mic';
import settings from '@iconify-icons/lucide/settings-2';
import DictationPanel from '../features/dictation/DictationPanel';
import SettingsPanel from '../features/settings/SettingsPanel';
import { useVoiceFlow } from '../shared/runtime/useVoiceFlow';
import type { Settings } from '../shared/ipc/runtime';
export default function App() {
  const [view, setView] = useState<'dictation' | 'settings'>('dictation');
  const [settingsDraft, setSettingsDraft] = useState<Settings | null>(null);
  const main = useRef<HTMLElement>(null);
  const previousView = useRef(view);
  useEffect(() => {
    if (previousView.current !== view) {
      main.current?.focus();
      previousView.current = view;
    }
  }, [view]);
  const [native] = useState(isTauri);
  const flow = useVoiceFlow(native);
  return <div className="app-shell">
    <a className="skip-link" href="#main">Aller au contenu</a>
    <header className="app-header flex items-center justify-between gap-5 flex-wrap">
      <a className="wordmark" href="#main" onClick={() => setView('dictation')}><img className="brand-mark" src="/brand/voice-flow-mark.svg" alt="" width={40} height={40} aria-hidden="true"/><span>Voice Flow<small>POUR LINUX</small></span></a>
      <nav className="main-navigation flex items-center gap-1" aria-label="Navigation principale">
        <button className="nav-button" aria-current={view === 'dictation' ? 'page' : undefined} onClick={() => setView('dictation')}><Icon icon={mic} width={17} aria-hidden="true"/>Dictée</button>
        <button className="nav-button" aria-current={view === 'settings' ? 'page' : undefined} onClick={() => setView('settings')}><Icon icon={settings} width={17} aria-hidden="true"/>Réglages</button>
      </nav>
    </header>
    <main id="main" ref={main} tabIndex={-1}>
      {flow.error && <div role="alert" className="app-alert"><p>{flow.error}</p>{(!flow.info || !flow.settingsState || !flow.snapshot || !flow.hotkey) && <button className="button" onClick={flow.retry}>Réessayer</button>}</div>}
      {view === 'dictation' ? <DictationPanel flow={flow} openSettings={() => setView('settings')}/> : <SettingsPanel flow={flow} editedSettings={settingsDraft} setDraft={setSettingsDraft}/>}
    </main>
    <footer className="flex items-center justify-between gap-4 flex-wrap"><span>Voice Flow{flow.info?.version ? ` · ${flow.info.version}` : ''}</span><span>{native ? 'Aucun historique de transcription.' : 'Aperçu de l’interface · fonctions natives désactivées.'}</span></footer>
  </div>;
}
