# Corrections CI, raccourci et diagnostic microphone — 9 novembre 2026

## Changements

- CI : runner Ubuntu 24.04, dépendances natives Tauri/ALSA/X11, Rust stable, Node 24 et build des ressources frontend avant compilation desktop. Build et tests utilisent explicitement `src-tauri/Cargo.toml` et `--locked`.
- F-02 : la commande de compatibilité `replace_hotkey` passe par une action de l’acteur de dictée. La garde d’inactivité est revérifiée à l’exécution, dans la même boucle que les démarrages de capture et les transactions de réglages. Elle ne modifie pas les options d’insertion. Aucun changement de raccourci n’est appliqué si le runtime est indisponible.
- F-14 : l’énumération CPAL ne propose le périphérique par défaut que s’il existe. Le diagnostic vérifie le microphone sélectionné ; un périphérique absent ou un nom ambigu ne produit plus un diagnostic positif. Ce contrôle ne démarre pas de capture et ne garantit pas les permissions ou l’ouverture future du flux.

## Régressions couvertes dans le code

- Action demandée au repos, puis dictée démarrée avant son exécution : refus sans appeler le changement de raccourci.
- Refus pendant les phases actives, y compris avant mise en file pendant insertion ; application au repos sans modification des options.
- Diagnostic sans périphérique, sans périphérique par défaut, avec sélection déconnectée, périphérique présent et noms en double.

## Vérifications sur ce poste

- Chargement YAML du workflow avec PyYAML, syntaxe de chaque script avec `bash -n`, chemins Cargo et présence des étapes natives/frontend : réussis.
- `git diff --check` : réussi.
- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked` : non exécuté, `cargo` introuvable. Rust/Cargo absents ; la tentative d’accès à `static.rust-lang.org` échoue à la résolution DNS.

Les nouveaux tests Rust et la compilation desktop restent à exécuter avec la chaîne Rust et les dépendances natives. Aucun microphone, raccourci global ou service Groq n’a été activé pour ces vérifications. La CI GitHub n’a pas été déclenchée depuis cette session.


## Validation complémentaire du 10 novembre 2026

Après préparation temporaire de Rust et du SDK ALSA, les régressions du raccourci et du diagnostic microphone ont été compilées et exécutées avec succès dans la suite `--no-default-features --features native-audio --locked` : 76 tests unitaires et 12 tests de processus réussis dans l’état final. Le build desktop complet et la CI GitHub restent à vérifier séparément. Voir `bilan-t07-t08.md` pour la finalisation de l’interface et le test X11 synthétique.
