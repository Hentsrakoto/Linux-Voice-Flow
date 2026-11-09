# T-05 — Capture et WAV privés

## Implémentation

`audio.rs` expose `AudioBuffer` (mono PCM16/16 kHz), `AudioError` (`code`, `message`) et `Microphone`. La conversion valide les échantillons finis, moyenne les canaux, limite l'amplitude et applique le filtre sinc de rubato 0.16.2 hors callback. Le délai du filtre est compensé et la durée conservée à l'échantillon près. Les buffers détenus par l'application sont effacés avec zeroize ; le resampler est réinitialisé après conversion.

`audio_native.rs`, activé par `native-audio`, conserve le flux CPAL 0.16.0 sur un thread propriétaire. Les appels passent par canaux ; aucun `unsafe impl Send` n'est utilisé. Aucun flux n'existe au repos. Le callback ne fait ni écriture disque, ni rééchantillonnage, ni journalisation ; il écrit uniquement dans un buffer mono préalloué, avec verrou non bloquant. Un dépassement, une donnée invalide, une contention ou une erreur CPAL invalide tout l'enregistrement. La borne brute est `60 × fréquence × sizeof(f32)` (46 080 000 octets maximum à 192 kHz), indépendante du nombre de canaux ; le PCM16 final est limité à 1 920 000 octets.

Le callback refuse également toute donnée à partir de l’échéance monotone. Le thread vérifie erreur/échéance toutes les 5 ms et détruit le flux indépendamment du contrôleur (à l’ordonnancement du système près). Une limite atteinte invalide la session, sans produire de clip. `stop` détruit le flux avant conversion ; `discard` et `Drop` le détruisent et effacent les données. Les identifiants `name:<nom>` résistent à un réordonnancement de l'énumération ; les noms ambigus sont refusés. `default` sélectionne le microphone par défaut du système.

`temp_audio.rs` ouvre chaque composant absolu avec `openat/O_NOFOLLOW`, crée les répertoires en 0700 et les WAV en 0600 dès leur création, et vérifie propriétaire/permissions. Chaque instance a un sous-répertoire privé et un marqueur verrouillé avec `flock`. Les handles retirent leur WAV explicitement ou à leur destruction ; ils maintiennent le répertoire vivant même après destruction du store. La récupération après crash ne traite que des répertoires appartenant à l'utilisateur, privés, portant un marqueur exact, sans verrou actif et ne contenant que les noms WAV attendus. Les symlinks, fichiers étrangers et instances actives sont conservés. Rust reste seul propriétaire de la suppression ; Python lit sous la racine privée commune.

## Vérification

Tests synthétiques : silence court, durée 16/44,1/48 kHz, downmix/clipping, données non finies, débordement et invalidation, amplitude/fréquence d'un sinus 1 kHz et atténuation des alias 12 kHz ; permissions, nettoyage explicite/Drop, collisions, récupération après crash, instance active, fichiers étrangers, chemin relatif et symlinks.

Vérifications exécutées avec Rust 1.99 installé temporairement :

- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked --offline` : 30 tests unitaires et 12 tests de processus moteur réussis lors du premier passage complet ; deux tests de stockage supplémentaires ont ensuite réussi dans le passage natif complet.
- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked --offline audio` : 11 tests audio/stockage réussis.
- `PKG_CONFIG_PATH=/tmp/lvf-native-sysroot/usr/lib/x86_64-linux-gnu/pkgconfig cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features native-audio --locked --offline` : 34 tests unitaires et 12 tests de processus moteur réussis, sans avertissement. Cette configuration compile et lie CPAL/ALSA ; les deux tests natifs n'ouvrent aucun périphérique (thread au repos et session synthétique d'échéance/erreur).
- `rustfmt --edition 2021` sur les trois fichiers Rust modifiés.

Le SDK ALSA utilisé pour cette vérification provient du paquet officiel `libasound2-dev` extrait dans `/tmp` ; la bibliothèque runtime utilisée est celle de l'hôte. L'environnement ne disposait initialement pas des métadonnées de développement ALSA. Un premier téléchargement Cargo en sandbox a échoué par résolution DNS ; les dépendances ont ensuite été téléchargées avec le réseau autorisé. Aucun faux résultat de compilation ou de capture n'en est déduit.

Aucune capture microphone réelle n'a été tentée automatiquement. La recette matérielle reste à réaliser volontairement sous Cinnamon/X11 (appui court, maintien, relâchement, annulation, déconnexion et limite de 60 s) ; les tests synthétiques ne prouvent pas le comportement du matériel ni du serveur audio.

## Références

Recherche menée avant implémentation : [CPAL 0.16.0](https://docs.rs/cpal/0.16.0/cpal/), [API DeviceTrait](https://docs.rs/cpal/0.16.0/cpal/traits/trait.DeviceTrait.html), [SincFixedIn rubato 0.16.2](https://docs.rs/rubato/0.16.2/rubato/struct.SincFixedIn.html), [tempfile et nettoyage](https://docs.rs/tempfile/latest/tempfile/), [rustix openat](https://docs.rs/rustix/latest/rustix/fs/fn.openat.html). Implémentation originale ; aucun exemple amont copié.


## Complément final du coordinateur

Après intégration et corrections de revue : suite desktop complète **42 tests unitaires +12 tests processus réussis**, Clippy desktop et sans desktop avec `-D warnings` réussis, formatage réussi. Frontend **20 tests réussis**, TypeScript/ESLint/build réussis ; Python **36 réussis, 1 modèle réel sauté, 69 sous-tests réussis**. Les compteurs intermédiaires ci-dessus décrivent leur passage historique. Le bilan consolidé `bilan-t04-t06.md` contient les commandes, le timeout de démarrage Vitest transitoire et les recettes matérielles encore ouvertes. La zone de résultat finale suit la règle de hauteur stable `resize-none` avec défilement interne, documentée dans UX-CONTRACT.md ; les décisions de sélecteurs natifs sont persistées dans premium-ui.json. Aucun succès microphone/modèle réel n’est revendiqué.
