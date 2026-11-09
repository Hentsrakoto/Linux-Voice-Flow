# Prototype du raccourci global X11

## Décision

Utiliser le plugin officiel `tauri-plugin-global-shortcut` côté Rust. Le plugin fournit `Pressed` et `Released` ; la politique du projet filtre les doublons pour produire un seul cycle par maintien. React ne s’enregistre pas lui-même auprès du système.

La sélection du plugin est fondée sur son API, pas sur une validation matérielle déjà acquise. Le test réel sous Cinnamon/X11 reste nécessaire avant T-05/T-07.

## Remplacement

Le gestionnaire enregistre le nouveau raccourci avant de désinscrire l’ancien. Si le nouvel enregistrement échoue, l’ancien reste actif. Si l’ancien ne peut pas être retiré, tenter d’annuler le nouvel enregistrement et signaler l’erreur ; ne jamais annoncer un remplacement réussi dans cet état. Refuser le remplacement pendant un appui maintenu.

Une erreur d’enregistrement système est plus fiable que `is_registered` pour les conflits externes : cette dernière API ne voit que les enregistrements du processus. Même une inscription acceptée ne prouve pas que Cinnamon ou une application ne consomme pas certains événements ; d’où la recette réelle.

## Recette nécessaire

1. Ouvrir la fenêtre desktop sous Cinnamon/X11.
2. Vérifier le raccourci initial `Ctrl+Alt+Space`, puis le maintenir et le relâcher dans un éditeur et dans un navigateur.
3. Vérifier un seul événement d’appui et un seul de relâchement par cycle, y compris maintien prolongé et auto-répétition.
4. Essayer un raccourci invalide ou déjà utilisé : l’erreur doit être lisible et l’ancien raccourci conservé.
5. Changer de fenêtre pendant le maintien et vérifier qu’un relâchement est reçu ; toute perte doit être documentée avant capture réelle.
6. Fermer et relancer : aucun enregistrement résiduel ; aucun microphone ouvert par ce prototype.

Les tests unitaires simulent le backend pour vérifier la politique ; ils ne constituent pas une preuve de réception d’événements X11.


## Résultats de revue et synchronisation

Le callback natif publie un tuple dans un relais FIFO indépendant de Tauri, sans attendre le verrou du gestionnaire. Un worker retient les événements pendant une transaction d’enregistrement ; l’arrêt utilise un drapeau atomique et `App::run_return` avant jonction. Les deux tests du relais s’exécutent sans WebView. Les snapshots portent `revision`, incrémentée au remplacement réussi et aux transitions physiques, pour empêcher une réponse IPC ancienne d’écraser un événement récent dans React.

Les compteurs et le raccourci du prototype sont réinitialisés au redémarrage. La persistance relève de T-08. Les tests automatisés passent ; la recette X11 réelle reste ouverte.
