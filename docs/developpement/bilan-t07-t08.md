# Bilan T-07 / T-08 — reprise du 10 novembre 2026

## Fonctionnalités présentes

- Dictée : raccourci réellement enregistré, état, indication Local/Groq, annulation et transcription éphémère. Aucun formulaire de configuration ni diagnostic sur cette vue.
- Réglages : fournisseur, modèle local ou dossier, microphone, langue, raccourci, insertion automatique et consentement à la copie. Sauvegarde atomique privée versionnée côté Rust et restauration au lancement.
- T-07 : cible X11 mémorisée au démarrage, revérification avant et entre les blocs de frappe, récupération par copie explicite ou consentie, effacement du propriétaire du presse-papiers et avertissement après insertion incertaine. Annulation, génération et identité de session empêchent la livraison de résultats périmés ou répétés.
- Navigation : brouillon conservé entre les vues, focus clavier déplacé vers main uniquement après navigation explicite, réglages bloqués pendant les opérations actives. Le diagnostic se rafraîchit après les changements d’état stables et propose des actions pour les prérequis manquants.
- Présentation : titre court, raccourci au-dessus du texte, icônes Lucide embarquées avec Iconify offline, texte et contrôles lisibles sur petits écrans. Pas de police distante ni de requête vers une API d’icônes.

## Skill appliqué

`/home/hentsrakoto/.agents/skills/ui-ux-pro-max/SKILL.md` et ses références `quick-reference.md` et `pro-rules.md` ont été lus. Stack React détectée dans package.json. La recherche UX « keyboard focus navigation » a renvoyé Keyboard Navigation et Focus States, applicables à cette interface desktop. Les recherches sur progressive disclosure et la préservation du formulaire ont donné des résultats peu adaptés ; les règles intégrées state-preservation, focus-on-route-change, field-grouping et progressive-disclosure ont servi de recommandations générales. Le design existant reste la référence ; aucune palette ou police arbitraire n’a été générée.

## Vérifications réalisées

- `npm ci` : dépendances verrouillées installées.
- `npm run typecheck`, `npm run lint`, `npm run build` : réussis.
- `npm run test -- --run` : **30 tests réussis**. Les nouveaux cas couvrent le brouillon après navigation, le focus de la vue et les diagnostics actionnables. Le test existant de reprise après bootstrap incomplet a révélé une fausse alerte de raccourci : corrigée en distinguant un statut inconnu d’un échec d’enregistrement confirmé.
- Chromium headless, aperçu navigateur aux largeurs **900 et 360 px** : vues Dictée et Réglages sans débordement horizontal ni exception JavaScript ; focus sur main après navigation. Captures inspectées visuellement.
- Axe, WCAG A/AA 2.0/2.1/2.2 dans les deux vues en aperçu navigateur : **aucune violation détectée**. Ceci ne vaut pas validation exhaustive du desktop ni des technologies d’assistance.
- Contrastes calculés : textes ink/muted/primary sur paper/surface entre **5,66:1 et 12,41:1**.
- `git diff --check` : réussi.

## Validation native

Rust 1.99.1 préparé temporairement dans `/tmp/lvf-cargo` et `/tmp/lvf-rustup`, sans modification du profil utilisateur. SDK ALSA et outils Xvfb/xdotool extraits localement depuis les paquets Ubuntu ; aucune installation de paquet système. Les fixtures GTK et WebKit sont prêtes sur un serveur isolé **:198**. La suite Rust avec `--no-default-features --features native-audio --locked` a réussi : **76 tests unitaires et 12 tests de processus**. Elle compile aussi le propriétaire CPAL et les régressions du raccourci et du diagnostic microphone.

Le test opt-in `x11_output_smoke` a d’abord échoué : avec `xdotool --delay 0`, « Bonjour élève, café — Unicode 🐢 » devenait « Bonjour lve, caf  Unicode ». Une sonde synthétique a reproduit le problème à 1 ms et réussi à 5, 12 et 20 ms. La frappe utilise maintenant **12 ms par caractère**, par blocs de 16 caractères maximum ; le contrôle d’annulation reste entre blocs, avec les limites de temps existantes. Le même test d’intégration est ensuite passé dans le champ GTK et la zone de texte WebKit : **1 test réussi**, comparaison Unicode exacte, aucune soumission de message. Il ne s’agit pas d’une dictée vocale ni du navigateur utilisateur.

Un dernier passage parallèle a exposé une course dans l’acquisition du presse-papiers : la lecture de contrôle pouvait échouer avant que le propriétaire ait acquis la sélection. Le module réessaie maintenant uniquement cette lecture, tant que son propriétaire vit et avant la limite de 700 ms. Une régression déterministe force l’échec de la première lecture puis confirme la copie. La suite complète finale réussit : **76 unitaires + 12 processus**, après cette correction. Aucune frappe ni copie n’est relancée automatiquement.

## Recette utilisateur restante

T-07 et T-08 sont implémentés ; **J2 n’est pas encore déclaré acquis**. Il reste à dicter réellement dans un éditeur puis un navigateur sur Cinnamon/X11, vérifier le changement de focus et le fallback, restaurer les réglages après redémarrage de la vraie application et mesurer NF-04. Le build desktop complet reste à vérifier avec les en-têtes GTK/WebKit du poste. Aucun microphone n’a été enregistré, aucun audio envoyé à Groq et aucun vrai chat modifié pendant cette reprise. Les limites de X11 sur un changement de curseur dans une même fenêtre et l’insertion partielle restent documentées.

Le protocole de recette synthétique X11 est dans `tests/manual/README.md`. T-09/T-10 restent distincts : audit global de confidentialité, recette complète et distribution du moteur Python hors du dossier source.


## Correction de lancement après signalement utilisateur

Le terminal utilisateur ne trouvait pas Cargo, la chaîne de tests étant temporaire. Rust/Cargo ont été déplacés vers `~/.rustup` et `~/.cargo`, le fichier env adapté et son chargement ajouté à `~/.zshrc` avec sauvegarde préalable. Nouveau zsh interactif : Cargo et rustc 1.99.1, résolution dans `~/.cargo/bin` et `cargo metadata` réussis. Le serveur Vite d’aperçu démarré par l’agent a été arrêté pour libérer le port 1420. Les paquets natifs manquants nécessitent une installation sudo par l’utilisateur sur son terminal ; aucun mot de passe demandé ou manipulé dans le chat.


## Compilation desktop confirmée

Après installation des paquets système par l’utilisateur, `cargo check --manifest-path src-tauri/Cargo.toml --locked --offline` a réussi le 10 novembre 2026, avec le feature desktop par défaut, GTK/WebKit et CPAL. La limite de compilation desktop mentionnée plus haut est donc levée. Le signalement ultérieur « raccourci sans effet » a révélé l’absence de `.venv/bin/python` ; la configuration Groq était lisible mais le moteur ne pouvait pas être lancé. Voir `diagnostic-raccourci-python.md`.
