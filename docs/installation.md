# Installation de développement

## Cible et prérequis

Cible : Linux Mint, Cinnamon sous **X11**. Wayland n’est pas une plateforme garantie par ce MVP.

- Node.js compatible avec les versions verrouillées de Vite/Vitest ; environnement de développement observé : Node **24.14.0**, npm **11.9.0**.
- Rust **1.90 ou plus récent**, Cargo ; installer avec [rustup officiel](https://rustup.rs/) si nécessaire. Le plugin raccourci peut relever son minimum : vérifier le verrou Cargo.
- Python **3.12** pour le verrou initial du projet ; environnement observé : **3.12.3**.
- Dépendances natives décrites par [Tauri](https://v2.tauri.app/start/prerequisites/), notamment WebKitGTK **4.1**, GTK3, OpenSSL et outils de compilation.

Pour Mint basé sur Ubuntu/Debian, installer les paquets système nécessaires sur le poste de développement :

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libasound2-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev python3-venv pkg-config xdotool xclip
```

Cette commande est destinée à l’utilisateur ; elle n’a pas été exécutée automatiquement sur le système pendant l’implémentation.

Vérifier :

```bash
node --version
npm --version
rustc --version
cargo --version
python3 --version
pkg-config --modversion gtk+-3.0 webkit2gtk-4.1
printenv XDG_SESSION_TYPE
```

Lancer le desktop depuis une session graphique X11. Un test réussi dans un navigateur ne prouve pas le fonctionnement du raccourci global.

## Installer Rust durablement

Les répertoires `/tmp/lvf-cargo` et `/tmp/lvf-rustup` utilisés pour les premières vérifications étaient temporaires. Ils peuvent disparaître après un redémarrage. Ne pas les utiliser pour le lancement quotidien.

Si `cargo` est introuvable, installer Rust dans le compte utilisateur avec [l’installateur officiel](https://rust-lang.org/tools/install/) :

```bash
unset RUSTUP_HOME CARGO_HOME
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup-install.sh
sh /tmp/rustup-install.sh -y --profile minimal --default-toolchain stable
source "$HOME/.cargo/env"
cargo --version
rustc --version
```

Rust et Cargo sont alors conservés dans `~/.rustup` et `~/.cargo`. Si le terminal courant utilisait encore les anciens exports, exécuter `unset RUSTUP_HOME CARGO_HOME` puis `source "$HOME/.cargo/env"` avant `npm run tauri dev`. L’installation ne met pas à jour les variables d’un terminal déjà ouvert. Le premier lancement peut encore télécharger les dépendances Cargo et compiler le projet.

### Correction du poste actuel — 10 novembre 2026

La chaîne Rust utilisée par les tests dans `/tmp/lvf-cargo` et `/tmp/lvf-rustup` a été déplacée durablement vers `~/.cargo` et `~/.rustup`. `~/.cargo/env` charge le nouveau chemin ; `~/.zshrc` le source pour les nouveaux terminaux. Une sauvegarde de ce dernier a été conservée dans `~/.zshrc.lvf-backup-2026-11-10`. Cargo/Rust 1.99.1 et `cargo metadata --no-deps --format-version 1` ont été vérifiés dans un nouveau zsh interactif.

Dans un terminal déjà ouvert, exécuter `source "$HOME/.cargo/env"` avant de relancer Tauri. Sur ce poste, les paquets de développement WebKit/GTK, ALSA, libxdo, appindicator et librsvg ainsi que xdotool doivent encore être installés avec la commande sudo ci-dessus. La résolution de Cargo ne remplace pas ces prérequis natifs.

## Frontend et Tauri

```bash
npm ci
npm run dev
```

Le port Vite est fixé à **1420**. Il doit être libre. Le serveur est local ; aucun service réseau de transcription n’est démarré.

```bash
npm run tauri dev
```

Ouvrez **Réglages** pour choisir Local/Groq, le modèle local, le microphone, la langue et le raccourci. Cliquez sur **Enregistrer et préparer**. Les paramètres sont conservés dans `settings.json` du répertoire de configuration Tauri (`app_config_dir()`), avec les permissions 0600 ; ils ne contiennent ni clé API, ni audio, ni transcription.

Dans **Dictée**, attendez « Prêt à dicter », placez le curseur dans le champ d’une application externe, maintenez le raccourci pour parler, puis relâchez. L’insertion automatique exige que la fenêtre et le focus X11 n’aient pas changé. Le texte est aussi disponible pendant 120 secondes dans l’application. En cas d’insertion incertaine, vérifiez le champ avant de copier et coller pour éviter les doublons. La copie automatique est désactivée par défaut ; le bouton **Copier** est une action explicite. **Effacer ma copie du presse-papiers** agit uniquement sur le processus propriétaire de notre copie ; un gestionnaire tiers peut conserver son historique.

Le diagnostic est accessible dans **Réglages → Diagnostic de l’application**. Les binaires **xdotool** et **xclip** sont nécessaires respectivement à l’insertion et à la copie. Sous Wayland, ce parcours global n’est pas garanti. Un changement de champ dans une même fenêtre peut rester indétectable : désactivez l’insertion automatique si nécessaire.

Pour Groq, suivez [la configuration API](groq.md) : renseignez `GROQ_API_KEY` et `GROQ_MODEL` dans `.env`, puis sélectionnez Groq dans Réglages et enregistrez. Seul ce mode transmet l’audio au fournisseur ; le service sauvegardé est préparé au prochain lancement, sans démarrer le microphone.

Contrôles :

```bash
npm run typecheck
npm run lint
npm run test -- --run
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
cargo check --manifest-path src-tauri/Cargo.toml
```

Le feature `desktop` est activé par défaut. Le désactiver permet de tester les contrats et transitions sans GTK/WebKit ; cela ne remplace pas la compilation desktop.

## Moteur Python

```bash
python3 -m venv .venv
.venv/bin/python -m pip install -r engine/requirements.lock.txt
.venv/bin/python -m pip install -r engine/requirements-dev.lock.txt
.venv/bin/python -m pytest engine/tests
```

Les dépendances s’installent pendant la préparation, avec réseau explicite. Le fonctionnement du moteur doit ensuite rester hors ligne.

### Préparer le modèle

Le projet propose un installateur explicite pour les modèles multilingues **Whisper Tiny** et **Whisper Base** :

```bash
.venv/bin/python scripts/install_models.py
```

Les fichiers sont conservés dans `models/tiny` et `models/base`. Dans Tauri, actualiser les modèles si la fenêtre était déjà ouverte, choisir « Modèle de transcription », puis « Préparer le moteur ». Changer la sélection ne remplace le modèle actif qu’après cette préparation. Voir [les vérifications hors ligne et les détails](modeles.md).

Importer un répertoire de modèle **CTranslate2 compatible faster-whisper**, pas un simple fichier GGML/GGUF. Fournir au minimum `model.bin`, `config.json`, `tokenizer.json` et les ressources du modèle distribué. Le tokenizer doit être présent localement pour empêcher tout repli distant. Respecter les conditions de distribution du modèle.

Ne pas mettre de modèle ou d’enregistrement personnel dans Git. Aucun modèle n’est téléchargé pour les tests par défaut.

Le moteur se lance depuis la racine du dépôt avec les arguments documentés par :

```bash
.venv/bin/python -m engine.main --help
```

Il émet `ready` sur stdout uniquement après chargement effectif. stdout est réservé au protocole ; ne pas y ajouter de `print` de diagnostic. Le moteur reçoit des WAV depuis un répertoire audio privé explicitement désigné ; il ne capture pas le microphone lui-même.

## Limites de validation de l’environnement initial

Au démarrage du travail : GTK3 présent ; Rust/Cargo et WebKitGTK 4.1 absents. Une chaîne Rust temporaire dans `/tmp` sert aux tests exécutés par l’agent ; ce chemin n’est pas un prérequis durable du projet et ne doit pas être copié comme configuration utilisateur.

Le bilan détaille les commandes réellement exécutées et distingue tests automatisés, build frontend et recette desktop/modèle réelle. Pas de paquet final à ce stade ; l’empaquetage du moteur relève de T-10.


## Parcours intégré disponible après T-04–T-06

Le développement utilise le Python de `.venv/bin/python` à la racine du dépôt. Installer les dépendances runtime avant de préparer le modèle :

```bash
.venv/bin/python -m pip install -r engine/requirements.lock.txt
.venv/bin/python -m pip check
```

1. Lancer `npm run tauri dev` dans le terminal où `cargo --version` fonctionne.
2. Dans **Réglages**, sélectionner le service et le modèle local ou saisir son chemin **absolu et canonique**, sans symlink ; choisir microphone et langue, puis **Enregistrer et préparer**.
3. Attendre « Prêt à dicter ». Aucun microphone n’est ouvert pendant cette préparation.
4. Maintenir le raccourci actif (initialement `Ctrl+Alt+Space`), parler puis relâcher. Le texte est inséré si la cible X11 est inchangée ; il reste également visible dans la zone de résultat.
5. Tester « Annuler » pendant la capture et la transcription ; Échap est également demandé globalement pendant les opérations actives. Avec les modificateurs maintenus, utiliser aussi ces modificateurs avec Échap. Si l’inscription d’Échap échoue, la notice indique d’utiliser le bouton.
6. Vérifier « Effacer » et l’effacement automatique du résultat après 120 secondes. Les réglages sont persistants depuis T-08 ; le formulaire en cours est également conservé lors des allers-retours entre Dictée et Réglages.

Les WAV sont privés dans le sous-répertoire `audio` du cache utilisateur Tauri, ouverts seulement après relâchement pour le transport Python et supprimés à chaque issue. Une instance possède son marqueur/verrou ; les restes d’une instance abandonnée sont nettoyés au prochain démarrage. Le chemin exact suit `app_cache_dir()` de Tauri ; aucun chemin audio n’est envoyé à React.

Nouveau prérequis de compilation audio sous Mint : **`libasound2-dev`**. Pendant la vérification de l’agent, le paquet officiel a été extrait dans un SDK temporaire `/tmp` et lié à la bibliothèque ALSA du système, sans installation système. Pour le poste de développement, utiliser le paquet système normal, pas ce SDK temporaire.
