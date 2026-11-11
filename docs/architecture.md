# Architecture — socle T-01 à T-03

## Frontières

L’application est un programme desktop local. Aucun serveur HTTP de transcription ou microservice n’est nécessaire. Le serveur Vite n’existe qu’en développement.

| Zone | Responsabilité actuelle | Accès exclus |
|---|---|---|
| React/TypeScript | État initial, connexion desktop, prototype de diagnostic raccourci | Pas de capture, de shell, de fichiers audio ni de modèle |
| Rust métier | Contrat JSONL, machine à états et politique de raccourci testables sans WebView | Pas d’inférence Python intégrée à ce stade |
| Adaptateurs Tauri | Fenêtre, métadonnées et enregistrement X11 du raccourci | Pas d’injection de texte ni de presse-papiers automatique |
| Python | Validation d’un modèle local complet, VAD et transcription d’un WAV privé ; CLI JSONL | Pas de microphone, d’historique ou de service distant |

`src-tauri/src/lib.rs` assemble les modules ; `desktop.rs` contient les adaptateurs Tauri. Le feature Cargo `desktop`, activé par défaut, est séparé des modules purs. `cargo test --no-default-features` permet leur vérification sans les bibliothèques WebKit/GTK.

## Communications

React utilise `invoke` pour `get_app_info`, `get_hotkey_status` et `replace_hotkey`. Les permissions de la fenêtre principale autorisent seulement ces commandes et l’abonnement/désabonnement aux événements. Les raccourcis sont enregistrés dans Rust ; l’UI ne reçoit que statut et compteurs. Les événements du prototype utilisent `hotkey://event`.

Le contrat futur Rust/Python est décrit dans [protocole.md](protocole.md) et `contracts/engine-v1.schema.json`. Les constantes de timeout sont définies avec le contrat, mais leur application dans un superviseur Rust appartient à **T-04**. Ne pas confondre contrat testé et moteur déjà intégré à Tauri.

## Session et annulation

La machine à états pure ne contient ni audio ni transcription. Elle associe les événements à une identité de session et produit des effets. Les réponses tardives et répétées ne peuvent pas créer une seconde insertion.

Le futur contrôleur doit générer des identités monotones, exécuter les effets une fois, vérifier le focus puis réévaluer `can_insert` sous le verrou de session juste avant toute insertion. Aucun texte n’est injecté par le socle actuel.

Python garde un seul worker de transcription. Sa boucle de contrôle reste disponible pour les messages `cancel`. L’annulation invalide le résultat ; l’acquittement n’est envoyé qu’après la fin effective du worker. Un calcul natif bloqué ne peut pas être interrompu de façon garantie par un thread : le superviseur Rust de T-04 devra terminer le processus après le délai de 3 secondes, puis le récolter avant redémarrage.

À EOF, le moteur neutralise le résultat et quitte. Le worker Python est daemon ; cela n’est pas une preuve d’arrêt immédiat de toute bibliothèque native, d’où la supervision externe prévue.

## Audio et modèle

Le moteur exige un modèle CTranslate2 local comprenant son tokenizer ; il refuse les identifiants de modèle distants et les ressources manquantes. Les variables hors ligne et `local_files_only` renforcent cette validation. Le modèle n’est jamais téléchargé au démarrage.

Les WAV doivent être mono PCM 16 bits à 16 kHz, non vides, d’au plus 60 secondes, dans un répertoire privé 0700 appartenant à l’utilisateur ; fichiers 0600, réguliers et sans traversée de symlink. Le format correspond au transport que Rust devra produire en T-05. Python ne supprime pas ces fichiers : Rust en restera propriétaire.

`duration_ms` dans une réponse de transcription représente la durée de l’audio validé. Le temps de traitement et la réactivité UI doivent être mesurés séparément en recette.

## Journaux et données

stdout Python est exclusivement JSONL. La CLI possède un descripteur de sortie dédié et neutralise les sorties natives non filtrées ; les erreurs utiles sont transmises par code et message constants, sans recopier les chemins, l’audio ou la transcription. L’intégration Rust devra appliquer la même minimisation sur stderr et ses propres logs.

Pas de base de données, de journal d’historique ou de modèle génératif de réécriture. Le texte transcrit ne doit pas devenir un message technique ou une donnée persistante au fil des lots suivants.

## Suites du plan

T-04 : démarrage/supervision et transcription de fixture depuis Rust. T-05 : capture réelle et WAV temporaires nettoyés. T-07 : focus/insertion/fallback. T-08 : réglages persistants et diagnostic complet. T-09/T-10 : audit transversal, recette matérielle et distribution.

## Incrément T-04–T-06

Le contrôleur `dictation.rs` possède une boucle sérialisée sur un thread dédié avec runtime Tokio. Il orchestre le superviseur `engine.rs`, le propriétaire CPAL et le stockage privé. `launch` fournit immédiatement un handle de moteur : le chargement du modèle n’empêche ni l’annulation ni la fermeture. Le superviseur valide JSONL, borne les files et lectures, applique 60/120/3 secondes et récolte le processus après arrêt. Un échec conduit à une préparation explicite avant reprise ; aucun redémarrage automatique infini.

L’audio reste en mémoire pendant l’appui. Le thread propriétaire du flux applique sa propre limite de 60 secondes, indépendamment de la disponibilité du contrôleur. Au relâchement, le flux est fermé puis le signal mono est rééchantillonné avec filtrage anti-repliement vers 16 kHz/PCM16. Seul le WAV privé final est envoyé à Python. Une instance du magasin garde un marqueur verrouillé ; le nettoyage de crash ignore les instances actives et les entrées étrangères.

Le relais global conserve la distinction appui/relâchement sans verrou bloquant dans le callback X11. Les commandes de capture portent une génération : une annulation ou une saturation invalide les appuis encore en file, pour ne pas démarrer une capture après un relâchement perdu. Toute invalidation de session abandonne aussi le microphone. Les événements de résultat exigent l’identité active ; les acquittements d’annulation exigent celle annulée. Échap et son équivalent avec les modificateurs du raccourci ne sont inscrits que pendant les phases actives.

L’UI reçoit `dictation://state` et ses révisions, sans chemin audio. Elle affiche des messages contrôlés à partir de codes et d’un motif `recording_limit`, sans recopier les diagnostics arbitraires du moteur. Le texte du résultat est sélectionnable et effaçable, avec une rétention de 120 secondes. La capture et la transcription sont maintenant intégrées ; l’insertion X11, le suivi du focus, le presse-papiers et les réglages persistants ne le sont pas encore.

Les sections précédentes décrivent l’état du socle T-01–T-03 au moment de sa livraison. Pour l’état courant, ce complément et le bilan T-04–T-06 prévalent.


## Incrément T-07–T-08

Le contrôleur utilise `DesktopOutput` : capture de la fenêtre active et du focus enfant X11, insertion `xdotool` par blocs UTF-8 et copie consentie via un propriétaire `xclip`. Le texte passe sur stdin, sans shell. Annulation et génération sont revérifiées ; aucune nouvelle tentative automatique après insertion incertaine. Le curseur dans une même fenêtre reste une limite documentée. Les réponses en retard ne sont pas livrées une seconde fois.

`settings.rs` sauvegarde atomiquement la configuration privée versionnée et la restaure au lancement. Les changements de réglages et l’ancienne commande de raccourci sont sérialisés avec les démarrages de dictée. React dispose des vues Dictée et Réglages ; le brouillon survit à la navigation. Les diagnostics et les choix Local/Groq, modèle, microphone, langue et raccourci sont dans Réglages. La vue Dictée montre le raccourci enregistré, l’état et le résultat éphémère, avec copie/effacement et annulation. Ces éléments remplacent les anciennes descriptions de fonctionnalités encore futures dans les sections historiques ci-dessus.

Le code et les tests automatisés ne remplacent pas la recette microphone réel, éditeur et navigateur sur Cinnamon/X11. Voir `docs/developpement/bilan-t07-t08.md` pour les validations de la reprise.
