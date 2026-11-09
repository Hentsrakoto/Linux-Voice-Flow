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
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libasound2-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev python3-venv pkg-config
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

## Frontend et Tauri

```bash
npm ci
npm run dev
```

Le port Vite est fixé à **1420**. Il doit être libre. Le serveur est local ; aucun service réseau de transcription n’est démarré.

```bash
npm run tauri dev
```

La fenêtre indique « Linux Voice Flow » et propose la préparation de la dictée. Choisir un modèle local, le microphone et la langue, puis cliquer sur « Préparer le moteur ». Quand le moteur est prêt, maintenir le raccourci pour parler et le relâcher pour transcrire. Le résultat apparaît uniquement dans l’application ; l’insertion dans les autres fenêtres relève de T-07.

Pour utiliser le service externe Groq, suivez [la configuration API](groq.md) : copiez `.env.exemple` en `.env`, renseignez `GROQ_API_KEY` et `GROQ_MODEL`, puis choisissez Groq et préparez le moteur. Le mode local reste proposé au démarrage ; seul le mode Groq transmet l’audio au fournisseur.

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
2. Dans « Dictée locale », saisir le chemin **absolu et canonique**, sans symlink, du répertoire CTranslate2 complet, choisir microphone et langue, puis « Préparer le moteur ».
3. Attendre « Prêt à dicter ». Aucun microphone n’est ouvert pendant cette préparation.
4. Maintenir le raccourci actif (initialement `Ctrl+Alt+Space`), parler puis relâcher. Lire le texte dans la zone de résultat.
5. Tester « Annuler » pendant la capture et la transcription ; Échap est également demandé globalement pendant les opérations actives. Avec les modificateurs maintenus, utiliser aussi ces modificateurs avec Échap. Si l’inscription d’Échap échoue, la notice indique d’utiliser le bouton.
6. Vérifier « Effacer » et l’effacement automatique du résultat après 120 secondes. Les réglages du panneau sont limités à la session ; leur sauvegarde viendra en T-08.

Les WAV sont privés dans le sous-répertoire `audio` du cache utilisateur Tauri, ouverts seulement après relâchement pour le transport Python et supprimés à chaque issue. Une instance possède son marqueur/verrou ; les restes d’une instance abandonnée sont nettoyés au prochain démarrage. Le chemin exact suit `app_cache_dir()` de Tauri ; aucun chemin audio n’est envoyé à React.

Nouveau prérequis de compilation audio sous Mint : **`libasound2-dev`**. Pendant la vérification de l’agent, le paquet officiel a été extrait dans un SDK temporaire `/tmp` et lié à la bibliothèque ALSA du système, sans installation système. Pour le poste de développement, utiliser le paquet système normal, pas ce SDK temporaire.
