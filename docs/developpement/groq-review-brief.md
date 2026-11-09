# Revue indépendante Groq

Demande et interfaces : `docs/developpement/plan-groq.md`. Examiner les sources finales et les rapports `groq-python-report.md`, `groq-rust-report.md`, `groq-ui-report.md`.

Pas de dépôt Git utilisable dans ce workspace : pas de diff ou historique fiable. La revue doit donc lire les nouvelles unités complètes et les interfaces modifiées, sans prétendre être une revue d’un commit.

Sources principales : `engine/groq_config.py`, `engine/groq_transcriber.py`, `engine/audio_input.py`, `engine/errors.py`, `engine/transcriber.py`, `engine/main.py`, `engine/runtime.py`, `engine/protocol.py`, `src-tauri/src/engine.rs`, `src-tauri/src/dictation.rs`, `src-tauri/src/protocol.rs`, `src/features/dictation/DictationPanel.tsx`, `src/shared/ipc/runtime.ts`, `contracts/engine-v1.schema.json`. Lire les tests nouveaux pour juger les preuves.

Contrôles ciblés : envoi seulement après préparation Groq, distinction service choisi/actif, configuration backend et absence de clé dans arguments/IPC/logs, WAV privé/VAD sans poids locaux, délai/réponse bornés/TLS/redirections/retries, annulation et résultats tardifs, compatibilité du mode local et des erreurs interlangages. La vraie API et le microphone ne sont pas autorisés pour les tests automatiques de cette revue. Ne pas lire `.env` réel : il est privé ; lire seulement `.env.exemple` et les fixtures de tests.

Rendre un verdict conformité et qualité, avec défauts concrets classés par gravité et chemins/lignes. Ne pas implémenter ni relancer toutes les suites déjà documentées sans un constat précis. Écrire `docs/developpement/groq-review-report.md` et retourner un résumé court. Pas de sous-agents.
