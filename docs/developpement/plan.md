# Linux Voice Flow — Plan de développement

> **Consigne pour l’agent :** suivre ce plan tâche par tâche, avec le skill `superpowers:executing-plans` lors de l’implémentation. Cocher une étape uniquement après vérification et consigner les résultats. Lire également le cahier des charges avant de reprendre le travail.

**Objectif :** livrer une application de dictée locale sur Linux Mint Cinnamon/X11 : maintenir un raccourci, parler, relâcher, puis insérer la transcription dans l’application ciblée ou proposer un collage manuel.

**Architecture :** React présente les états et réglages. Rust possède la session, le microphone, le raccourci, le focus X11, l’insertion et le processus enfant Python. Python charge un modèle local faster-whisper et applique la VAD ; les échanges Rust/Python utilisent JSONL v1.

**Stack imposée :** Tauri 2, React, TypeScript, Tailwind CSS, Rust, Python, faster-whisper, X11.

**Référence :** [CAHIER_DES_CHARGES_LINUX_VOICE_FLOW.md](../../CAHIER_DES_CHARGES_LINUX_VOICE_FLOW.md), version 1.0 du 3 novembre 2026. Le cahier des charges prévaut en cas de divergence ; une modification de périmètre doit être explicite.

**État initial avant implémentation :** seul le cahier des charges est présent comme source du produit. Ce plan ne constate aucune fonctionnalité implémentée. Les chemins de code ci-dessous sont à créer.

## 1. Règles de travail et contraintes globales

- Traiter un lot cohérent à la fois et respecter ses dépendances. Ne pas commencer par une interface complète avant d’avoir levé les risques desktop.
- Conserver une architecture compréhensible pour un étudiant M1 : modules courts, contrats explicites, documentation des choix Rust/Python et de leurs échanges avec React.
- Pour une logique critique : écrire le test comportemental, constater son échec, implémenter, puis vérifier son succès. Les tests matériels nécessitent aussi une recette réelle.
- Aucun réseau pendant la capture, la transcription et l’insertion. Préparer explicitement le modèle avant usage ; aucun téléchargement silencieux.
- Aucun compte, cloud, historique persistant, écoute permanente ou réécriture par modèle génératif. Wayland et F-15 restent hors MVP.
- Une seule dictée active. Ignorer la répétition de touche et tout nouveau déclenchement pendant le traitement.
- Capturer seulement pendant l’appui. Limite initiale : **60 secondes** ; atteindre cette limite arrête, nettoie et informe sans insertion.
- Mémoriser la cible au début ; vérifier sa présence et son focus immédiatement avant insertion. Toute incertitude déclenche le fallback.
- Le fallback disponible n’autorise pas une copie automatique : celle-ci exige un réglage de consentement distinct, désactivé par défaut.
- JSONL : version **1**, ligne limitée à **1 Mio**, démarrage **60 s**, transcription **120 s**, confirmation d’annulation **3 s**. Après timeout, invalider la requête avant toute récupération.
- Audio : tampon mémoire puis WAV temporaire privé ; répertoire **0700**, fichiers **0600**. Nettoyer après succès, erreur, annulation, timeout, fermeture et au redémarrage après crash.
- Aucun audio, texte dicté ou contenu du presse-papiers dans les logs. Les erreurs affichées ne doivent pas recopier les données sensibles reçues du moteur.
- Réactivité visuelle appui/relâchement : **300 ms maximum** hors charge exceptionnelle, bloquant en recette. Objectif de transcription : audio de **10 s** traité en **moins de 15 s** après relâchement ; mesurer et expliquer tout écart.
- À la fin de chaque tâche, mettre à jour ce fichier : fichiers touchés, exigences, commandes réellement exécutées, résultats, limites et prochaine étape. Les commandes prévues ci-dessous ne sont pas des vérifications déjà exécutées.
- Créer des commits cohérents uniquement si un dépôt Git utilisable est disponible et si cela fait partie du travail demandé ; aucun push ou déploiement implicite.

## 2. Décisions de préparation

Ces décisions précisent l’exécution du cahier des charges ; elles ne décrivent pas des choix supplémentaires validés par l’utilisateur.

| Sujet | Choix de départ et validation |
|---|---|
| Gestion du projet | npm pour React ; Cargo pour Rust ; environnement Python `.venv` avec dépendances verrouillées. Fixer les versions compatibles au lot 1 et documenter les versions utilisées. |
| Capture | Évaluer `cpal` et le backend audio réellement disponible. Le lot 6 doit prouver l’énumération, la capture, la conversion mono et le rééchantillonnage avant intégration. |
| Raccourci | Évaluer le plugin global-shortcut Tauri 2 ; retenir seulement une solution prouvant les événements distincts appui/relâchement sur Cinnamon/X11. Un remplacement X11 exige une décision documentée. |
| Insertion | Évaluer une API X11/XTest pour Unicode, sans détour automatique par le presse-papiers. Si l’injection Unicode fiable n’est pas possible, documenter la limite et proposer le fallback consenti. |
| Modèle | Import manuel d’un répertoire local pour le premier MVP ; profil CPU/quantification retenu après mesure matérielle. Ne pas choisir un modèle par sa seule taille. |
| Configuration | Fichier versionné dans le répertoire de configuration utilisateur résolu par Tauri ; écriture atomique et permissions privées. Documenter le chemin réel sur Mint. |
| Confidentialité UI | Proposition : purger un résultat de fallback après 120 s, à la fermeture de l’application ou sur action « Effacer ». Afficher le délai ; aucune suppression silencieuse du presse-papiers système. |

Les API et versions exactes des bibliothèques seront vérifiées dans leur documentation officielle avant installation. Les choix de capture, raccourci et insertion restent soumis aux prototypes, sans changer les interfaces métier.

## 3. Fichiers et responsabilités prévus

```text
src/
  app/App.tsx                       vue principale et navigation minimale
  features/dictation/                état visible, commandes et résultat éphémère
  features/settings/                 formulaire et validation de présentation
  features/diagnostics/               diagnostic local
  shared/ipc/                       types et adaptation des événements Tauri
src-tauri/
  capabilities/default.json          permissions minimales de la WebView
  src/lib.rs                        montage des services et cycle de fermeture
  src/commands.rs                   commandes UI validées côté Rust
  src/app_state.rs                  transitions et garde des sessions
  src/controller.rs                 orchestration de la dictée
  src/hotkey.rs                     raccourci et anti-répétition
  src/audio.rs                      capture, normalisation et limite
  src/temp_audio.rs                 WAV privé et nettoyage
  src/protocol.rs                   messages JSONL v1 et erreurs
  src/engine.rs                     processus Python et délais
  src/focus.rs                      identification et vérification X11
  src/insertion.rs                  injection et politique de fallback
  src/clipboard.rs                  copie consentie et effacement explicite
  src/settings.rs                   configuration et migrations
  tests/                            intégration Rust et faux moteur
engine/
  main.py                           boucle JSONL et gestion d’annulation
  protocol.py                       validation et erreurs
  transcriber.py                    modèle local, VAD et transcription
  requirements.in                   dépendances directes
  requirements.lock.txt             versions reproductibles
  tests/                            tests Python
contracts/
  engine-v1.schema.json              contrat Rust/Python
  fixtures/                         messages valides et invalides partagés
tests/
  fixtures/audio/                    silence et voix autorisées fr/en
  fixtures/README.md                 provenance, consentement et licence
  manual/recette.md                  R-01 à R-16 et preuves
docs/
  developpement/plan.md               ce plan et son journal
  architecture.md                   frontières, événements et cycle de session
  decisions/                        résultats des prototypes et arbitrages
  installation.md                   environnement, build et modèle local
  confidentialite.md                fichiers temporaires, presse-papiers, logs
  recette.md                        environnement, mesures et limitations
README.md                            démarrage et usage
```

Les tests unitaires Rust peuvent être placés dans les modules concernés. Les tests React vivent auprès des composants. Ne créer chaque fichier que lorsque le lot correspondant en a besoin.

## 4. Ordre, dépendances et jalons

| Ordre | Lot du plan | Référence CDC | Dépendances | Sortie vérifiable |
|---|---|---|---|---|
| 1 | Socle et environnement | T-01 | Aucune | Fenêtre et commande Tauri fonctionnelles |
| 2 | Prototype du raccourci X11 | T-06 | 1 | Appui/relâchement démontrés dans deux applications |
| 3 | Contrats et état | T-02 | 1 | Schémas et transitions testés |
| 4 | Moteur Python local | T-03 | 3 | Fixture transcrite sans réseau |
| 5 | Supervision Rust/Python | T-04 | 3, 4 | Fixture transcrite depuis Tauri |
| 6 | Audio privé et push-to-talk | T-05 | 2, 3 | Capture réelle, annulation et limite testées |
| 7 | Orchestration, focus et insertion | T-07 | 2, 5, 6 | Dictée réelle et fallback fonctionnels |
| 8 | Réglages, UI et diagnostic | T-08 | 7 | Parcours complet utilisable au clavier |
| 9 | Durcissement transversal | T-09 | 4 à 8 | Confidentialité et récupération vérifiées |
| 10 | Recette et distribution | T-10 | 1 à 9 | Build reproductible, paquet et rapport |

Le prototype T-06 passe volontairement avant le moteur : il vérifie tôt le risque principal du push-to-talk. Les jalons du CDC restent : **J1** après les lots 1, 3, 4, 5 ; **J2** après 7 ; **J3** après 10.

## 5. Tâches à exécuter

### Lot 1 — Initialiser le socle desktop

**Suivi :** code et tests frontend ajoutés ; recette native encore ouverte (WebKitGTK/rsvg manquants).

**Exigences :** NF-08, NF-09 ; T-01.
**Fichiers :** `package.json`, verrou npm, configuration TypeScript/Vite/Tailwind, `src/app/App.tsx`, `src-tauri/Cargo.toml`, verrou Cargo, `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs`, `src-tauri/capabilities/default.json`, `README.md`, `docs/installation.md`.
**Interface produite :** commande Tauri `get_app_info() -> AppInfo`, avec `name` et `version` ; aucun accès système arbitraire depuis React.

- [x] Relever les versions installées, les prérequis WebKitGTK/Tauri et la session graphique ; distinguer une absence d’outil d’un défaut du produit.
- [x] Initialiser Tauri 2/React/TypeScript/Tailwind et une fenêtre minimale ; limiter les capacités Tauri aux besoins réels.
- [x] Définir les scripts npm `dev`, `build`, `typecheck`, `lint`, `test` et `tauri` ; configurer Vitest/Testing Library et les contrôles Rust.
- [x] Afficher les informations renvoyées par `get_app_info` et une présentation textuelle de l’état initial.
- [ ] Vérifier `npm run typecheck`, `npm run lint`, `npm run build`, `cargo check --manifest-path src-tauri/Cargo.toml`, puis `npm run tauri dev` sur un poste graphique. Attendu : compilation réussie et fenêtre visible recevant la réponse Rust.
- [x] Documenter les commandes d’installation et les versions ; enregistrer le bilan du lot.

### Lot 2 — Prouver le raccourci global sur X11

**Suivi :** raccourci maintenant relié à la capture/transcription, Échap temporaire et tests natifs validés ; réception matérielle Cinnamon/X11 encore à recetter. Les deux assertions appui/relâchement sont regroupées dans `held_key_emits_one_press_and_one_release`. Cycle initial Rust écrit avant code, sans exécution rouge faute de compilateur ; suite finale verte.

**Exigences :** F-01, F-02 ; T-06.
**Fichiers :** `src-tauri/src/hotkey.rs`, `hotkey_relay.rs`, `desktop.rs`, `src/features/hotkey/`, branchement dans `lib.rs`, `docs/decisions/001-raccourci-x11.md`.
**Interface produite :** `HotkeyManager::replace(shortcut: &str) -> Result<(), HotkeyError>` et événements internes `HotkeyEvent::{Pressed, Released}`.

- [x] Écrire les tests `held_key_emits_one_press`, `release_emits_once`, `failed_replace_preserves_previous_shortcut` ; un maintien ne produit qu’un cycle et un échec conserve l’ancien raccourci.
- [ ] Exécuter ces tests avant implémentation, puis réaliser le prototype `Ctrl+Alt+Space` ; isoler la gestion de répétition de la bibliothèque choisie.
- [x] Rendre le remplacement transactionnel : enregistrer le nouveau raccourci avant d’abandonner l’ancien, ou restaurer celui-ci en cas d’échec.
- [ ] Vérifier par `cargo test --manifest-path src-tauri/Cargo.toml hotkey`, puis manuellement dans un éditeur et un navigateur X11 : maintien, relâchement, répétition, conflit, raccourci invalide et perte de focus de l’UI.
- [x] Documenter les versions et les limites. Si le relâchement global n’est pas fiable, résoudre ce blocage avant d’intégrer la capture ; aucun changement implicite vers un fonctionnement par bascule.

### Lot 3 — Fixer le contrat et la machine à états

**Suivi :** contrat et machine pure validés en Rust/Python ; aucune capture ou insertion encore intégrée. Les commandes de tests ci-dessous sont exécutées avec `--no-default-features --locked` tant que les prérequis desktop manquent.

**Exigences :** F-09, NF-06, NF-08 ; T-02.
**Fichiers :** `contracts/engine-v1.schema.json`, `contracts/fixtures/`, `src-tauri/src/protocol.rs`, `src-tauri/src/app_state.rs`, `engine/protocol.py`, `src/shared/ipc/types.ts`, `docs/architecture.md`.
**Interfaces produites :** `DictationState`, `SessionId`, `AppError`, `EngineRequest`, `EngineResponse` ; `transition(state, event) -> Result<(state, effects), AppError>` et `parse_engine_line(bytes) -> Result<EngineResponse, AppError>` côté Rust.

- [x] Définir les états initialisation, prêt, enregistrement, transcription, insertion, succès, annulé et erreur ; préciser les transitions de récupération. Une requête invalidée ne peut jamais produire une insertion.
- [x] Définir `ready`, `transcribe`, `result`, `error`, `cancel`, ainsi qu’un acquittement `cancelled` dans la proposition v1, avec `protocol_version`, `request_id` et types stricts. Préciser comment le moteur reçoit le chemin de modèle au démarrage et comment il vérifie sa cohérence avec les requêtes.
- [x] Reprendre toutes les erreurs normalisées du CDC ; conserver le détail technique hors du message utilisateur et exclure les contenus sensibles des logs.
- [x] Écrire des fixtures partagées et tests : JSON invalide, champ absent, mauvais type, version inconnue, UTF-8 invalide, ligne dépassant 1 Mio, message fragmenté, identifiant inconnu et transition interdite.
- [x] Implémenter la validation aux deux extrémités et aligner les types UI sur les événements Rust ; le navigateur ne reçoit pas de chemin temporaire audio.
- [x] Exécuter `cargo test --manifest-path src-tauri/Cargo.toml protocol`, `cargo test --manifest-path src-tauri/Cargo.toml app_state` et `.venv/bin/python -m pytest engine/tests/test_protocol.py`. Attendu : mêmes fixtures acceptées/refusées des deux côtés, y compris limites exactes.

### Lot 4 — Construire le moteur Python hors ligne

**Suivi :** moteur et tests rapides ajoutés ; installation complète du runtime et recette réelle fr/en hors réseau encore ouvertes. stderr natif neutralisé ; erreurs utiles normalisées dans JSONL. Les cas de test prévus sont couverts sous les noms précis indiqués dans le rapport moteur.

**Exigences :** F-04, F-05, F-06, F-10, NF-01 ; T-03.
**Fichiers :** `engine/main.py`, `engine/transcriber.py`, tests Python, fichiers de dépendances, `tests/fixtures/`, `docs/installation.md`.
**Interface produite :** `Transcriber.transcribe(audio_path: Path, language: Literal['fr', 'en', 'auto']) -> TranscriptResult` ; CLI JSONL selon le contrat du lot 3.

- [x] Créer `.venv`, verrouiller les dépendances Python compatibles et documenter l’import d’un modèle local complet, y compris les ressources nécessaires à la VAD.
- [x] Écrire les tests `missing_model_never_downloads`, `corrupt_model_reports_load_error`, `silence_returns_no_speech`, `language_is_forwarded` ; utiliser un adaptateur simulé pour les tests rapides.
- [x] Charger faster-whisper uniquement depuis le chemin local validé ; `auto` devient la détection automatique attendue par l’API. Réserver stdout à JSONL, stderr aux diagnostics filtrés.
- [x] Implémenter VAD et collecte complète du résultat. Définir le comportement texte vide, fichier absent/invalide et moteur occupé, sans envoyer plusieurs résultats pour une requête.
- [x] Permettre à la boucle de contrôle de recevoir `cancel` pendant le calcul ; confirmer une annulation seulement quand le résultat est neutralisé. La terminaison forcée par Rust reste le recours si le calcul ne s’interrompt pas.
- [ ] Exécuter `.venv/bin/python -m pytest engine/tests` puis un test d’intégration marqué `local_model`, avec fixtures fr/en et silence, modèle local préparé et réseau coupé. Attendu : résultat pertinent fr/en, aucune insertion possible pour le silence et aucun téléchargement tenté. Signaler un test sauté faute de modèle.

### Lot 5 — Superviser le processus Python depuis Rust

**Suivi :** superviseur intégré et 12 tests de vrais processus Python de test validés ; fixture avec vrai modèle depuis Tauri toujours à recetter. `launch` fournit le handle immédiatement ; `start` attend `ready`.

**Exigences :** F-05, F-10, NF-05, NF-06, NF-11 ; T-04.
**Fichiers :** `src-tauri/src/engine.rs`, intégration dans `commands.rs` et `lib.rs`, `src-tauri/tests/engine_process.rs`, faux moteur de test.
**Interface produite :** `EngineClient::start(config)`, `transcribe(request)`, `cancel(session_id)`, `shutdown()` ; résultats asynchrones associés au `request_id`.

- [x] Écrire un faux processus qui simule `ready`, succès, retard, crash, JSON invalide, stdout excessif et absence d’acquittement ; tests avec horloge contrôlée lorsque possible.
- [x] Lancer un exécutable Python et des arguments séparés, sans shell ; lire stdout/stderr sans bloquer l’UI et sans croissance mémoire non bornée.
- [x] Attendre `ready` 60 s, limiter une transcription à 120 s, refuser la concurrence avec `ENGINE_BUSY` et ignorer les réponses inconnues ou tardives sans journaliser leur texte.
- [x] À l’annulation, invalider la session immédiatement, envoyer `cancel`, attendre au plus 3 s puis terminer si nécessaire. Récolter le processus terminé ; redémarrer de façon bornée avant une nouvelle dictée.
- [x] À la fermeture, arrêter et attendre l’enfant ; tester aussi un arrêt pendant le démarrage et pendant une transcription.
- [ ] Exécuter `cargo test --manifest-path src-tauri/Cargo.toml --test engine_process`. Attendu : délais, crash et récupération passent sans processus orphelin ; vérifier ensuite une fixture depuis Tauri avec le vrai moteur local. **J1** acquis seulement après cette démonstration.

### Lot 6 — Capturer et transporter l’audio sans rétention

**Suivi :** capture CPAL, conversion et stockage privé intégrés ; compilation et tests natifs synthétiques validés. Microphone réel non ouvert automatiquement.

**Exigences :** F-03, F-11, F-12, F-13, NF-02 ; T-05.
**Fichiers :** `src-tauri/src/audio.rs`, `src-tauri/src/temp_audio.rs`, tests associés.
**Interfaces produites :** `AudioRecorder::start(device_id)`, `stop() -> AudioBuffer`, `discard()` ; `TempAudioStore::write_wav(session_id, buffer) -> TempAudioHandle`, avec nettoyage explicite et protection à la destruction.

- [x] Tester avec une source audio simulée : aucune capture au repos, appui très court, arrêt unique, déconnexion micro, annulation et limite à 60 s ; borne mémoire calculée selon la durée et le format.
- [x] Implémenter l’énumération et la capture ; convertir explicitement les formats microphone vers WAV PCM mono 16 kHz, avec tests de durée/fréquence/amplitude de la conversion.
- [x] Relier l’appui/relâchement validé au lot 2. Sur limite ou erreur audio : arrêter, abandonner la session et informer sans lancer de transcription.
- [x] Créer les fichiers temporaires sans collision ni suivi de lien symbolique, vérifier les permissions et limiter leur emplacement au répertoire privé de l’application.
- [x] Tester `private_permissions`, `cleanup_on_all_exit_paths`, `stale_cleanup_does_not_follow_symlinks` et le refus d’un chemin extérieur ; ne jamais supprimer un fichier étranger lors du nettoyage.
- [ ] Exécuter `cargo test --manifest-path src-tauri/Cargo.toml audio` et `cargo test --manifest-path src-tauri/Cargo.toml temp_audio`, puis réaliser une capture microphone réelle. Attendu : WAV exploitable, aucun enregistrement au repos et aucun fichier restant après les chemins de sortie testés.

### Lot 7 — Relier dictée, focus, insertion et fallback

**Exigences :** F-01, F-07, F-08, F-09, F-12, NF-05 ; T-07.
**Fichiers :** `src-tauri/src/controller.rs`, `focus.rs`, `insertion.rs`, `clipboard.rs`, tests de politiques et vue minimale de fallback.
**Interfaces produites :** `FocusTracker::capture_target() -> TargetRef`, `verify_target(target) -> FocusStatus`, `TextInserter::insert(target, text) -> InsertionOutcome`, `ClipboardService::copy(text)` ; `DictationController::{start, stop, cancel}`.

- [ ] Écrire les tests de politique avec faux focus/inserteur : cible inchangée, fenêtre fermée, autre fenêtre active, focus incertain, champ différent détectable, insertion désactivée et Unicode français/anglais.
- [ ] Orchestrer capture → fichier privé → moteur → validation de session et de cible → insertion unique → nettoyage. Révérifier au plus près de l’injection ; documenter la limite d’atomicité X11.
- [ ] Ajouter la cible plus précise si disponible ; sinon documenter le changement de champ dans une même fenêtre et fournir la désactivation de l’insertion automatique.
- [ ] Éviter que la fenêtre d’état vole le focus pendant la dictée ; toute ouverture de réglages pendant une session doit rester sûre.
- [ ] Tester le fallback avec presse-papiers sentinelle : sans consentement il reste intact ; clic « Copier » ou consentement explicite autorisent la copie. L’échec de copie garde le résultat disponible jusqu’à expiration ou effacement.
- [ ] Prévoir « Effacer le résultat » et l’effacement manuel du presse-papiers, avec information sur sa nature partagée ; aucune restauration automatique d’un ancien contenu.
- [ ] Tester annulation et double réponse juste avant insertion : aucun texte après annulation, aucune double insertion. Lors d’une insertion partielle incertaine, avertir avant de proposer de recoller le texte entier.
- [ ] Exécuter `cargo test --manifest-path src-tauri/Cargo.toml`, puis les parcours réels éditeur/navigateur : texte accentué, focus changé, cible fermée, insertion refusée, copie refusée et copie consentie. **J2** acquis après dictée réelle et fallback démontrés.

### Lot 8 — Finaliser réglages, états et diagnostic

**Exigences :** F-02, F-06, F-09 à F-14, NF-04, NF-07 ; T-08.
**Fichiers :** `src/features/{dictation,settings,diagnostics}/`, `src/shared/ipc/`, `src-tauri/src/settings.rs`, `commands.rs`, tests React et Rust.
**Interfaces produites :** commandes `get_status`, `get_settings`, `update_settings`, `list_microphones`, `get_diagnostics`, `cancel_dictation`, `copy_result`, `clear_result`, `clear_clipboard` ; événement `dictation://state` avec session et état.

- [ ] Définir `Settings` version 1 : champs du CDC, `auto_insert_enabled: true` et `clipboard_auto_copy_consent: false`. `clipboard_fallback: true` signifie que l’option de récupération est disponible, jamais que le consentement est acquis.
- [ ] Valider `fr/en/auto`, chemins lisibles, périphérique, raccourci et durée positive bornée ; proposition de plage UI : 1 à 60 s pour le MVP. Sauvegarder atomiquement ; garder la configuration précédente si un réglage ou son application échoue.
- [ ] Tester configuration absente/corrompue, version inconnue, sauvegarde impossible et conflit de raccourci ; afficher l’erreur sans écraser le fichier invalide silencieusement.
- [ ] Créer les vues état/réglages/diagnostic ; états textuels, bouton Annuler, erreurs avec action corrective et fallback visible. Un état « succès » ne doit pas annoncer une insertion lorsque seule la récupération du résultat est proposée.
- [ ] Relier Échap à l’annulation globale pendant une session sous X11 et conserver le bouton UI ; tester aussi l’annulation pendant la transcription.
- [ ] Proposer microphone, modèle local, langue, raccourci, limite, insertion automatique et consentement à la copie avec explication. Ne pas appliquer un changement de moteur au milieu d’une session.
- [ ] Diagnostiquer X11, microphone, modèle, moteur ; sous Wayland afficher une incompatibilité claire sans promettre le parcours garanti.
- [ ] Tester React : transitions, moteur occupé, erreurs, boutons, consentement initial absent et purge du résultat après 120 s. Vérifier clavier, focus visible, libellés accessibles et contraste.
- [ ] Exécuter `npm run typecheck`, `npm run lint`, `npm run test -- --run`, `npm run build`, `cargo test --manifest-path src-tauri/Cargo.toml`, puis vérifier les réglages après redémarrage et mesurer la notification visuelle appui/relâchement.

### Lot 9 — Vérifier la confidentialité et la résistance aux échecs

**Exigences :** NF-01, NF-02, NF-05, NF-06, NF-10, NF-11 ; T-09.
**Fichiers :** tests d’intégration Rust/Python, configuration Tauri, `docs/confidentialite.md`, `docs/architecture.md`.

- [ ] Auditer les chemins UI → Rust → Python : entrées validées, chemins contrôlés, absence de shell, privilèges minimaux, aucune ressource distante chargée par l’UI.
- [ ] Tester une transcription contenant une chaîne sentinelle : elle n’apparaît ni dans les logs Rust/Python ni dans les erreurs techniques ; contrôler aussi le mode verbeux.
- [ ] Tester succès, erreur, annulation, timeout, fermeture et redémarrage après crash : buffers libérés, fichiers privés nettoyés, processus enfant récolté.
- [ ] Tester réponses tardives, annulation suivie immédiatement d’une nouvelle session, temp malveillant, micro retiré et lecture protocole trop volumineuse. Attendu : aucune insertion obsolète ni suppression hors périmètre.
- [ ] Observer le trafic de l’application et de son enfant pendant une dictée ; distinguer leurs connexions de celles du reste du poste. Compléter par une exécution réseau coupé.
- [ ] Documenter les traces temporaires sur disque, la rétention UI, les accès au presse-papiers et les limites des champs secrets/focus X11.
- [ ] Relancer uniquement les suites concernées et inscrire les preuves, les défauts corrigés et les limites restantes.

### Lot 10 — Recette finale et livraison reproductible

**Exigences :** NF-03, NF-04, NF-07, NF-09 ; T-10 et tous les critères de livraison.
**Fichiers :** `tests/manual/recette.md`, `docs/recette.md`, `docs/installation.md`, `README.md`, configuration de packaging.

- [ ] Relever Mint, Cinnamon/X11, CPU, RAM, backend audio, Python, version du paquet, modèle, quantification, langue et microphone.
- [ ] Exécuter R-01 à R-16 selon la matrice du CDC ; inscrire pour chaque scénario : procédure, résultat attendu/observé, preuve, statut et limitation. Un test matériel indisponible reste « non exécuté ».
- [ ] Mesurer le délai visuel appui/relâchement et le temps après relâchement sur une fixture de 10 s. NF-04 doit tenir 300 ms ; NF-03 doit être mesuré et son éventuel écart expliqué.
- [ ] Vérifier les builds et tests : `npm run typecheck`, `npm run lint`, `npm run test -- --run`, `npm run build`, `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`, `cargo test --manifest-path src-tauri/Cargo.toml`, `.venv/bin/python -m pytest engine/tests`, `npm run tauri build`.
- [ ] Produire un paquet Linux Mint installable et documenter le moteur Python : emplacement, environnement et dépendances natives. Vérifier les chemins dans l’application installée, sans dépendre du répertoire du dépôt.
- [ ] Tester l’installation sur un environnement Mint propre ou équivalent documenté ; fournir le modèle séparément/import manuel, sa provenance et ses conditions de distribution. Aucune installation réseau implicite lors de la dictée.
- [ ] Livrer guides d’installation, préparation du modèle, usage hors ligne, dépannage, architecture, confidentialité, rapport de recette et limites. Faire une démonstration hors ligne incluant le fallback.
- [ ] Déclarer **J3** seulement après satisfaction des critères ci-dessous.

## 6. Couverture et points de revue prioritaires

| Exigences | Lots responsables |
|---|---|
| F-01, F-02 | 2, 7, 8 |
| F-03, F-12, F-13 | 6, 7, 8 |
| F-04, F-05, F-06 | 4, 5, 8 |
| F-07, F-08 | 7, 8 |
| F-09, F-10, F-11, F-14 | 3 à 8 |
| F-15 | Hors MVP, limite explicitée en 9 et 10 |
| NF-01, NF-02 | 4, 6, 7, 9 |
| NF-03, NF-04 | 8, 10 |
| NF-05, NF-06, NF-10, NF-11 | 3, 5, 9 |
| NF-07, NF-08, NF-09 | 1, 3, 8, 10 |

Points à contrôler particulièrement lors des revues ; leurs tests sont prévus dans les lots indiqués :

1. **Réponse reçue après annulation ou timeout :** aucune insertion, même après le début d’une nouvelle session — lots 3, 5, 7, 9.
2. **Champ changé dans la même fenêtre :** refuser si détectable, sinon limite expliquée et insertion désactivable — lots 7, 8, R-16.
3. **Presse-papiers déjà rempli :** il reste intact sans consentement ; une erreur de copie ne détruit pas immédiatement le résultat — lot 7, R-06.
4. **Crash laissant un WAV ou un lien symbolique :** nettoyage restreint aux fichiers privés de l’application, aucun fichier étranger supprimé — lots 6, 9, R-14.
5. **Modèle absent, corrompu ou mal empaqueté :** erreur locale exploitable sans téléchargement de secours, y compris dans le paquet installé — lots 4, 10, R-08.

## 7. Conditions de fin

Une tâche est terminée lorsque son livrable fonctionne, ses tests pertinents passent, ses erreurs sont vérifiées et ses résultats sont consignés. Une compilation seule ne valide pas l’usage desktop.

Pour le MVP : toutes les P0 satisfaites ; R-01 à R-10, R-12 à R-14 et R-16 validés sur Linux Mint Cinnamon/X11 ; R-15 satisfait NF-04 et rapporte NF-03 ; démonstration hors ligne complète avec fallback.

Ce plan prévoit également les P1, dont la limite configurable et R-11. Tout report doit être explicite avec son impact ; une limite de capture fixe et documentée reste obligatoire si F-13 est reportée. Pas de revendication de compatibilité Wayland ni de protection universelle des champs secrets.

## 8. Journal de progression

| Date | Lot | État | Vérifications réellement exécutées | Suite |
|---|---|---|---|---|
| 2026-11-03 | Planification | Plan rédigé ; développement non commencé | Lecture du CDC et examen du dossier ; aucune compilation ou recette produit | Lot 1, puis prototype X11 du lot 2 |
| 2026-11-03 | T-01 / lot 1 | Code ajouté ; recette native ouverte | Typecheck, lint, 8 tests React, build frontend et contrôle navigateur réussis ; diagnostic WebKitGTK/rsvg absents | Installer prérequis et lancer la fenêtre Tauri |
| 2026-11-03 | T-06 / lot 2 | Prototype ajouté ; recette X11 ouverte | Politique et relais testés sans WebView ; deux courses corrigées après revue | Vérifier maintien/relâchement dans deux applications |
| 2026-11-03 | T-02 / lot 3 | Contrat et état validés automatiquement | Suite Rust 18/18, protocole Python 5 tests/69 sous-tests, fmt/Clippy verts | Brancher au superviseur T-04 |
| 2026-11-03 | T-03 / lot 4 | Code ajouté ; recette modèle réelle ouverte | Python 36 réussis, 1 test modèle sauté, 69 sous-tests ; runtime complet non installé ici | Installer runtime et tester modèle/fr/en/silence hors réseau |


Pour chaque futur bilan, ajouter les chemins modifiés, les commandes et résultats réels, les exigences validées, les blocages et la prochaine étape. Garder les cases composites ouvertes tant que toute leur validation n’est pas exécutée. Voir [bilan T-01 à T-03](bilan-t01-t03.md) pour les fichiers modifiés, résultats et contrôles utilisateur.


## 9. Incrément T-04 à T-06

Code et tests ajoutés le 3 novembre 2026. Voir [bilan T-04 à T-06](bilan-t04-t06.md) pour les fichiers, commandes réellement exécutées, résultats et recette utilisateur. Validation finale native : 42 tests unitaires + 12 tests processus ; React 20 ; Python 36 réussis/1 modèle réel sauté. Typecheck, lint, build frontend, compilation et Clippy natifs réussis.

Un panneau de préparation limité à la session a été ajouté pour rendre les lots testables ; la persistance et le diagnostic complet restent en T-08. Le contrôleur prévisualise le texte et ne réalise aucune insertion T-07. Une génération d’entrée invalide les appuis en attente après annulation ou saturation. L’UI utilise les codes et un motif `recording_limit`, sans recopier les messages bruts du moteur.

Les cases combinant test automatique et recette matérielle restent ouvertes. Ne pas déduire J1/J2 de la simulation : modèle réel, micro, X11 et performances restent à vérifier. Prochaine tâche produit : T-07, après reprise de ces recettes.

## 10. Incrément demandé : modèles installables et sélection

Le catalogue épinglé `config/models.json`, l’installateur explicite `scripts/install_models.py`, la commande Tauri `list_local_models` et le sélecteur de transcription sont ajoutés. Un modèle choisi est appliqué uniquement par « Préparer le moteur » ; le modèle actif est identifié dans les snapshots. Les chemins personnalisés restent possibles. Aucun téléchargement à l’exécution ni préférence persistante n’est ajouté ; T-08 reste distinct.

Voir [les modèles locaux](../modeles.md), [la revue](revue-modeles.md), [le bilan interface](modeles-ui-report.md) et [le bilan backend](modeles-backend-report.md). L’installation effective et ses contrôles sont consignés séparément dans `modeles-install-report.md`. La sélection Tiny → Base est testée avec un faux moteur ; elle ne constitue pas une mesure de précision ni une recette microphone/X11.

## 11. Extension demandée : transcription Groq optionnelle

Le 4 novembre 2026, ajout du choix Local/Groq, de la configuration backend par `.env` et de l’adaptateur API. Le mode local demeure proposé par défaut ; seul le fournisseur explicitement préparé peut transcrire. Le contrat, les tests et la recette utilisateur sont dans [le plan Groq](plan-groq.md) et [le bilan](bilan-groq.md). Cette extension ne réalise pas l’insertion T-07 ni la persistance T-08. L’appel réel Groq attend une clé et une dictée consentie de l’utilisateur.


## Reprise T-07–T-08 — 10 novembre 2026

L’implémentation de ces deux lots est présente dans `dictation.rs`, `focus.rs`, `insertion.rs`, `settings.rs`, `desktop.rs` et les vues React. Les sections précédentes sont des états historiques. La reprise finalise la vue Dictée (raccourci, état et résultat), les réglages dans une vue dédiée, les diagnostics actionnables, le brouillon conservé en navigation et le focus clavier. Les icônes Lucide sont embarquées via Iconify ; le skill UI/UX Pro Max a été appliqué en conservant le design existant.

Les cas automatisés et les limites de recette sont consignés dans [bilan-t07-t08.md](bilan-t07-t08.md). Les cases mêlant implémentation et recette matérielle restent ouvertes tant que la dictée réelle au microphone dans un éditeur et un navigateur et les mesures NF-04 n’ont pas été réalisées. Ne pas reconstruire T-07/T-08 à la reprise : terminer leur recette native puis poursuivre T-09/T-10.
