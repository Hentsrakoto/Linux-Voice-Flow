# Sélection des modèles locaux

Le panneau de dictée interroge `list_local_models` pour afficher les modèles installés. Tiny est présélectionné quand il est disponible ; une réponse tardive ne remplace pas un choix déjà effectué. Le chemin personnalisé reste accessible. La liste indique la taille des fichiers sur disque, sans la présenter comme une consommation mémoire.

Changer la sélection ne lance aucun téléchargement, moteur ou enregistrement. Le bouton « Préparer le moteur » transmet le chemin sélectionné au contrôleur existant. Le libellé « Modèle actif » suit le snapshot du moteur ; il reste distinct de la sélection en attente. Les réglages sont désactivés pendant les opérations actives et restent limités à la session.

Les erreurs de catalogue permettent une nouvelle tentative ou un dossier personnalisé. Un modèle retiré de la liste conserve sa sélection et bloque la préparation, sans substitution silencieuse. Les abonnements et réponses tardives sont invalidés au démontage.

Vérifications exécutées après reprise : typecheck, lint sans avertissement, 25 tests React et build Vite réussis. Les 15 tests du panneau incluent cinq scénarios ajoutés : sélection Tiny/Base explicite, catalogue tardif, erreur/récupération, modèle actif distinct et sélection devenue indisponible. Audit UI strict sans constat. La fenêtre native et la qualité sur une vraie dictée restent à vérifier sur le poste cible.
