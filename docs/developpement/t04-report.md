# T-04 — Supervision du moteur Python

Le client `EngineClient` est indépendant de Tauri. `launch(config, timeouts)` rend immédiatement un client et son canal d’événements sur un runtime Tokio actif : le contrôleur peut donc arrêter et attendre le processus pendant son initialisation. `Ready` et les erreurs de démarrage arrivent ensuite sur le canal. `start(config, timeouts)` est asynchrone et attend le `ready` validé avant de rendre le client clonable et son récepteur d'événements. L'appelant doit garder cette attente dans une tâche annulable pour conserver l'interface réactive : abandonner la future de démarrage ferme le canal d'initialisation et déclenche la récolte du processus.

Le processus reçoit des arguments séparés, sans shell, un modèle et un répertoire audio absolus canoniques, les options CPU/int8 et les variables de fonctionnement hors ligne. Une seule transcription est admise. Une annulation invalide immédiatement la réponse en cours ; les résultats tardifs et les identifiants inconnus sont ignorés. Les valeurs par défaut sont 60 s au démarrage, 120 s par transcription et 3 s à l'annulation. Les écritures stdin ont aussi un délai borné. Un crash, délai ou protocole invalide ferme le client ; le contrôleur peut explicitement créer un nouveau client, sans boucle de redémarrage automatique.

Stdout est lu par blocs de 8 Kio dans le décodeur JSONL borné à 1 Mio par ligne. Le canal de lecture (8 messages), les commandes (8) et les événements (32) sont bornés. Un consommateur d'événements saturé provoque un arrêt, sans attente de place qui empêcherait la fermeture. Stderr est vidé par blocs fixes et intégralement jeté : aucune exception ou transcription brute n'est journalisée. L'arrêt termine et attend l'enfant ; `kill_on_drop` sert seulement de protection supplémentaire.

## Validation

Tests d'intégration avec un paquet Python temporaire : succès, réponse inconnue, concurrence refusée, résultat tardif après annulation, absence d'acquittement, délai de transcription, EOF, JSON invalide, ligne excessive, erreur et délai d'initialisation, abandon du dernier client, abandon de la future de démarrage, fermeture du récepteur, configuration invalide, nouveau démarrage explicite et saturation des événements, blocage de stdin et arrêt explicite avant `ready`. Les tests vérifient également l'absence du PID sous `/proc` après récolte.

Commande :

```sh
RUSTUP_HOME=/tmp/lvf-rustup CARGO_HOME=/tmp/lvf-cargo PATH=/tmp/lvf-cargo/bin:$PATH cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --test engine_process --offline --locked
```

Premier passage : échec attendu (module et dépendances absents). Un passage intermédiaire était bloqué par l’absence de `hound` dans le cache ; les dépendances ont ensuite été récupérées. Résultat final : **12 tests d’intégration réussis, 0 échec**, en 0,69 s puis 0,80 s lors du passage complet. Le test `launch_then_shutdown_before_ready_no_orphan` confirme explicitement la récolte avant la fin d’une initialisation bloquée.

Suite Rust complète sans dépendances desktop :

```sh
RUSTUP_HOME=/tmp/lvf-rustup CARGO_HOME=/tmp/lvf-cargo PATH=/tmp/lvf-cargo/bin:$PATH cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --offline --locked
```

Résultat : **30 tests unitaires + 12 tests d’intégration réussis, 0 échec** ; 0 doctest. `rustfmt --edition 2021 --check src-tauri/src/engine.rs src-tauri/tests/engine_process.rs` réussit. La vérification générale `cargo fmt --check` a signalé les autres fichiers en cours de développement au moment du passage ; ils restent sous la responsabilité de leurs auteurs.

Le vrai modèle Whisper et la validation Tauri matérielle ne sont pas disponibles ici : le jalon J1 reste à démontrer sur la machine cible.


## Complément final du coordinateur

Après intégration et corrections de revue : suite desktop complète **42 tests unitaires +12 tests processus réussis**, Clippy desktop et sans desktop avec `-D warnings` réussis, formatage réussi. Frontend **20 tests réussis**, TypeScript/ESLint/build réussis ; Python **36 réussis, 1 modèle réel sauté, 69 sous-tests réussis**. Les compteurs intermédiaires ci-dessus décrivent leur passage historique. Le bilan consolidé `bilan-t04-t06.md` contient les commandes, le timeout de démarrage Vitest transitoire et les recettes matérielles encore ouvertes. La zone de résultat finale suit la règle de hauteur stable `resize-none` avec défilement interne, documentée dans UX-CONTRACT.md ; les décisions de sélecteurs natifs sont persistées dans premium-ui.json. Aucun succès microphone/modèle réel n’est revendiqué.
