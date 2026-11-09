# T-02 — Contrats et état

Exigences : F-09, NF-06, NF-08 ; JSONL v1, erreurs normalisées, identité de session, annulation et absence de double insertion. Périmètre limité au contrat et à une machine pure ; aucune capture/injection réelle n'est validée ici.

Fichiers : `contracts/engine-v1.schema.json`, `contracts/fixtures/messages.json`, `src-tauri/src/protocol.rs`, `src-tauri/src/app_state.rs`, `src/shared/ipc/types.ts`, `engine/protocol.py`, `engine/tests/test_protocol.py`, `docs/protocole.md`, ce rapport.

Recherche préalable : dialecte officiel JSON Schema 2020-12, attributs de conteneur et enums taguées Serde, documentation officielle serde_json::from_slice. Les liens précis figurent dans `docs/protocole.md`.

Le contrat valide les messages dans les deux sens, rejette les champs inconnus, types erronés, clés dupliquées, versions inconnues, encodages invalides et lignes trop grandes. Fixtures partagées ; test supplémentaire de conformité sémantique au schéma avec `jsonschema`. Les exceptions d'encodage (clés dupliquées, représentation entière, UTF-8, limite d'octets) se vérifient dans les parseurs, au-delà des valeurs JSON du schéma.

Le décodeur Rust conserve au maximum 1 Mio de données incomplètes, gère fragmentation/coalescence et abandonne le reste d'une ligne excessive jusqu'à LF. Le lecteur Python borne `readline` et refuse EOF partiel. Les encodeurs valident également leurs sorties. Le modèle reçoit un chemin initial CLI et chaque requête doit conserver exactement ce chemin.

La machine couvre initialisation, prêt, capture, transcription, insertion, succès, annulation en attente, annulation terminée et erreur. Un résultat périmé/dupliqué n'émet pas d'insertion. L'annulation invalide aussi un effet d'insertion encore en attente via `can_insert` : l'adaptateur doit refaire cette vérification sous le même verrou que Cancel, immédiatement avant l'effet externe. Les identités doivent croître sans réutilisation. L'identité 0 désigne une erreur d'initialisation ; les autres identités restent dans la plage sûre JavaScript.

## Vérifications effectivement exécutées

- Tests Python écrits avant `protocol.py` : `python3 -m unittest engine.tests.test_protocol` a d'abord échoué sur l'import du module absent, puis 4 tests ont passé après implémentation.
- `.venv/bin/python -m pytest engine/tests/test_protocol.py -q` : **5 passed, 69 subtests passed** ; conformité JSON Schema incluse.
- `.venv/bin/python -m pytest engine/tests -q` : **32 passed, 1 skipped, 69 subtests passed** à cette étape. Le test facultatif de transcription sur modèle local reste ignoré sans modèle préparé ; voir rapport T-03 pour les vérifications finales du moteur.
- Une tentative intermédiaire de découverte avec le Python système a échoué sur `pytest` absent et `engine.runtime` encore en cours de création par la tâche parallèle. La suite exécutée ensuite dans `.venv` est verte.
- Rust : tests comportementaux écrits avant implémentation, mais `cargo` absent du PATH au moment du cycle initial. La compilation était alors en attente ; elle a ensuite été exécutée par le coordinateur (voir complément ci-dessous).

## Commandes Rust du lot

`cargo test --manifest-path src-tauri/Cargo.toml --no-default-features protocol`

`cargo test --manifest-path src-tauri/Cargo.toml --no-default-features app_state`

Puis formatage via `rustfmt` et suite complète sans fonctionnalités desktop. Les tests ne requièrent ni microphone, ni modèle, ni session graphique.

## Limites et suite

La génération monotone des identités, l'exécution des effets, les délais réels, le redémarrage du processus, le focus et le verrou d'insertion relèvent de T-04/T-07. Les constantes définissent 60/120/3 secondes ; elles ne démarrent aucun timer dans la machine pure. Aucun test matériel n'a été exécuté pour T-02. La recette desktop demeure indispensable. La compilation et les tests métier Rust ont depuis été validés ; les limites matérielles des étapes suivantes restent applicables.


## Complément du coordinateur — vérifications finales

Rust 1.99.1 installé dans `/tmp`, sans modification du profil utilisateur. `Cargo.lock` contient 427 paquets résolus. `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked --offline` : **18 tests réussis**, dont 5 tests de machine à états, 4 de protocole et les tests raccourci/relais/métadonnées. Les mêmes fixtures sont validées dans les deux langages. `cargo fmt -- --check` et Clippy sans feature desktop, tous targets avec `-D warnings`, passent après formatage. La suite Python finale compte 36 réussis, 1 test de modèle réel sauté et 69 sous-tests. Cette validation couvre les modules purs, pas l’adaptateur Tauri/X11.
