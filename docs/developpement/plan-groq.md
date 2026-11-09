# Intégration Groq — 4 novembre 2026

Demande : configurer une clé API et un modèle dans un fichier local, puis choisir entre transcription locale et Groq dans l’interface. Cette demande étend explicitement le CDC initial hors ligne. Le mode local reste celui proposé au démarrage ; seul le mode Groq autorise l’envoi de l’audio au fournisseur.

## Contrat commun

- `RuntimeConfig.provider` vaut `local` (valeur par défaut pour compatibilité) ou `groq`. `DictationSnapshot.provider` identifie le moteur réellement préparé, distinct du choix en attente dans l’interface. Aucun secret dans IPC, arguments de processus, réponses, journaux ou interface.
- Le moteur Python accepte `--provider local|groq` (local par défaut). L’argument `--model-path` reste l’identité absolue de la session JSONL ; pour Groq, Rust fournit le répertoire canonique du projet, sans exigence de poids locaux. Cela préserve les messages JSONL existants et leur contrôle d’identité. Cette identité n’est pas le nom du modèle distant.
- Python lit `.env` à la racine du projet, uniquement pour Groq ; `GROQ_API_KEY` et `GROQ_MODEL`. Variables d’environnement prioritaires. Parser de valeurs simples, éventuellement entre guillemets, sans exécution ni interpolation. Fournir `.env.exemple` versionné ; `.env` ignoré. Le modèle proposé est `whisper-large-v3` ; `whisper-large-v3-turbo` est aussi pris en charge. Une nouvelle préparation recharge la configuration.
- Endpoint fixe : `https://api.groq.com/openai/v1/audio/transcriptions`, multipart WAV, modèle configuré, `verbose_json`, langue fr/en ou détection automatique sans champ language. TLS vérifié, aucun redirect ni retry automatique, délai réseau borné et réponse limitée. Utiliser la bibliothèque standard Python pour éviter une dépendance réseau supplémentaire. Les erreurs ne révèlent ni clé, ni réponse distante, ni transcription, ni chemin personnel.
- Codes d’erreur ajoutés : `API_CONFIG_ERROR`, `API_AUTH_ERROR`, `API_RATE_LIMIT`, `API_UNAVAILABLE`. Délai réseau : `ENGINE_TIMEOUT`. Réponse vide/sans parole : `NO_SPEECH`. Protocole résultat inchangé : uniquement fr/en.
- Conserver validation WAV privé/mono16k/PCM16/60s et protection contre liens symboliques, annulation et effacement. Le mode API ne charge aucun poids Whisper local. Réutiliser le VAD local fourni par faster-whisper pour ne pas envoyer un silence détecté ; désactiver la télémétrie avant import natif. Annuler interrompt la session côté app et ignore les résultats tardifs ; un audio déjà reçu par Groq ne peut pas être rappelé.

## Tâches

1. **Moteur Python** : configuration, adaptateur Groq, extraction de validation audio commune, CLI et nouveaux codes Python. Tests sans API réelle : multipart/langue, config absente, statut HTTP, timeout, réponse invalide, annulation, audio privé et silence. Possède `engine/**` et tests Python. Pas de changement aux verrous de dépendances nécessaire.
2. **Contrôleur Rust** : provider typé, propagation au processus, snapshot actif, absence d’exigence de poids pour Groq, codes d’erreur et schéma JSON partagé. Possède `src-tauri/**`, `contracts/**`. Tests de sélection locale/Groq et maintien des protections existantes.
3. **Interface** : sélecteur natif Local/Groq ; modèle et chemin locaux conditionnels ; indication visible d’envoi audio pour Groq, aide `.env`, aucun champ clé. État actif distinct du choix, blocage pendant travail, erreurs actionnables. Possède `src/features/dictation/**`, `src/shared/ipc/runtime.ts`, `DESIGN.md`, `UX-CONTRACT.md`.
4. **Intégration et recette** : `.env.exemple`, `.gitignore`, documentation utilisateur/CDC, vérifications Python/React/Rust adaptées et revue indépendante. Aucune clé fournie : pas de requête réelle Groq ni capture automatique du microphone.

## Suivi

Préparation : documentation officielle Groq lue avant code. L’architecture réutilise le superviseur et le contrôleur ; aucun microservice supplémentaire. Le dépôt Git reste indisponible : travailler dans le workspace et conserver les rapports dans ce dossier. Les agents ont des fichiers distincts, avec les interfaces ci-dessus comme contrat.

- [x] Moteur Python et tests sans fournisseur réel.
- [x] Contrôleur et contrats Rust ; formatage, tests, compilation native et Clippy.
- [x] Interface Local/Groq ; tests, typecheck, lint, build et audit strict.
- [x] Configuration, CDC, documentation et revue indépendante ; corrections de texte intégrées.
- [ ] Recette avec une vraie clé Groq et le microphone, à réaliser par l’utilisateur après configuration.

Preuves et contrôles utilisateur : `bilan-groq.md`, rapports `groq-*-report.md`. Aucun appel réel API ni capture automatique pendant l’implémentation.
