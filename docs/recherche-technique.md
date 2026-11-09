# Recherche technique préalable — T-01 à T-03

Recherche effectuée le 3 novembre 2026 avant le code des modules concernés. Sources primaires : documentation officielle et dépôts des mainteneurs. Aucun code tiers copié sans attribution ; exemples utilisés pour vérifier les API et frontières.

| Source | Observation | Application dans le projet |
|---|---|---|
| [Tauri — Vite](https://v2.tauri.app/start/frontend/vite/) | Port de développement stable, sortie frontend explicite et configuration adaptée à la WebView | Vite/TypeScript, `strictPort`, frontend servi localement uniquement |
| [Tauri — commandes Rust](https://v2.tauri.app/develop/calling-rust/) | Les commandes exposent des valeurs sérialisables et peuvent retourner des erreurs structurées | Adaptateur IPC unique React ; accès système et validation Rust |
| [Tauri — capabilities](https://v2.tauri.app/security/capabilities/) | Les capacités limitent les accès des fenêtres ; privilégier les permissions nécessaires | Une fenêtre locale, aucune permission shell/fs/http côté UI |
| [Tauri — AppManifest](https://docs.rs/tauri-build/latest/tauri_build/struct.AppManifest.html) | Génération de permissions pour les commandes de l’application | Liste explicite des commandes autorisées |
| [Template Tauri upstream](https://github.com/tauri-apps/create-tauri-app/tree/dev/templates) | Organisation frontend + `src-tauri`, configuration Vite et point d’entrée natif | Socle organisé comme un projet Tauri standard |
| [Tailwind — plugin Vite](https://tailwindcss.com/docs/installation/using-vite) | Intégration Tailwind via plugin dédié | Pas de pipeline CSS personnalisé superflu |
| [Plugin global-shortcut upstream](https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/global-shortcut) et [API Rust](https://docs.rs/tauri-plugin-global-shortcut/latest/tauri_plugin_global_shortcut/) | Événements Pressed/Released ; `is_registered` n’est pas un détecteur universel des raccourcis d’autres applications | Filtre anti-répétition, remplacement transactionnel et recette X11 réelle indispensable |
| [Tauri — prérequis Linux](https://v2.tauri.app/start/prerequisites/) | Rust et WebKitGTK 4.1 nécessaires pour compiler/lancer le desktop | Préflight documenté ; tests métier indépendants de la WebView |
| [JSON Schema 2020-12](https://json-schema.org/draft/2020-12) | Contrat explicite des propriétés et variantes de message | Corpus commun Rust/Python et validation stricte |
| [faster-whisper upstream](https://github.com/SYSTRAN/faster-whisper) | CPU int8 disponible ; PyAV embarque le décodage audio | Profil initial CPU, pas de dépendance FFmpeg système ajoutée sans besoin |
| [Chargement et transcription upstream](https://github.com/SYSTRAN/faster-whisper/blob/master/faster_whisper/transcribe.py) | `local_files_only` ne suffit pas si `tokenizer.json` manque ; segments évalués en partie paresseusement | Validation du modèle local avant construction et erreurs couvertes à l’appel comme à l’itération |
| [VAD upstream](https://github.com/SYSTRAN/faster-whisper/blob/master/faster_whisper/vad.py) et [assets](https://github.com/SYSTRAN/faster-whisper/blob/master/MANIFEST.in) | Modèle Silero ONNX fourni avec le paquet ; nom variable selon version | Vérifier la distribution verrouillée, aucun téléchargement VAD à la dictée |
| [CTranslate2 Whisper](https://opennmt.net/CTranslate2/python/ctranslate2.models.Whisper.html) | Pas de garantie publique d’interruption immédiate du calcul natif | Annulation coopérative, résultat neutralisé ; arrêt forcé du processus par Rust prévu en T-04 |

## Choix d’architecture

Un cœur Rust modulaire et un moteur Python enfant suffisent pour le MVP local. Des microservices réseau compliqueraient la distribution, l’annulation et la confidentialité sans répondre à un besoin du CDC.

Le modèle n’est jamais téléchargé pour les tests ou au lancement. Les tests rapides injectent le modèle au niveau de l’adaptateur ; les tests d’intégration réels sont distincts et explicitement sautés sans modèle/fixture autorisés.

## Limites de la recherche

Les branches upstream et pages `latest` évoluent. Les versions réellement retenues sont celles des fichiers de verrouillage du projet ; une API consultée ne constitue pas une preuve de fonctionnement sur Cinnamon/X11. Les mesures matérielles et la recette d’installation restent nécessaires.

## Complément préalable à T-04–T-06

- [Tokio process](https://docs.rs/tokio/latest/tokio/process/) et [Child](https://docs.rs/tokio/latest/tokio/process/struct.Child.html) : `kill_on_drop` seul ne garantit pas une récolte immédiate. Le superviseur conserve la propriété du processus et exécute `kill` puis `wait`, y compris après annulation, timeout et arrêt au démarrage.
- [CPAL 0.16](https://docs.rs/cpal/0.16.0/cpal/) et [exemple upstream de capture](https://github.com/RustAudio/cpal/blob/master/examples/record_wav.rs) : le flux appartient à un thread dédié ; le callback ne fait ni disque, ni conversion lourde, ni journalisation. La version 0.16 est fixée pour son API vérifiée ; le master upstream est différent.
- [Rubato 0.16.2](https://docs.rs/rubato/0.16.2/rubato/) : rééchantillonnage avec filtre anti-repliement et compensation de la latence du filtre, hors callback. Les tests contrôlent durée, amplitude et atténuation d’une fréquence au-dessus de Nyquist.
- [rustix openat](https://docs.rs/rustix/latest/rustix/fs/fn.openat.html) et [TempDir](https://docs.rs/tempfile/latest/tempfile/struct.TempDir.html) : permissions privées dès création, descripteurs sans suivi de liens et nettoyage explicite ; un destructeur ne couvre pas un crash. Les WAV du projet utilisent une instance privée avec marqueur et verrou pour reconnaître les fichiers abandonnés sans effacer ceux d’une instance active.

Ces sources ont été consultées avant le code des modules correspondants. Les tests de processus utilisent un vrai Python avec une implémentation de protocole de test, distincte de l’inférence faster-whisper réelle. Aucun modèle ou enregistrement personnel n’a été téléchargé.
