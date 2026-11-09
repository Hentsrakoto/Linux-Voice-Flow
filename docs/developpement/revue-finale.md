# Revue finale indépendante — T-01 à T-03 et prototype T-06

Revue statique du 3 novembre 2026, avant la fin de la préparation Rust menée par l'intégrateur. Références : CDC (T-01/T-02/T-03, F-01/F-02/F-04/F-05/F-06/F-09/F-10, NF-01/NF-06/NF-08), lots 1/3/4 et prototype du lot 2 du plan, rapports T-01/T-02/T-03, revue native et protocole. Périmètre : frontend, modules Rust, adaptateur Tauri, configuration et capacités, moteur Python, schéma/fixtures et verrous de dépendances. Aucun code modifié, aucune installation et aucune suite générale réexécutée.

## Constat actionnable

### P2 — Une réponse de remplacement tardive écrase un événement clavier récent

**Emplacement :** `src/features/hotkey/HotkeyProbe.tsx:57–60`, particulièrement `setStatus(value)` ligne 60.

Le chargement initial protège son snapshot avec un compteur de révision, mais `apply` applique inconditionnellement la réponse de `replace_hotkey`. Après que Rust a construit le snapshot de retour (par exemple `held: false`, compteurs 0/0), un appui peut être consommé par le relais et son événement reçu par React avant la résolution de la commande. L'écouteur affiche alors « Appuyé », puis `setStatus(value)` le remplace par « Relâché » et remet les anciens compteurs. L'affichage reste faux jusqu'au prochain événement. Le même ordre avec un relâchement laisse affiché « Appuyé » après le relâchement physique. La recette du prototype peut donc présenter un état contraire au backend alors que les événements natifs ont bien été traités.

**Reproduction déterministe à ajouter :** résoudre le statut initial, laisser la promesse `replace_hotkey` en attente, envoyer un événement plus récent via le callback `listen`, puis résoudre la promesse avec le snapshot antérieur. Vérifier que l'état et les compteurs récents restent visibles. Le test existant couvre uniquement `get_hotkey_status`, pas cette séquence.

**Correction recommandée :** ajouter une révision monotone aux snapshots Rust, incrémentée au remplacement réussi et à chaque transition physique acceptée, puis ignorer les révisions antérieures dans un unique chemin fonctionnel de mise à jour React. Un simple compteur local d’événements ne suffit pas à ordonner un événement de l’ancien raccourci et une réponse plus récente de remplacement. L’intégrateur a annoncé cette correction pendant la revue ; sa livraison et sa vérification restent à constater.

Constat déduit directement de l'ordre des appels et des deux écritures de statut ; aucune reproduction graphique native n'est revendiquée.

## Verdict de conformité au périmètre

Les livrables logiciels attendus sont présents : socle Tauri/React et commande de métadonnées, contrat JSONL partagé, machine à états pure et CLI de transcription locale. Le périmètre réellement disponible est annoncé dans l'UI. Les fonctions ultérieures (capture, supervision Rust/Python, focus et insertion, configuration persistante) restent correctement séparées et ne sont pas traitées comme des omissions de T-01 à T-03.

La validation complète des lots reste conditionnée aux vérifications explicitement documentées : compilation/recette desktop sur Cinnamon/X11, installation runtime Python complète, modèle et fixtures de parole réels hors réseau. L'absence de WebKitGTK 4.1 ou de modèle réel n'est pas un défaut du code. La préparation Rust et le verrou Cargo étaient encore en cours au moment de cette revue ; le verrou absent à cet instant n'est pas présenté comme un résultat final de l'intégration.

## Verdict de qualité

Une correction P2 est demandée pour la cohérence du statut du prototype. Aucun autre défaut actionnable identifié dans le périmètre inspecté. Ce verdict est une revue statique, pas une certification de compilation ou de fonctionnement matériel.

Le relais natif actuel supprime la dépendance circulaire signalée dans la première revue : le callback dépose un tuple dans la file sans attendre le verrou du gestionnaire ; le consommateur conserve l'événement pendant la transaction. Son drapeau d'arrêt est contrôlé pendant l'attente du verrou et la boucle `run_return` permet la jonction après arrêt. L'ancien interblocage n'est donc pas réémis comme constat.

Les contrôles du modèle exigent notamment un tokenizer local complet avant l'import/chargement, et le chargement demande le mode local uniquement. Le worker d'inférence ne publie jamais directement ; le thread de contrôle neutralise les résultats annulés et attend la fin du worker avant `cancelled`. Les parseurs bornent les messages, rejettent les types/champs invalides et partagent les fixtures ; la machine n'émet l'insertion qu'une fois pour la session attendue et fournit la garde de dernière minute documentée. La mise en œuvre de cette garde par le futur contrôleur reste à vérifier lors de T-07.

Les commandes exposées et leur manifeste ACL sont cohérents à la lecture ; aucune permission système arbitraire côté WebView n'a été ajoutée. Le désabonnement tardif et le snapshot initial du frontend sont protégés. Un contrôle Python en lecture seule a comparé les dépendances directes et leurs versions entre `package.json` et `package-lock.json` : correspondance exacte. Les verrous Python déclarent explicitement leur plateforme et leurs limites d'installation ; aucune incohérence supplémentaire n'a été identifiée à la lecture. Les résultats de tests cités dans les rapports existants n'ont pas été réexécutés ni présentés comme des preuves nouvelles de cette revue.

## Contre-revue ciblée — constat P2 résolu

La correction a été relue dans `HotkeyProbe.tsx`, son test de régression,
`hotkey.rs` et `desktop.rs`. Le gestionnaire Rust possède désormais une révision
incrémentée après un remplacement réussi et après chaque transition physique
acceptée. Une répétition ignorée n'incrémente pas cette révision. Les snapshots
construits sous le verrou du gestionnaire exposent cette même valeur.

Les trois sources frontend — snapshot initial, événements et réponse du
remplacement — passent toutes par une mise à jour fonctionnelle et le comparateur
`latestSnapshot`. Une réponse ancienne ne peut donc plus remplacer un état dont
la révision est supérieure. Le nouveau test suspend explicitement la réponse du
remplacement, publie un appui plus récent, puis résout la réponse ancienne et
vérifie l'état, les compteurs et le raccourci affichés.

**Verdict actualisé : le seul constat P2 est corrigé ; aucun constat actionnable
reste ouvert dans cette revue.** Cette contre-revue est limitée à la correction,
sans nouvelle suite de tests exécutée par le relecteur. L'intégrateur rapporte
un test Rust de révision passé de RED à GREEN, puis 18 tests Rust headless verts,
et l'agent frontend rapporte la régression passée de RED à GREEN avec 8 tests
verts ; le contrôle final consolidé relève de l'intégrateur.

Mise à jour de contexte communiquée par l'intégrateur : Mint 22.2 Cinnamon/X11
identifié, `Cargo.lock` désormais produit avec 427 paquets ; WebKitGTK 4.1 et
librsvg 2 manquent encore pour la validation desktop. Les réserves de recette
native et de transcription sur modèle réel restent des limites de validation,
sans remettre en cause la résolution du défaut de snapshots.
