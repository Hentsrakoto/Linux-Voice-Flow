# Linux Voice Flow

Application de dictée locale ou via Groq pour Linux Mint Cinnamon/X11 : Tauri 2, React/TypeScript, cœur Rust et moteur Python.

**État : T-01 à T-06 implémentés, avec choix Local/Groq.** La fenêtre permet de préparer un modèle local ou le service Groq, choisir un microphone et dicter par maintien du raccourci. Le texte est affiché dans l’application ; l’insertion dans une autre fenêtre reste prévue en T-07. Le parcours réel microphone/modèle/X11 et la transcription avec une vraie clé Groq restent à recetter sur le poste cible.

- [Cahier des charges](CAHIER_DES_CHARGES_LINUX_VOICE_FLOW.md)
- [Plan et suivi](docs/developpement/plan.md)
- [Installation et vérifications](docs/installation.md)
- [Installer et choisir les modèles locaux](docs/modeles.md)
- [Configurer Groq avec une clé API](docs/groq.md)
- [Recherches techniques](docs/recherche-technique.md)
- [Contrat du moteur](docs/protocole.md)
- [Bilan T-01 à T-03](docs/developpement/bilan-t01-t03.md)
- [Bilan T-04 à T-06 et contrôles manuels](docs/developpement/bilan-t04-t06.md)

## Démarrer

```bash
npm ci
npm run dev
```

Le navigateur affiche un aperçu de l’interface ; les fonctions natives nécessitent Tauri :

```bash
npm run tauri dev
```

Voir les prérequis Rust/WebKitGTK dans le guide d’installation. Aucun modèle n’est téléchargé par l’application. Le moteur Python se teste séparément avec un modèle local préparé explicitement.

Whisper Tiny est installé dans cette copie du projet. Pour ajouter explicitement Base :

```bash
.venv/bin/python scripts/install_models.py base
```

Dans la fenêtre Tauri, choisissez **Modèle de transcription**, puis **Préparer le moteur**. Tiny est proposé par défaut. Pour changer de modèle, sélectionnez Base puis préparez à nouveau le moteur. Le libellé **Modèle actif** confirme le modèle chargé. Un dossier personnalisé reste possible ; le choix reste limité à la session.

Pour Groq, renseignez `GROQ_API_KEY` et `GROQ_MODEL` dans `.env` (modèle de fichier : `.env.exemple`). Sélectionnez **Service de transcription → Groq — API**, puis **Préparer le moteur**. Seul ce mode envoie l’audio à Groq ; le mode local ne nécessite aucune clé. Voir [le guide Groq](docs/groq.md).

## Vérifications

```bash
npm run typecheck
npm run lint
npm run test -- --run
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
python3 -m venv .venv
.venv/bin/python -m pip install -r engine/requirements-dev.lock.txt
.venv/bin/python -m pytest engine/tests
```

Le mode Rust `--no-default-features` vérifie le métier sans WebView ; il ne valide pas le lancement desktop. Les tests avec modèle réel sont explicitement séparés des tests rapides.
