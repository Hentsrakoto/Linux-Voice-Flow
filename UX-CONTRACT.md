# Contrat d’interaction — Dictée et Réglages

Le cahier des charges et `docs/developpement/plan-t07-t08.md` définissent le métier ; DESIGN.md définit la présentation.

- **Navigation — propriétaire App :** vues Dictée / Réglages, boutons natifs avec aria-current et focus visible. Pas de formulaire de configuration ni diagnostic sur Dictée. Les abonnements runtime restent montés pendant navigation. Le brouillon des réglages reste dans App et survit aux allers-retours ; le focus rejoint main après un changement de vue explicite, jamais après un événement de dictée.
- **Select/Listbox — propriétaire natif :** `SettingsPanel` utilise des selects HTML pour service, modèle, microphone et langue. Le navigateur/WebView possède popup, géométrie et clavier. Aucun composant popup parallèle.
- **Formulaire — propriétaire SettingsPanel :** réglages restaurés depuis Rust ; valeurs conservées après erreur. Enregistrer et préparer sauvegarde puis configure le moteur. Les changements sont bloqués pendant préparation/capture/transcription/insertion/annulation. Limite fixe60s explicitée, aucune fausse option.
- **Service et modèle :** le fournisseur choisi reste distinct du moteur actif. Groq n’exige pas de modèles locaux. Les champs locaux sont masqués pour Groq, leur contenu est conservé au retour. Aucun changement ne prépare automatiquement un autre fournisseur avant l’action Enregistrer. Les fichiers manquants ne sont pas remplacés silencieusement par un autre modèle.
- **Groq :** information d’envoi audio avant sauvegarde ; indication du fournisseur réellement actif sur Dictée. Clé et modèle distant configurés dans `.env` ; aucune clé dans les champs ou commandes UI. Aucun basculement automatique vers l’API.
- **Raccourci :** Dictée affiche le raccourci enregistré, pas une constante obsolète. Édition et saisie au clavier sont dans Réglages. Échap annule la saisie d’un raccourci et reste réservé à l’annulation de dictée.
- **Activité microphone — propriétaire MicrophoneActivity :** impulsion ponctuelle à la réception du raccourci ; halos verts uniquement quand le backend natif confirme recording. Après relâchement et changement de phase, indicateur de traitement puis repos. Aucun niveau sonore simulé. Réduction des mouvements : état et texte conservés, halos et rotation supprimés. L’aperçu navigateur reste inactif.
- **Retour asynchrone :** `useVoiceFlow` accepte les snapshots par révision monotone, s’abonne avant la lecture initiale, libère les abonnements et ignore leurs réponses après démontage. Réglages corrompus et diagnostics absents ne sont pas présentés comme disponibles. Un fichier corrompu ne peut être remplacé que par l’action explicite Enregistrer et préparer, expliquée dans la notice.
- **Annulation :** bouton Annuler disponible pendant les opérations actives, y compris insertion. Échap temporaire appartient au backend ; une notice d’échec est affichée une seule fois. Aucun microphone ne démarre depuis une navigation ou un enregistrement de réglages.
- **Insertion et copie :** insertion automatique activée par défaut, consentement à la copie automatique désactivé par défaut. Expliquer le changement de focus et sa limite X11. Copier est explicite, appelle `copy_dictation_result` sans texte frontend et n’est disponible que pour un résultat vivant. Un fallback ne prétend jamais à une insertion réussie ; le drapeau insertion_uncertain conserve l’avertissement de doublon possible après copie et changement de code d’erreur. Aucun envoi automatique de message dans le chat.
- **Résultat :** textarea native en lecture seule, six lignes, défilement interne, taille fixe resize-none. Texte sélectionnable, purge backend120s, Effacer explicite. Effacer ma copie du presse-papiers n’agit que sur le propriétaire clipboard de l’application.
- **Messages et accessibilité :** français, erreurs constantes dérivées des codes, aucune exception brute ni transcription dans les logs. État annoncé poliment, minuterie non annoncée à chaque tick. Labels natifs, navigation clavier et focus visible. Browser preview honnête sans appel natif ; réglages désactivés.

## Canonical UI Map

| Capability | Canonical owner | Source of truth | Allowed variants | Verification |
|---|---|---|---|---|
| Navigation | App | UX-CONTRACT.md, DESIGN.md | boutons Dictée et Réglages | src/app/App.test.tsx ; recette clavier |
| Select/Listbox | Native HTML select in SettingsPanel | UX-CONTRACT.md, DESIGN.md | native | src/app/App.test.tsx ; popup natif Mint |
| Form | SettingsPanel | Settings IPC, UX-CONTRACT.md | configuration persistante local ou Groq | src/app/App.test.tsx |
| Result | DictationPanel | dictation snapshot | inserted, clipboard, preview, fallback | src/features/dictation/DictationPanel.test.tsx |
| Scrollbar | src/styles.css | DESIGN.md | application, textarea résultat | contrôle navigateur ; recette clavier Mint |
