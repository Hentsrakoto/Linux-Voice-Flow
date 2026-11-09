# Revue indépendante Groq — 4 novembre 2026

## Verdict

Conformité fonctionnelle et protections principales satisfaisantes à la lecture des sources finales. Aucun défaut bloquant ni fuite de secret confirmés. Les deux incohérences mineures de messages repérées ont été corrigées et relues ; aucun constat actionnable ne reste ouvert. Ce verdict ne certifie pas une transcription réelle auprès de Groq.

Aucun dépôt Git exploitable : cette revue porte sur les unités et interfaces finales complètes, pas sur un diff ou un commit. Lecture du brief, du plan, des rapports Python/Rust/UI et des tests concernés. Aucune modification du code de production, aucune lecture du `.env` réel, aucune requête distante ni utilisation du microphone par le relecteur.

## Constats corrigés et vérifiés

- **P3 résolu — Message IPC incorrect en mode Groq**, `src-tauri/src/dictation.rs:531`. Après le relâchement du raccourci, le snapshot publie « Transcription locale en cours… » quel que soit le fournisseur. Le panneau actuel ne rend pas ce champ, donc le défaut n'induit pas actuellement son utilisateur en erreur, mais le contrat d'état contient une information inexacte. Correction relue : le message est maintenant « Transcription en cours… ».
- **P3 résolu — Instruction de reprise incomplète**, `src/features/dictation/DictationPanel.tsx:18-19` et `:23`, en relation avec `src-tauri/src/dictation.rs:619-622`. Les erreurs quota, indisponibilité et délai Groq invitent à attendre/vérifier puis réessayer. Le contrôleur arrête pourtant le moteur sur ces erreurs et invalide `engine_ready` ; un nouvel appui sur le raccourci sera ignoré jusqu'à une nouvelle préparation. Correction relue : les trois messages demandent maintenant explicitement de préparer le moteur à nouveau. La stratégie de reprise est inchangée.

## Contrôles réalisés par lecture

- **Consentement et état actif** : Local est initial. Le sélecteur seul ne lance aucune préparation ni transcription. La préparation transmet un fournisseur fermé ; la capture requiert `engine_ready`. Le fournisseur actif est publié par le backend et reste distinct du choix UI. L'interface rappelle le transfert Groq si Local est sélectionné sans nouvelle préparation. Les réglages sont bloqués pendant les opérations.
- **Configuration et secrets** : configuration chargée dans Python uniquement pour Groq, chemin ancré dans le projet, environnement prioritaire sur les valeurs du fichier, parser sans exécution/interpolation, contenu borné, lien final refusé. Clé masquée dans `repr`, absente des arguments et du schéma IPC. Erreurs constantes, diagnostics natifs détournés et stderr drainé. `.env.exemple` ne contient aucune clé ; règles d'ignorance présentes. Une préparation ne valide pas l'authentification distante et les rapports le précisent.
- **Audio et VAD** : validation commune aux deux fournisseurs ; répertoire 0700 possédé, fichier régulier 0600 possédé, openat/O_NOFOLLOW, taille bornée, mono PCM16/16 kHz, maximum 60 secondes et troncature vérifiée. Validation remet le curseur à zéro avant multipart. Le VAD local intervient avant upload et ne charge pas les poids Whisper. Les variables anti-télémétrie sont fixées avant import natif.
- **Transport** : endpoint HTTPS fixe, vérification TLS par défaut urllib, handler refusant les redirections, aucune boucle de retry, nom WAV générique, modèle borné aux deux valeurs prévues. Auto omet la langue, fr/en la transmettent. Réponse limitée à 1 Mio et texte/langue contrôlés. Authentification, quota, indisponibilité et timeout sont traduits sans transmettre le corps distant.
- **Délais** : 45 secondes est un timeout d'opération réseau, pas une échéance totale urllib. L'échéance globale effective est fournie par le superviseur Rust à 120 secondes ; arrêt forcé et attente du processus. Cette distinction est correctement explicitée dans le rapport Python.
- **Annulation et effacement** : vérifications Python avant réseau et après réponse ; publication centralisée privilégiant les commandes déjà reçues ; identité invalidée dans le contrôleur ; résultats tardifs rejetés, arrêt forcé après le budget d'annulation. Le handle WAV est abandonné sur résultats/erreurs/annulation. Le transfert déjà effectué ne peut pas être rappelé.
- **Compatibilité** : fournisseur Rust par défaut local, valeur inconnue refusée ; local conserve sa validation de modèle ; Groq utilise l'identité absolue du projet sans poids. Codes API cohérents entre Python, Rust et schéma. Format JSONL, langue fr/en et contrôles d'identité conservés.

## Preuves et limites

Tests lus : configuration et multipart, statuts HTTP, réponse invalide/trop grande, annulation et silence, protections audio, CLI Groq et pipeline simulé ; sélection/préparation et erreurs UI ; sélection Groq sans poids et retour Local côté Rust. Les tests HTTP remplacent le transport, et le refus des redirections est contrôlé directement sur le handler ; cela ne constitue pas une recette réseau réelle.

Résultats des exécutants, non relancés par cette revue : Python 79 réussis, 1 sauté et 77 sous-tests ; UI 30 tests avec typecheck/lint/build réussis ; Rust headless 45 unitaires et 12 tests de processus, formatage réussi. L'agent principal confirme aussi une exécution fraîche Python et un smoke test Tiny hors ligne sans parole. Le rapport Rust final indique compilation native `cargo check --offline --locked` et `cargo clippy --offline --locked --all-targets -- -D warnings` réussies. Après correction de texte, formatage/check incrémental Rust réussis selon l'agent principal ; la vérification UI finale après correction est également réussie selon celui-ci : typecheck, lint, 30 tests et build. La relecture des corrections est limitée aux messages concernés et confirme les deux résolutions.

Restent à réaliser séparément : recette clavier/WebView native et transcription consentie avec une vraie clé. La suite automatique ne valide ni le compte/quota Groq ni les conditions réelles du réseau et du microphone.
