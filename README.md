# Linux Voice Flow

Application de dictée locale ou via Groq pour Linux Mint Cinnamon/X11 : Tauri 2, React/TypeScript, cœur Rust et moteur Python.

**État : T-01 à T-08 implémentés, recette native finale en cours.** Les vues **Dictée** et **Réglages** sont séparées : raccourci actif et résultat éphémère sur Dictée ; fournisseur Local/Groq, modèles, microphone, langue, raccourci, insertion et diagnostic dans Réglages. L’insertion X11 vérifie la cible ; en cas d’échec, le résultat reste récupérable par copie explicite ou copie automatique consentie. Les réglages sont sauvegardés et restaurés. La dictée réelle au microphone dans un éditeur et un navigateur sur le poste cible reste à recetter avant de déclarer J2 acquis.

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

Avant de préparer **Local ou Groq**, installez le moteur Python du projet (Groq utilise aussi la détection de parole locale) :

```bash
python3 -m venv .venv
.venv/bin/python -m pip install -r engine/requirements.lock.txt
```

Dans **Réglages**, cliquez ensuite sur **Enregistrer et préparer**. Le raccourci ne démarre la capture qu’après l’état **Prêt à dicter**.

Voir les prérequis Rust/WebKitGTK dans le guide d’installation. Aucun modèle n’est téléchargé par l’application. Le moteur Python se teste séparément avec un modèle local préparé explicitement.

Les modèles ne sont pas inclus dans Git. Pour installer explicitement Tiny ou Base :

```bash
.venv/bin/python scripts/install_models.py tiny
.venv/bin/python scripts/install_models.py base
```

Dans la fenêtre Tauri, ouvrez **Réglages**, choisissez le service et le modèle ou son dossier personnalisé, puis **Enregistrer et préparer**. Revenez dans **Dictée** et attendez **Prêt à dicter**. Placez le curseur dans votre application cible, maintenez le raccourci pour parler et relâchez-le pour transcrire et insérer. Les choix enregistrés sont restaurés au prochain lancement.

Pour Groq, renseignez `GROQ_API_KEY` et `GROQ_MODEL` dans `.env` (modèle de fichier : `.env.exemple`). Dans **Réglages**, sélectionnez **Service de transcription → Groq — API**, puis **Enregistrer et préparer**. Seul ce mode envoie l’audio à Groq ; le mode local ne nécessite aucune clé. Voir [le guide Groq](docs/groq.md).

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
