# Intégration Groq — contrôleur Rust et protocole

Le contrôleur accepte désormais `RuntimeConfig.provider` (`local` par défaut, ou `groq`) et publie le fournisseur réellement préparé dans `DictationSnapshot.provider`. Le champ ne transporte aucune clé et ne contient pas le nom du modèle distant : ceux-ci restent dans la configuration Python.

`protocol::TranscriptionProvider` est un enum fermé, sérialisé en minuscules. Les anciennes configurations sans fournisseur restent locales ; une valeur inconnue est refusée. `EngineConfig` transmet uniquement `--provider` au processus Python. Les arguments existants de CPU/int8 restent inoffensifs pour Groq, dont le backend Python ne charge aucun poids Whisper local.

En mode local, les vérifications du chemin absolu/canonique et du répertoire modèle restent identiques. En mode Groq, le chemin d’identité JSONL est le répertoire projet, sans dépendance à un dossier `models`. Le moteur Python est lié à cette identité pour empêcher des requêtes décalées ; il ne s’agit pas du modèle distant. Une configuration locale invalide ne remplace pas le moteur Groq déjà prêt. La sélection ne change jamais automatiquement le fournisseur.

Les quatre codes `API_CONFIG_ERROR`, `API_AUTH_ERROR`, `API_RATE_LIMIT` et `API_UNAVAILABLE` sont ajoutés à l’enum Rust, au schéma et au corpus JSONL partagé. Les messages du contrôleur sont des constantes ; aucun corps de réponse distant ni exception système n’est exposé. Les protections de capture/annulation et l’effacement des WAV restent dans le contrôleur existant.

Fichiers fonctionnels : `src-tauri/src/protocol.rs`, `src-tauri/src/engine.rs`, `src-tauri/src/dictation.rs`, `src-tauri/tests/engine_process.rs`, `src-tauri/tests/fixtures/fake_engine.py`, `contracts/engine-v1.schema.json`, `contracts/fixtures/messages.json`. Le formatage Rust a également normalisé les blocs existants.

Tests : première régression de fournisseur observée rouge (snapshot sans provider), puis verte. Deux tests ajoutés vérifient la compatibilité locale par défaut, le rejet des fournisseurs inconnus, la préparation Groq sans poids locaux avec contrôle effectif des arguments du processus, la conservation du fournisseur actif après une configuration locale invalide et le passage explicite Groq → local. Suite headless complète après formatage : **45 unitaires + 12 tests de processus réussis**. `cargo fmt -- --check` réussi. Les outils rustfmt et Clippy officiels ont été installés pour le toolchain utilisateur durable.

Correction après revue : le message de transcription est devenu neutre (« Transcription en cours… ») pour décrire correctement les deux fournisseurs. Changement de texte uniquement, sans modification du comportement.

Compilation native `cargo check --offline --locked` réussie (1 min 43 s), puis `cargo clippy --offline --locked --all-targets -- -D warnings` réussi (11,98 s), sans diagnostic. Les dépendances système ALSA/WebKit et les commandes Tauri sont donc vérifiées à la compilation. Aucun appel réel Groq ni enregistrement microphone dans ces tests. Le faux moteur teste l’orchestration ; les tests HTTP simulés appartiennent au lot Python.
