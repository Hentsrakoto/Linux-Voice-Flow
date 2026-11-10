import { Icon } from '@iconify/react/offline';
import mic from '@iconify-icons/lucide/mic';
import loader from '@iconify-icons/lucide/loader-circle';
import type { DictationSnapshot } from '../../shared/ipc/runtime';

type Props = {
  native: boolean;
  phase: DictationSnapshot['phase'] | undefined;
  shortcutHeld: boolean;
};

export default function MicrophoneActivity({ native, phase, shortcutHeld }: Props) {
  const recording = native && phase === 'recording';
  const processing = native && !!phase && ['loading_engine', 'transcribing', 'inserting', 'cancelling'].includes(phase);
  const received = native && shortcutHeld && !recording && !processing;
  const state = recording ? 'recording' : processing ? 'processing' : received ? 'received' : 'idle';
  const label = recording ? 'Micro actif' : processing ? phase === 'loading_engine' ? 'Préparation…' : phase === 'cancelling' ? 'Arrêt en cours…' : 'Traitement…' : received ? 'Raccourci reçu' : 'Micro inactif';
  const description = recording ? 'Microphone actif : enregistrement en cours' : processing ? 'Microphone inactif : traitement en cours' : received ? 'Raccourci reçu, en attente du démarrage du microphone' : 'Microphone inactif';

  return <div className="microphone-activity" data-state={state}>
    <div className="microphone-activity-symbol" role="img" aria-label={description}>
      {recording && <><span className="microphone-halo" aria-hidden="true"/><span className="microphone-halo microphone-halo-delayed" aria-hidden="true"/></>}
      {received && <span className="microphone-acknowledgement" aria-hidden="true"/>}
      <span className="microphone-disc"><Icon icon={processing ? loader : mic} width={28} aria-hidden="true"/></span>
    </div>
    <span className="microphone-activity-label" aria-hidden="true">{label}</span>
  </div>;
}
