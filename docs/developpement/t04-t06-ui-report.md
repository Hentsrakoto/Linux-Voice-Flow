# Interface T04–T06

Interface native ajoutée après connexion desktop : dossier absolu du modèle local, sélection native du microphone, langue fr/en/auto, préparation explicite, statut réel, durée d’enregistrement fournie par Rust, annulation et résultat sélectionnable/effaçable. Valeurs conservées uniquement pendant cette session. Aucune capture automatique, API audio React, persistance, injection ni copie presse-papiers. Aucun journal de texte ou de chemin utilisateur.

L’abonnement `dictation://state` précède la lecture initiale. Tous les résultats d’événements et commandes sont acceptés uniquement si leur révision est au moins celle affichée. Les abonnements tardifs sont libérés après démontage. La purge Rust du résultat supprime aussi le champ affiché. La liste des microphones se charge indépendamment et peut être actualisée après erreur. Les erreurs techniques brutes ne sont pas affichées.

L’aperçu navigateur reste sans appel natif. Les tests App isolent le panneau de dictée ; ses tests propres simulent la frontière Tauri. Le diagnostic HotkeyProbe existant n’a pas été modifié par cette contribution.

## Vérifications exécutées

- Tests écrits avant implémentation : échec attendu initial sur le composant absent.
- `npm run typecheck` : succès.
- `npm run lint` : succès.
- `npm test -- --run` : 20 tests réussis (3 fichiers), sans erreur non gérée.
- `npm run build` : succès.
- `python3 /home/hents/.codex/plugins/cache/openai-curated-remote/frontend-design-premium/1.4.0/skills/frontend-design-premium/scripts/audit_project.py /home/hents/Documents/wispr_flow --mode strict --config /tmp/wispr-ui-audit.json` : zéro résultat. Configuration temporaire utilisée : `{"ownership":{"Select/Listbox":"native"}}`. Décision durable documentée dans DESIGN.md ; aucun fichier de configuration projet créé hors du périmètre autorisé.

Couverture des tests : chargement, souscription avant lecture, validation du dossier, configuration explicite, révisions anciennes ignorées (y compris lecture initiale tardive), capture active bloquant la préparation, annulation, affichage/effacement/purge du résultat, erreur de liste et reprise, erreur moteur/action sans exposition du payload, nettoyage des listeners après démontage et résolution tardive.

## Vérifications manuelles à effectuer en intégration

Non réalisées dans cette contribution : affichage navigateur étroit/desktop et zoom, popup des sélections au clavier, modèle réel et microphone matériel. Vérifier dans la fenêtre native : préparer un dossier valide sans démarrer le microphone ; maintenir puis relâcher Ctrl+Alt+Space ; voir durée puis transcription ; annuler pendant chargement/capture/transcription ; effacer puis vérifier purge après 120 secondes ; essayer modèle absent et microphone indisponible. L’environnement matériel et le modèle local sont nécessaires pour confirmer ces scénarios.

## Notice Échap (intégration T06)

Le panneau s’abonne également à `dictation://notice` (texte assaini par le backend). La notice est visible près des commandes, annoncée comme statut, et rappelle d’utiliser Annuler. L’échec de cette souscription produit une explication sûre sans empêcher les statuts de dictée. Tests ajoutés avant implémentation : notice lors d’une capture avec Annuler disponible, nettoyage des deux listeners et résolution tardive du listener de notice après démontage.

## Corrections de revue finale

Les instructions ne fixent plus Ctrl+Alt+Space : elles renvoient au raccourci actif du diagnostic. `reason?: 'recording_limit' | null` est accepté dans le snapshot pour compatibilité avec les anciennes fixtures. La limite de 60 secondes explique explicitement l’abandon audio sans transcription. NO_SPEECH est annoncé en statut même en phase ready. MODEL_NOT_FOUND, MODEL_LOAD_FAILED, MIC_UNAVAILABLE, ENGINE_TIMEOUT, ENGINE_CRASHED et PROTOCOL_ERROR ont une explication et une action corrective constantes. Les rejets de commandes structurés code/error_code utilisent la même table ; aucune lecture du message brut. Tests ajoutés avant correction : NO_SPEECH, limite, modèle absent, absence de raccourci fixe et erreur de commande structurée sans fuite du chemin.

Le textarea autorise désormais le redimensionnement vertical à la demande explicite de la revue. Cette décision remplace la règle générique resize-none de l’audit premium ; son audit statique signale donc cette exception intentionnelle. Les contrôles projet (typecheck/lint/20 tests/build) passent.


## Complément final du coordinateur

Après intégration et corrections de revue : suite desktop complète **42 tests unitaires +12 tests processus réussis**, Clippy desktop et sans desktop avec `-D warnings` réussis, formatage réussi. Frontend **20 tests réussis**, TypeScript/ESLint/build réussis ; Python **36 réussis, 1 modèle réel sauté, 69 sous-tests réussis**. Les compteurs intermédiaires ci-dessus décrivent leur passage historique. Le bilan consolidé `bilan-t04-t06.md` contient les commandes, le timeout de démarrage Vitest transitoire et les recettes matérielles encore ouvertes. La zone de résultat finale suit la règle de hauteur stable `resize-none` avec défilement interne, documentée dans UX-CONTRACT.md ; les décisions de sélecteurs natifs sont persistées dans premium-ui.json. Aucun succès microphone/modèle réel n’est revendiqué.
