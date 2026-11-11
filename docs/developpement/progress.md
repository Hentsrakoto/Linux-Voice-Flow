# SDD ledger — plan: docs/developpement/plan.md

## Périmètre et méthode

Demande du 3 novembre 2026 : implémenter T-01 à T-03 du CDC avec agents, recherche préalable et bilan de chaque étape. Le prototype T-06 est traité tôt conformément au plan, avec recette X11 séparée.

Aucun dépôt Git utilisable : le dossier .git est vide et protégé par l’environnement. Travail dans le workspace demandé, sans création de worktree ni commit ; rapports de revue conservés dans docs/developpement.

| Interface ou lot | Vérification préalable | Décision |
|---|---|---|
| T-01 seul | Socle React/Tauri conforme au CDC ; le produit n’est pas encore prêt à dicter | UI initiale honnête, sans enregistrement simulé |
| T-02 seul | Contrat et état testables sans microphone ni WebView | Modules métier Rust séparés du feature desktop |
| T-03 seul | Modèle local et annulation native potentiellement non interruptible | Boucle JSONL disponible pendant calcul ; supervision forcée Rust prévue T-04 |
| T-01 → T-02 | Types UI et commandes Rust | AppInfo distinct des types de dictée ; contrats du moteur non exposés en commandes shell |
| T-02 → T-03 | Schéma, messages, parsing strict, acquittement d’annulation | Python consomme engine/protocol.py ; même corpus JSONL que Rust |
| T-06 → capture future | Appui/relâchement distincts nécessaires | Prototype précoce ; pas de bascule implicite |

## Contraintes de l’environnement

Node 24.14.0, npm 11.9.0, Python 3.12.3 et GTK 3.24.41 présents ; Rust et WebKitGTK 4.1 absents initialement. Rust préparé dans /tmp pour les tests métier, sans modifier le profil utilisateur. Installation des dépendances dans le workspace ou /tmp. Pas de modification système implicite.

## Décisions d’exécution

- T-01..T-03 désigne les identifiants du CDC ; le lot 4 du plan correspond donc à T-03.
- Architecture locale modulaire conservée. Pas de microservices réseau : ils contrediraient le besoin hors ligne et ajouteraient des frontières inutiles.
- Recherche officielle et code upstream effectués avant implémentation ; sources dans docs/recherche-technique.md.
- Vérifications graphiques réellement indisponibles ou non réalisées restent signalées, même avec tests métier verts.


## Résultats et revue

- T-01 code ajouté, 8 tests React, typecheck/lint/build verts ; contrôle visuel navigateur large/étroit réalisé. Fenêtre desktop non validée : WebKitGTK 4.1/rsvg2 absents.
- T-02 contrat et machine pure validés : suite Rust 18/18, protocole Python 5 tests et 69 sous-tests ; fmt/Clippy sans desktop verts.
- T-03 code ajouté : Python 36 réussis, 1 test modèle réel sauté, 69 sous-tests. Verrous transitifs complets écrits ; runtime complet non installé, modèle réel absent.
- Prototype T-06 ajouté : garde appui/relâchement, rollback, relais FIFO, panneau non persistant. Recette matérielle ouverte.
- Revue native : cycle d’interblocage corrigé par relais ; tests indépendants de WebView.
- Revue finale indépendante : course de snapshot corrigée par révision monotone Rust/UI ; test React rouge puis vert et test Rust de révision rouge puis vert. Contre-revue sans constat actionnable ouvert.
- Aucun commit/push ni paquet système installé. Rapports conservés dans ce dossier puisqu’il n’existe pas de dépôt Git utilisable.

Voir bilan-t01-t03.md pour la liste exhaustive des preuves et vérifications à faire. Ne pas redévelopper ces lots à la reprise ; reprendre les recettes manquantes puis T-04.


## Incrément demandé ensuite : T-04 à T-06

- Recherche officielle Tokio/CPAL/Rubato/rustix avant code ; interfaces indépendantes distribuées entre agents.
- T-04 : superviseur Python intégré, 12 tests de processus et récolte des PID validés.
- T-05 : propriétaire CPAL dédié, limite indépendante, normalisation et WAV privés ; tests synthétiques natifs validés, aucune capture réelle automatique.
- T-06 : relais vers contrôleur et commandes Tauri, Échap temporaire, préparation UI limitée à la session, résultat éphémère affiché sans insertion.
- Revue : deux P1 contrôleur reproduits puis corrigés (capture après panne moteur et appui différé après relâchement perdu), deux P2 UI corrigés et contre-revus (raison cachée, raccourci obsolète). Regressions supplémentaires annulation/résultat et acquittement ID.
- Final : Rust desktop 42 unitaires +12 processus, React20, Python36/1sauté/69sous-tests ; checks/lints/build verts. Vitest timeout de forks sous compilations concurrentes puis passages1worker et standard verts ; voir bilan.
- SDK ALSA officiel extrait dans /tmp pour compiler ; aucun paquet système installé. Le poste doit installer libasound2-dev.
- Aucun modèle téléchargé ni micro ouvert par les tests. Recette réelle ouverte ; T-07 pas commencé.

Reprendre depuis bilan-t04-t06.md et les recettes matérielles, puis T-07 ; ne pas reconstruire les modules déjà livrés.

## Incrément : modèles locaux et choix utilisateur

- Whisper Tiny multilingue réellement installé dans `models/tiny` (78 205 610 octets avec README), révision officielle épinglée et SHA-256 des poids contrôlé. Les anciennes mentions « aucun modèle téléchargé » ci-dessus décrivent la situation avant cet incrément. Base est proposé par l’installateur mais reste non installé, compte tenu de la connexion lente.
- Catalogue local Rust en lecture seule, commande Tauri autorisée et sélecteur React ajoutés. Tiny est proposé par défaut ; un autre modèle installé ou un dossier personnalisé peut être choisi. La préparation explicite applique le choix ; « Modèle actif » suit le moteur. Les préférences restent limitées à la session.
- Contrôle réel : Tiny chargé par le `Transcriber` de production sur CPU/int8, silence privé retournant `NO_SPEECH`, connexions Python bloquées. Aucun micro enregistré. La précision sur parole et la recette native restent ouvertes.
- Ce contrôle a révélé l’incompatibilité PyAV 19/faster-whisper 1.2.1 : correction par PyAV 18.1.0, test de décodage sans poids rouge puis vert. Télémétrie ONNX désactivée avant import, régression rouge puis verte, contre-revue effectuée.
- Vérifications : 25 tests React, typecheck/lint/build et audit UI strict réussis ; Python 43 réussis, 1 test de parole sauté et 69 sous-tests. Reçus Tiny vérifiés hors ligne, `pip check` sans conflit. Rust : 43 unitaires +12 processus réexécutés depuis les binaires précédemment compilés après les modifications ; aucune nouvelle compilation à la reprise, le compilateur temporaire ayant disparu. Compilation native initiale documentée dans le rapport backend.
- Agents séparés pour catalogue, interface, installation et revue ; rapports `modeles-*-report.md` et `revue-modeles.md`. Guide utilisateur : `docs/modeles.md`. Prochaine étape : recette microphone/français/anglais, puis T-07.

## Correction du lancement : Rust persistant

Les exports utilisateur vers `/tmp/lvf-rustup` et `/tmp/lvf-cargo` échouaient après disparition de ces dossiers. Rust stable 1.99.1 et Cargo 1.99.1 ont été installés durablement avec rustup officiel dans `~/.rustup` et `~/.cargo`. Le profil zsh charge désormais `~/.cargo/env`. Vérifications fraîches : versions Rust/Cargo, résolution de Cargo dans un nouveau shell zsh et `cargo metadata --no-deps --format-version 1` du manifeste Tauri réussies. Aucun changement au code ni nouvelle compilation de l’application dans cette correction. Le terminal utilisateur déjà ouvert doit retirer les anciennes variables puis charger le nouvel environnement ; procédure ajoutée à `docs/installation.md`.

## Extension Local / Groq — 4 novembre 2026

Demande explicite : clé et nom du modèle dans `.env`, choix du fournisseur dans l’application. Plan `plan-groq.md`, bilan `bilan-groq.md`, guide `docs/groq.md`. Trois agents d’implémentation aux fichiers distincts, puis revue indépendante. Adaptateur Python standard HTTPS sans SDK ajouté, VAD et validation audio privée partagés, provider Rust typé et sélecteur natif UI. Secret uniquement côté moteur, aucune requête réelle Groq pendant les vérifications. `.env.exemple` sans clé et copie `.env` privée préparés ; pas de fichier utilisateur écrasé.

Validation finale : Python 79/1 sauté/77 sous-tests, React30, Rust45+12 ; compilation native, Clippy, formatage, typecheck/lint/build et audit UI strict réussis. Vrai Tiny testé hors ligne après refactor. Deux messages de revue corrigés ; aucun défaut important confirmé. Pas de commit/worktree, dépôt Git toujours indisponible. Recette réelle API/microphone ouverte après configuration par l’utilisateur ; prochaine tâche produit T-07.

### Diagnostic réel Groq après configuration utilisateur

Faux refus de clé reproduit : HTTP 403 avec le client urllib par défaut, HTTP 200 avec un User-Agent d’application explicite sur la même clé. Correction du header et séparation de 401/403 ; notice Échap dédoublonnée. Test réel de l’endpoint de transcription réussi avec silence synthétique, aucun micro ni audio personnel. Clé jamais imprimée ni modifiée. Python79/1 sauté/77 sous-tests, React30, typecheck/lint/build réussis. Rapport `groq-auth-fix.md`. Recette de parole au microphone encore à faire ; inscription native d’Échap inchangée.


## Finalisation T-07–T-08 — 10 novembre 2026

Vues Dictée et Réglages finalisées selon la demande avec UI/UX Pro Max et icônes Iconify/Lucide embarquées. Brouillon de réglages conservé lors de la navigation, focus clavier explicite et diagnostics actionnables rafraîchis. Documentation utilisateur actualisée. Vérifications : React30, typecheck/lint/build, affichages Chromium 900/360 px et audit Axe sans violation sur aperçu ; Rust76 unitaires +12 processus avec native-audio. Recette réelle de frappe synthétique sur Xvfb :198 dans GTK/WebKit : un défaut de perte Unicode à délai nul reproduit et corrigé par un délai de 12 ms, test rouge puis vert. Bilan `bilan-t07-t08.md`, protocole `tests/manual/README.md`.

Ces validations ne constituent pas encore J2 : dictée au microphone dans un éditeur/navigateur réels, redémarrage desktop et mesure NF-04 restent à recetter sur Cinnamon/X11. Le build desktop complet et la distribution autonome restent distincts. Aucun audio personnel ni appel Groq pendant cette reprise.


## Diagnostic du raccourci sur le nouveau poste

Le moteur Python du projet était absent, alors que les réglages Groq étaient déjà enregistrés. `.venv` et les 23 dépendances runtime verrouillées ont été installés ; téléchargements repris après interruptions et SHA-256 des roues vérifiés. `pip check`, VAD réel sur silence avec réseau Python bloqué et CLI Groq `ready` réussis, sans microphone ni appel à l’API audio. L’UI explique la préparation nécessaire et affiche un retour lorsqu’un événement de raccourci est reçu ; libellés canoniques control/alt normalisés. React31, typecheck/lint/build et compilation desktop complète hors ligne réussis. Rapport `diagnostic-raccourci-python.md`. L’application déjà ouverte doit être préparée à nouveau via Enregistrer et préparer.

## Microphone animé et identité — 10 novembre 2026

Choix utilisateur : logo sobre, vert et arrondi, micro avec onde de voix. Réalisation via les skills drawing-from-references (SVG Lab) et design-brand-identity ; dessin SVG manuel depuis référence, export éditable, déclinaisons sans modification de géométrie pour en-tête, favicon et icône native. Guide : `docs/brand-identity.md`.

`MicrophoneActivity` distingue réception du raccourci, capture confirmée et traitement. Les halos apparaissent seulement pendant `recording` natif ; aucune simulation de volume. Réduction des mouvements : état lisible sans halos ni rotation. L’aperçu navigateur reste inactif.

Vérifications : 33 tests React réussis, typecheck/lint/build et cargo check verrouillé hors ligne réussis ; contrôle Chromium 900/360 px sans débordement, Axe sans violation sur les deux vues. Transitions reçu/capture/traitement/repos et réduction des mouvements contrôlées dans le navigateur avec props de test isolées. Aucun microphone ni appel audio API utilisé dans cette validation ; recette native du raccourci à effectuer dans l’application desktop.
