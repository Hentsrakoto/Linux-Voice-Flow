# Bilan T-01 à T-03 — 3 novembre 2026

## État réel

Le code des trois étapes du CDC est ajouté, avec un prototype précoce de T-06 conformément au plan. **T-02 est validé automatiquement ; la recette complète de T-01 et T-03 reste ouverte.** Aucun jalon de dictée intégrée n’est déclaré atteint : Rust ne lance pas encore le moteur Python, aucun microphone n’est capturé et aucun texte n’est injecté.

Les agents ont séparé frontend, contrats et moteur, puis une revue indépendante a examiné l’ensemble. Les accès système restent dans Rust et le moteur demeure un processus Python local ; aucun microservice réseau ajouté. La recherche préalable et ses sources officielles/GitHub sont dans [recherche-technique.md](../recherche-technique.md).

## T-01 — Socle Tauri/React

**Ajouté :** React/TypeScript strict, Vite, Tailwind, scripts de développement et vérification, configuration Tauri 2, commande Rust `get_app_info`, capacités minimales, CSP locale, icône et documentation. UI française distinguant chargement, connexion, erreur récupérable et aperçu navigateur, sans annoncer une dictée prête.

**Vérifié :** `npm run typecheck`, `npm run lint`, `npm run test -- --run` (**8 tests**), `npm run build` : succès. Cohérence du verrou avec `npm ci --dry-run --offline --ignore-scripts --no-audit --no-fund` : succès ; ce dry-run ne remplace pas une installation propre. Audit UI statique strict : zéro constat. Aperçu réellement inspecté dans le navigateur intégré, en fenêtre large et étroite ; largeur du contenu 375 px pour 375 px disponibles, sans débordement horizontal.

**À vérifier sur ton poste :**

- [ ] Installer les prérequis Rust/WebKitGTK/rsvg décrits dans [installation.md](../installation.md).
- [ ] Exécuter `npm ci`, puis `npm run tauri dev` depuis Cinnamon/X11.
- [ ] Vérifier la fenêtre, le nom « Linux Voice Flow », la version 0.1.0 et la connexion desktop.
- [ ] Vérifier Tab, focus visible, texte lisible à 200 % et absence de capture audio.

**Limite observée :** le diagnostic Tauri identifie Linux Mint 22.2, Cinnamon/X11, mais WebKitGTK 4.1 et rsvg2 manquent. Aucun paquet système installé automatiquement. `cargo check --locked --offline` avec le feature desktop est bloqué dès la dépendance `adler2` non mise en cache ; il ne valide pas le code de l’adaptateur Tauri. `tauri info` a affiché les prérequis manquants puis a été interrompu pendant les vérifications complémentaires. Le lancement natif et le build desktop complet ne sont donc pas revendiqués.

Détail frontend : [t01-frontend-report.md](t01-frontend-report.md).

## Prototype T-06 — Raccourci X11 anticipé

**Ajouté :** `Ctrl+Alt+Space`, filtrage d’auto-répétition, remplacement transactionnel, conservation de l’ancien raccourci sur échec, refus de modification pendant maintien, panneau de compteurs et état réel. Le réglage du prototype n’est pas persistant ; T-08 ajoutera la configuration sauvegardée.

**Vérifié automatiquement :** politique de raccourci, rollback et relais FIFO non bloquant. Le callback X11 n’attend jamais le verrou du gestionnaire, ce qui évite le cycle d’interblocage trouvé en revue. Le worker est arrêté puis joint après `run_return`. Les snapshots ont une révision monotone ; React ignore ceux plus anciens, y compris une réponse de remplacement tardive. Régression React reproduite puis corrigée ; test Rust de révision d’abord rouge, puis vert.

**À vérifier sur ton poste :**

- [ ] Dans le panneau « Test du raccourci X11 », maintenir et relâcher le raccourci dans un éditeur, puis un navigateur.
- [ ] Vérifier un seul appui et un seul relâchement par cycle ; un maintien prolongé n’ajoute pas de répétitions.
- [ ] Essayer une combinaison invalide ou en conflit ; l’ancienne doit rester utilisable si l’inscription échoue.
- [ ] Changer de fenêtre pendant le maintien et vérifier le relâchement ; fermer puis relancer sans état bloqué.

Ce prototype n’est **pas encore prouvé sur le serveur X11 réel**. La fiabilité de la bibliothèque selon Cinnamon et les conflits externes doit être confirmée avant T-05/T-07. Voir [décision X11](../decisions/001-raccourci-x11.md).

## T-02 — Contrats et états

**Ajouté :** schéma JSONL v1, fixtures communes, encodeurs/parseurs stricts Rust/Python, décodeur borné à 1 Mio, erreurs normalisées, types UI et machine à états pure. Les identifiants empêchent les réponses périmées/doubles de planifier une insertion. `can_insert` est la garde que le futur contrôleur devra vérifier au dernier moment.

**Vérifié :** suite Rust sans WebView : **18 tests réussis** couvrant aussi le socle et le raccourci ; tests protocole Python : **5 tests et 69 sous-tests réussis**. Formatage Rust et Clippy `--all-targets -- -D warnings` sans feature desktop : succès. Tests indépendants de microphone, modèle et affichage graphique.

**À vérifier/reproduire :**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked
.venv/bin/python -m pytest engine/tests/test_protocol.py -q
```

Aucun matériel nécessaire. Les timers, la génération des identités et l’exécution des effets appartiennent au futur contrôleur. Détail : [t02-report.md](t02-report.md), [protocole](../protocole.md).

## T-03 — Moteur Python

**Ajouté :** CLI locale, modèle CTranslate2 local complet obligatoire avec tokenizer, profil CPU/int8, VAD, langues fr/en/auto, WAV privés validés, une inférence à la fois et boucle de contrôle disponible pendant le calcul. Annulation neutralisant le résultat ; acquittement uniquement après arrêt du worker. Erreurs constantes sans recopier audio, transcription ou chemins dans les journaux.

**Vérifié :** `.venv/bin/python -m pytest -q` : **36 réussis, 1 sauté, 69 sous-tests réussis** ; `compileall` et `pip check` passent pour l’environnement de test installé. Le test sauté exige un modèle et deux fichiers vocaux fournis explicitement. Les tests rapides injectent l’adaptateur du modèle ; ils ne prouvent pas l’inférence native. Asset VAD vérifié dans la wheel faster-whisper 1.2.1. Versions transitives runtime/dev fixées ; les contraintes des métadonnées ont été vérifiées. L’installation/import de tout le runtime reste à exécuter sur la cible.

**À vérifier sur ton poste :**

- [ ] Installer `engine/requirements.lock.txt`, puis `engine/requirements-dev.lock.txt` dans `.venv`.
- [ ] Vérifier `pip check` et le chargement de l’asset VAD selon [le rapport moteur](t03-report.md).
- [ ] Préparer un modèle CTranslate2 local complet et deux WAV fr/en autorisés : mono, PCM 16 bits, 16 kHz, 60 s maximum.
- [ ] Définir `LVF_TEST_MODEL_PATH`, `LVF_TEST_FR_WAV`, `LVF_TEST_EN_WAV`, couper le réseau et exécuter `.venv/bin/python -m pytest -m local_model -v`.
- [ ] Vérifier le sens de la phrase française et anglaise, l’absence de résultat pour le silence et l’absence de connexion réseau système.
- [ ] Mesurer le temps pour 10 s d’audio ; ne pas confondre durée audio `duration_ms` et durée de traitement.

**Limites :** aucun modèle ou enregistrement personnel téléchargé. Le calcul natif courant peut rester bloqué malgré une annulation coopérative ; T-04 devra appliquer le timeout de 3 s et terminer/récolter le processus. Les diagnostics natifs non filtrés sont neutralisés, ce qui réduit leur richesse pour le dépannage. Détails et provenance des fixtures générées : [t03-report.md](t03-report.md), [tests moteur](../../engine/tests/README.md).

## Revue et prochaine étape

La [revue indépendante](revue-finale.md) a confirmé la correction de la réponse IPC périmée ; aucun constat actionnable ouvert dans son périmètre statique. Cela ne remplace pas la recette matérielle ci-dessus.

Prochaine étape du développement : T-04, supervision du moteur Python depuis Rust, après préparation du poste et validation des points matériels manquants. La capture et l’insertion resteront séparées conformément au plan.
