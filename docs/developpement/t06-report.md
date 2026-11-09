# T-06 — Adaptateur natif et dictée locale

L’adaptateur Tauri initialise le contrôleur de dictée et le propriétaire du microphone au démarrage, sans lancer de capture. Seul un appui global accepté par la politique du raccourci est transmis au contrôleur ; le relâchement lance le parcours audio puis moteur. Le gestionnaire de raccourci libère son verrou avant de transmettre ces événements.

Les commandes exposées à la fenêtre principale sont `get_dictation_status`, `list_microphones`, `configure_dictation`, `cancel_dictation` et `clear_dictation_result`. Elles réutilisent les types du contrôleur ; les instantanés révisés sont émis sur `dictation://state`. Le manifeste de commandes et les capacités n’autorisent que ces opérations locales et les diagnostics déjà présents. Une erreur d’initialisation devient un `RuntimeError` contrôlé et laisse l’application ouverte. La préparation du moteur global est refusée hors X11.

Le chemin de développement est le parent canonique de `CARGO_MANIFEST_DIR`, avec Python dans `.venv/bin/python`. Le répertoire audio est placé sous `app_cache_dir()/audio` ; seul le parent de cache est créé par l’adaptateur, le stockage audio crée lui-même son répertoire privé. La résolution du Python et l’inclusion du paquet moteur dans une distribution restent à définir pour le conditionnement final.

La fermeture arrête le relais et conserve un `Arc` externe du contrôleur jusqu’après `run_return`. `shutdown()` arrête et rejoint son thread, lequel termine et récolte le Python, y compris pendant son initialisation. Aucun lancement du microphone réel n’est effectué par les tests de politique.

## Annulation globale

Échap simple et Échap accompagné des modificateurs du raccourci de dictée ne sont enregistrés que pendant le chargement, l’enregistrement, la transcription ou l’annulation. Le relais les reconnaît avant le filtre du raccourci de dictée et positionne immédiatement le signal d’annulation. Échap, avec ou sans modificateur, est réservé et refusé comme raccourci de dictée.

Les opérations du plugin s’exécutent sur le thread UI dans un état indépendant du verrou du gestionnaire de raccourci ; elles n’attendent donc pas un travailleur bloqué sur ce même thread. Le code du runtime Wry mis en cache confirme que `run_on_main_thread` exécute directement les tâches lorsqu’il est appelé du thread principal. Seuls les raccourcis effectivement enregistrés sont conservés pour le retrait. Les changements de phase identiques ne provoquent aucune nouvelle tentative répétitive. Un conflit d’enregistrement émet un message contrôlé `dictation://notice` invitant à utiliser le bouton Annuler ; un échec de retrait demande de relancer l’application. Le diagnostic frontend écoute cet événement et explique désormais qu’un appui peut réellement enregistrer lorsque le moteur est prêt.

## Validation

La nouvelle assertion frontend sur l’avertissement de capture a échoué avant la modification de la copie (RED), puis réussi. Les 7 tests du diagnostic et la suite frontend complète (**18 tests**) passent.

Compilation de l’adaptateur desktop avec les bibliothèques natives disponibles :

```sh
PKG_CONFIG_PATH=/tmp/lvf-native-sysroot/usr/lib/x86_64-linux-gnu/pkgconfig RUSTUP_HOME=/tmp/lvf-rustup CARGO_HOME=/tmp/lvf-cargo PATH=/tmp/lvf-cargo/bin:$PATH cargo check --manifest-path src-tauri/Cargo.toml --offline --locked
npm test -- --run
```

Le `cargo check` avec les fonctionnalités desktop par défaut réussit. Les deux tests desktop des phases actives et des modificateurs Échap réussissent (binaire natif lié). `npm run build` réussit également.

Un passage natif complet a ensuite trouvé 38 tests réussis et les deux nouveaux tests RED du contrôleur encore en cours de correction : `fatal_engine_failure_stops_microphone_immediately` et `saturated_release_invalidates_queued_press_without_starting_audio`. La suite sera relancée après les corrections du propriétaire de `dictation.rs`.

Une démonstration réelle reste nécessaire sur Cinnamon/X11 : modèle installé, capture volontaire, relâchement, Échap simple et avec modificateurs maintenus, bouton Annuler, fermeture pendant le chargement et vérification de la suppression du WAV. Aucun succès matériel ni jalon J1/J2 n’est annoncé par ces tests automatisés.


## Complément final du coordinateur

Après intégration et corrections de revue : suite desktop complète **42 tests unitaires +12 tests processus réussis**, Clippy desktop et sans desktop avec `-D warnings` réussis, formatage réussi. Frontend **20 tests réussis**, TypeScript/ESLint/build réussis ; Python **36 réussis, 1 modèle réel sauté, 69 sous-tests réussis**. Les compteurs intermédiaires ci-dessus décrivent leur passage historique. Le bilan consolidé `bilan-t04-t06.md` contient les commandes, le timeout de démarrage Vitest transitoire et les recettes matérielles encore ouvertes. La zone de résultat finale suit la règle de hauteur stable `resize-none` avec défilement interne, documentée dans UX-CONTRACT.md ; les décisions de sélecteurs natifs sont persistées dans premium-ui.json. Aucun succès microphone/modèle réel n’est revendiqué.
