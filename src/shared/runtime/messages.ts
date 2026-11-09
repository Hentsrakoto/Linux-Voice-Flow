import type { TranscriptionProvider } from '../ipc/runtime';
const errors: Record<string, string> = {
  NO_SPEECH: 'Aucune parole détectée. Vérifiez le microphone et réessayez.',
  MODEL_NOT_FOUND: 'Modèle local introuvable. Choisissez un modèle installé dans les réglages.',
  MODEL_LOAD_FAILED: 'Le modèle local ne peut pas être chargé. Vérifiez ses fichiers dans les réglages.',
  MIC_UNAVAILABLE: 'Microphone indisponible. Actualisez les microphones dans les réglages.',
  ENGINE_BUSY: 'Une dictée est en cours. Attendez sa fin ou annulez-la.',
  ENGINE_TIMEOUT: 'Le moteur n’a pas répondu à temps. Préparez-le à nouveau dans les réglages.',
  ENGINE_CRASHED: 'Le moteur s’est arrêté. Préparez-le à nouveau dans les réglages.',
  PROTOCOL_ERROR: 'L’action ne peut pas être effectuée. Vérifiez les réglages et réessayez.',
  API_CONFIG_ERROR: 'Configuration Groq absente ou invalide. Vérifiez GROQ_API_KEY et GROQ_MODEL dans .env, puis préparez le moteur à nouveau.',
  API_AUTH_ERROR: 'Clé Groq refusée. Vérifiez GROQ_API_KEY dans .env, puis préparez le moteur à nouveau.',
  API_RATE_LIMIT: 'Quota Groq atteint. Attendez, puis préparez le moteur à nouveau avant de réessayer.',
  API_UNAVAILABLE: 'Groq est indisponible. Vérifiez la connexion et le modèle dans .env, puis préparez le moteur à nouveau.',
  FOCUS_CHANGED: 'La fenêtre ciblée a changé. Le texte reste disponible ici ; copiez-le pour le coller au bon endroit.',
  INJECTION_FAILED: 'L’insertion n’a pas pu être confirmée. Vérifiez le champ cible avant de coller le texte, pour éviter un doublon.',
  CLIPBOARD_FAILED: 'La copie a échoué. Réessayez ou sélectionnez le texte pour le copier.',
  CANCELLED: 'Dictée annulée.',
};
export function errorMessage(code: string | null | undefined, provider: TranscriptionProvider = 'local'): string {
  if (provider === 'groq' && code === 'ENGINE_TIMEOUT') return 'Groq n’a pas répondu à temps. Vérifiez la connexion, puis préparez le moteur à nouveau.';
  return code && Object.hasOwn(errors, code) ? errors[code] : 'L’action a échoué. Vérifiez les réglages et réessayez.';
}
export function commandError(reason: unknown, provider?: TranscriptionProvider): string {
  if (typeof reason === 'object' && reason !== null) {
    const code = 'code' in reason ? reason.code : 'error_code' in reason ? reason.error_code : null;
    if (typeof code === 'string') return errorMessage(code, provider);
  }
  return 'L’action a échoué. Vos réglages sont conservés ; vérifiez la configuration et réessayez.';
}
