# Bilan T-04 à T-06 — 3 novembre 2026

## Disponible maintenant

Le raccourci global pilote la capture, puis Rust transmet un WAV privé au moteur Python local. Le résultat est affiché dans l’application et peut être effacé. **L’insertion dans une autre fenêtre n’est pas encore ajoutée** : elle relève de T-07. La sauvegarde des réglages relève de T-08.

Le code desktop est compilé et les tests natifs sont verts. La recette réelle microphone + modèle + Cinnamon/X11 reste ouverte : aucun microphone n’a été ouvert automatiquement et aucun modèle n’a été téléchargé pendant ce travail. Les jalons J1/J2 ne sont donc pas revendiqués sur la seule base des simulations.

## T-04 — Supervision Python

Ajout de `src-tauri/src/engine.rs`, des tests `src-tauri/tests/engine_process.rs` et d’un faux moteur Python de test. Le lancement n’utilise aucun shell. La boucle reste disponible pendant le chargement ; `launch` fournit immédiatement un handle annulable. Les lectures et files sont bornées, stdout est validé par le protocole v1 et stderr ne divulgue pas de texte ou de chemins.

Démarrage limité à 60 secondes, transcription à 120 secondes et acquittement d’annulation à 3 secondes. Après échec ou fermeture, le processus est tué puis récolté ; aucun redémarrage automatique infini. L’utilisateur peut préparer à nouveau le moteur. Les résultats inconnus, tardifs ou annulés ne sont pas acceptés.

**Vérifié :** 12 tests d’intégration par vrais processus Python couvrent succès, moteur occupé, crash/EOF, messages invalides ou trop grands, délais, stdin bloqué, annulation, fermeture pendant le démarrage, abandon du dernier handle et pression sur la file de résultats. Les tests vérifient aussi la disparition du PID.

**À vérifier volontairement :**

- [ ] Installer les dépendances runtime Python et préparer un modèle local complet.
- [ ] Cliquer « Préparer le moteur » et attendre « Prêt à dicter ».
- [ ] Annuler pendant le chargement et vérifier la possibilité de préparer à nouveau.
- [ ] Fermer l’application pendant le chargement ou une transcription ; aucun processus Python lié ne doit rester.

Rapport détaillé : [t04-report.md](t04-report.md).

## T-05 — Capture et audio privé

Ajout de `audio.rs`, `audio_native.rs`, `temp_audio.rs` et de leurs tests. CPAL possède son flux sur un thread dédié. Aucun flux n’est ouvert au repos. Le callback stocke un signal mono borné ; la conversion et le disque sont exécutés après arrêt du flux. Rubato effectue le rééchantillonnage avec anti-repliement vers WAV mono PCM16 à 16 kHz.

La limite de 60 secondes est appliquée indépendamment du contrôleur et abandonne le clip sans transcription. Les erreurs de périphérique ou de buffer invalident toute la capture. Les tampons contrôlés sont effacés via zeroize. Les WAV sont créés en 0600 dans des répertoires 0700, sans suivre de symlinks. Un marqueur verrouillé identifie les instances ; le nettoyage de crash préserve les instances actives et les fichiers étrangers. L’écriture WAV est regroupée et son flush vérifié.

**Vérifié :** conversion de silence/tonalités, durée et amplitude, atténuation d’un signal de 12 kHz au rééchantillonnage, débordement, limite et erreur synthétique, permissions, collisions, symlinks, crash et durée de vie des handles. Les tests natifs construisent des sessions synthétiques et un propriétaire inactif ; ils ne capturent pas le microphone réel.

**À vérifier volontairement :**

- [ ] Choisir le microphone, maintenir le raccourci et vérifier que le microphone devient actif seulement pendant l’appui.
- [ ] Relâcher : la capture doit cesser et la transcription commencer ; vérifier le texte.
- [ ] Tester Échap et le bouton Annuler ; aucun texte de la session annulée ne doit apparaître.
- [ ] Maintenir au-delà de 60 secondes : message explicite, audio abandonné, aucune transcription.
- [ ] Débrancher/désactiver le microphone : erreur récupérable et aucun enregistrement bloqué.
- [ ] Contrôler le cache après succès, erreur et annulation : aucun WAV de la session terminé ne doit rester.

Rapport détaillé : [t05-report.md](t05-report.md).

## T-06 — Parcours push-to-talk et intégration Tauri

Le relais filtre l’auto-répétition puis transmet les appuis/relâchements au contrôleur `dictation.rs`. Une génération attachée aux événements invalide les appuis encore en file après annulation ou saturation ; une perte de relâchement ne peut donc pas déclencher ensuite une capture retardée. Toute invalidation arrête également le microphone.

Échap simple et Échap avec les modificateurs du raccourci sont inscrits uniquement pendant les opérations actives. En cas de conflit, une notice invite à utiliser Annuler. Échap est réservé à l’annulation et refusé comme touche de dictée. Les opérations du plugin ne tiennent pas le verrou du gestionnaire depuis le callback X11.

Cinq commandes locales et leurs permissions Tauri relient la fenêtre au contrôleur. Les instantanés révisés empêchent les réponses anciennes d’écraser un état récent. Le résultat est sélectionnable, conservé 120 secondes au maximum dans l’interface et effaçable. Les messages UI sont dérivés de codes/motifs contrôlés ; les exceptions brutes ne sont pas affichées.

Un écran de préparation limité à la session permet de choisir modèle, microphone et langue pour tester ce parcours. Il ne constitue pas la configuration persistante complète de T-08.

**Vérifié :** compilation desktop, tests de politique Échap, contrôle de session et scénario intégré capture synthétique → WAV privé → vrai processus Python de test → aperçu → effacement → suppression des fichiers et récolte du processus. Ce scénario ne prouve pas la qualité d’inférence faster-whisper ni le comportement matériel X11.

**À vérifier volontairement :**

- [ ] Maintien et relâchement depuis un éditeur et un navigateur Cinnamon/X11.
- [ ] Un maintien prolongé produit une seule session ; un nouvel appui pendant la transcription n’en lance pas une autre.
- [ ] Changer le raccourci : le diagnostic doit indiquer la combinaison réellement active ; un échec conserve la précédente.
- [ ] Tester Échap tout en maintenant les modificateurs du raccourci, puis sans modificateurs pendant la transcription.
- [ ] Faire le parcours fr/en/silence avec le réseau coupé après préparation ; vérifier le sens des phrases et mesurer la latence.

Rapport détaillé : [t06-report.md](t06-report.md).

## Lancer et tester sur le poste

Depuis la racine du projet, dans le terminal où `cargo --version` fonctionne :

```bash
sudo apt install libasound2-dev
.venv/bin/python -m pip install -r engine/requirements.lock.txt
npm run tauri dev
```

Dans la fenêtre, choisir le répertoire absolu canonique d’un modèle CTranslate2 complet avec tokenizer, le microphone et la langue. Cliquer « Préparer le moteur ». Le raccourci initial est `Ctrl+Alt+Space` ; maintenir pour parler, relâcher pour transcrire. Le texte reste dans Voice Flow, sans collage automatique.

Les étapes ci-dessus impliquent une capture volontaire lors de l’appui. L’import du modèle reste manuel : aucun téléchargement automatique. Si Rust n’est disponible que dans la chaîne temporaire des tests, voir les consignes du chat ou installer Rust durablement ; ce n’est pas une dépendance livrée avec le produit.

## Vérification finale exécutée

- `cargo test --manifest-path src-tauri/Cargo.toml --locked --offline` avec fonctionnalités desktop : **42 tests unitaires + 12 tests moteur réussis** ; aucun échec.
- `cargo check` desktop : succès.
- Clippy desktop et sans fonctionnalités desktop, tous targets avec `-D warnings` : succès.
- Formatage Rust : succès.
- TypeScript, ESLint, build Vite : succès.
- Vitest : **20 tests réussis**, y compris explications « aucune parole »/limite et absence d’affichage de diagnostics bruts.
- Python : **36 tests réussis, 1 test de modèle réel sauté, 69 sous-tests réussis**.
- Audit UI statique strict : zéro constat, décisions de sélecteurs natifs et textarea documentées dans `UX-CONTRACT.md`/`premium-ui.json`.
- Aperçu navigateur effectivement ouvert ; aucune capture possible depuis cet aperçu. Le panneau natif complet reste à tester dans la fenêtre Tauri.

Les commandes Cargo natives de l’agent utilisaient le SDK officiel `libasound2-dev` extrait sous `/tmp` et la bibliothèque ALSA système, avec `PKG_CONFIG_PATH` adapté. Aucun paquet système n’a été installé par l’agent. Sur le poste, installer le paquet normal indiqué ci-dessus.

Un premier lancement Vitest en parallèle des compilations natives a échoué avant exécution des tests avec `Timeout starting forks runner`. Le passage limité à un worker, puis un nouveau passage **standard** sans compilation concurrente ont tous deux exécuté les 20 tests avec succès. Ce résultat transitoire n’a pas été compté comme une suite verte.

## Revue et limites

La revue a trouvé deux erreurs de cycle de vie du contrôleur, reproduites par tests rouges puis corrigées : capture restant active après panne moteur, et appui encore en file après relâchement perdu. Deux régressions supplémentaires couvrent la neutralisation d’un résultat après annulation et le refus d’un acquittement ancien. La contre-revue indépendante confirme aussi la correction des deux défauts UI (cause masquée et ancienne combinaison affichée).

Voir [revue indépendante](revue-t04-t06.md). Aucun constat actionnable ouvert dans ce périmètre. Restent la recette matérielle, le modèle réel, la mesure des 300 ms/latence, puis T-07 insertion/focus/fallback. Les réglages ne persistent pas encore et le chemin du moteur reste celui du dépôt de développement ; le conditionnement distribuable appartient à T-10.
