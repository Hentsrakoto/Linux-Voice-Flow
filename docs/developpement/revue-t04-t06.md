# Revue indépendante T-04 à T-06

Revue statique du code courant, sans Git exploitable, sans installation, modification du code ou répétition des tests déjà passés. Périmètre : superviseur Python, contrôleur, audio CPAL/conversion, stockage temporaire, raccourcis/relais, adaptateur Tauri et capacités, panneau de dictée/diagnostic et tests associés ; comparaison au CDC et aux lots 5–6. T-07 (insertion), T-10 (distribution) et validation matérielle sont hors du verdict de correction du code.

## Constats actionnables

### P2 — La limite et l'absence de parole ne sont pas expliquées à l'utilisateur

`src/features/dictation/DictationPanel.tsx:101` et `:103` n'utilisent ni le motif ni le code métier de l'instantané. Après soixante secondes, Rust abandonne correctement l'audio et produit un message explicite, mais l'écran accuse seulement le moteur ou le microphone et demande une nouvelle préparation. Après `NO_SPEECH`, Rust revient à `Ready` avec le code correspondant ; l'écran présente uniquement « prêt pour votre prochain appui », sans expliquer pourquoi aucun texte n'a été produit. La première situation manque le message explicite F-13/R-11 ; la seconde empêche de distinguer le silence d'une dictée réussie sans résultat.

Correction minimale : rendre une explication assainie pour ces motifs, via une table de codes et un motif distinct pour la limite (ou des messages backend explicitement garantis constants). Conserver le refus des erreurs/payloads bruts couvert par les tests actuels. Ajouter des assertions sur limite et absence de parole.

### P2 — Le panneau continue d'indiquer l'ancien raccourci après configuration

`src/features/dictation/DictationPanel.tsx:75` indique toujours `Ctrl+Alt+Space`. Le diagnostic permet pourtant de remplacer ce raccourci, et le backend retire alors l'ancien. Après une modification réussie, suivre l'instruction principale ne démarre donc aucune dictée.

Correction minimale : parler du « raccourci actif » et indiquer où le consulter, ou partager son état réel avec le panneau. Une modification de raccourci doit laisser les instructions cohérentes.

## Corrections précédentes vérifiées dans le code courant

Les deux P1 de `revue-controller-t04-t06.md` sont corrigés : `invalidate()` abandonne désormais la capture ; les événements en file portent une génération et une annulation/perte d'événement invalide les anciennes générations. Le signal d'annulation est également consulté avant traitement d'une réponse moteur ; l'annulation d'un succès efface le résultat ; l'acquittement d'annulation exige l'identifiant attendu. Ces anciens constats ne sont pas des problèmes encore ouverts.

## Qualité et conformité

Architecture cohérente pour ce jalon : subprocess sans shell, lecture JSONL bornée, stderr jeté, échéances et récolte de l'enfant ; propriété explicite du flux et des WAV ; callback sans disque, conversion ni verrou bloquant ; arrêt avant conversion ; borne native indépendante du contrôleur ; chemins privés ouverts sans suivi de liens ; nettoyage limité aux instances reconnues non verrouillées ; réponses liées à une session et résultat courant purgé après 120 secondes. Les opérations de raccourcis utilisent un relais pour éviter le verrou du gestionnaire dans le callback natif. Échap est limité aux phases actives et couvre les modificateurs du raccourci. Les capacités restent limitées aux commandes locales attendues. L'aperçu indique honnêtement que l'insertion appartient à l'étape suivante.

Les tests lus couvrent les principaux chemins de panne, suppression, délai, identité et cycle de vie des listeners. La compilation et les tests natifs ont été exécutés par l'intégrateur avec le véritable SDK ALSA extrait localement ; cette revue ne les a pas relancés et ne fige pas leurs compteurs pendant les dernières corrections. Les documents de résultats doivent utiliser le dernier passage effectif.

Verdict distinct : l'implémentation constitue un socle solide T04–T06, sous réserve des deux défauts d'explication UI ci-dessus. Le jalon fonctionnel complet reste non démontré : fixture depuis Tauri avec modèle réel, capture volontaire et comportement appui/relâchement/Échap sous Cinnamon/X11 sont encore à recetter. Cette lacune de validation n'est ni un échec de compilation native ni un défaut matériel démontré. La persistance des réglages et la distribution restent explicitement différées ; aucune conformité globale MVP/J1/J2 n'est déduite des seuls tests simulés.

## Résolution après contre-revue ciblée

Les deux P2 ci-dessus sont **clos dans le code courant**. Cette contre-revue est limitée à leurs corrections, sans nouvelle exécution des tests :

- `DictationPanel.tsx` indique maintenant « votre raccourci de dictée » et renvoie au diagnostic pour sa valeur active ; il ne présente plus l'ancienne combinaison comme une instruction universelle.
- `NO_SPEECH` produit une explication constante annoncée avec `role="status"`. La limite utilise le motif structuré `reason: "recording_limit"`, émis par Rust dans `audio_error`, initialisé à `None` et effacé lors d'une invalidation ou d'une autre erreur. Le panneau explique explicitement la limite de 60 secondes, l'abandon de l'audio et l'absence de transcription. Les autres erreurs reconnues sont traduites par des constantes frontend ; les messages bruts restent exclus.
- Les nouvelles régressions frontend vérifient ces explications, l'absence d'instruction figée et la non-exposition des chemins privés. La régression Rust vérifie l'émission du motif de limite. L'intégrateur rapporte le passage RED puis la correction, 20 tests frontend réussis et un parcours synthétique FakeAudio → processus Python factice → aperçu → effacement → récolte, avec racine temporaire vide ; les compteurs définitifs du passage complet appartiennent aux rapports de validation.

Verdict actualisé : **aucun constat actionnable restant dans cette revue T04–T06**. Les réserves de recette matérielle/modèle/X11 et l'absence de démonstration J1/J2 demeurent inchangées ; aucune capture réelle n'a été effectuée par cette revue.
