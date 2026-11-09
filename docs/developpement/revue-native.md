# Revue native indépendante

Périmètre : `desktop.rs`, `commands.rs`, `hotkey.rs`, `build.rs`, Cargo et configuration/capacités Tauri. Revue statique uniquement, sans modification du code ni réexécution de tests. Les contrôles matériels et la compilation desktop ne sont pas revendiqués.

## Important — Interblocage entre remplacement et événement clavier

Emplacements : `src-tauri/src/desktop.rs:77–79` et `:96–97`.

`replace_hotkey` conserve le mutex `service.manager` pendant `HotkeyManager::replace`, donc pendant les appels synchrones `register` et `unregister`. Le callback clavier attend ce même mutex. Déplacer la commande dans `spawn_blocking` ne supprime pas la dépendance circulaire.

Le plugin [global-shortcut officiel](https://github.com/tauri-apps/plugins-workspace/blob/global-shortcut-v2.3.1/plugins/global-shortcut/src/lib.rs) exécute l'inscription sur le thread principal et attend son résultat. Le [backend X11 officiel](https://github.com/tauri-apps/global-hotkey/blob/dev/src/platform_impl/x11/mod.rs) envoie cette inscription à son thread d'événements puis attend sa réponse ; ce thread appelle aussi le callback clavier de façon synchrone.

Scénario : une commande de remplacement obtient le mutex alors que `held` est encore faux ; un appui de l'ancien raccourci arrive avant que le thread X11 traite l'inscription. Le callback attend le mutex ; le thread UI attend la réponse X11 ; le worker qui détient le mutex attend le thread UI. La fenêtre et le remplacement peuvent rester bloqués. C'est une déduction des dépendances synchrones, pas une reproduction matérielle exécutée.

Correction recommandée : le callback doit seulement déposer les événements dans une file sans attendre le mutex du contrôleur. Un consommateur sérialisé applique les événements et les remplacements. Une autre conception peut séparer état d'événements et transaction d'inscription, à condition de ne jamais conserver un verrou requis par le callback pendant une opération du plugin. Un simple `try_lock` qui abandonne l'événement perdrait des appuis/relâchements et n'est pas une correction suffisante.

Validation attendue : un backend synchronisé provoque un callback pendant `register` et vérifie que la transaction et l'événement finissent sans perte ; compléter par un appui concurrent au remplacement sur Cinnamon/X11.

## Verdict

Aucun autre défaut actionnable identifié dans ce périmètre. Le filtrage des répétitions, l'identité canonique du raccourci, le remplacement transactionnel et les capacités limitées sont cohérents à la lecture. La commande de métadonnées reste sans accès système arbitraire. Les erreurs de restauration sont explicitement signalées. L'interblocage ci-dessus doit être corrigé avant de considérer le remplacement utilisable en concurrence avec les événements physiques.

## Correction réalisée après la revue

Sur instruction du responsable d'intégration, `desktop.rs` utilise désormais un
relais FIFO indépendant de Tauri dans `hotkey_relay.rs`. Le callback X11 ne fait
qu'envoyer `(raccourci, appui/relâchement)` dans une file ; il ne prend plus aucun
verrou du gestionnaire. Le consommateur conserve l'événement en attente lorsque
le gestionnaire est occupé ; il réessaie sans abandonner l'appui/relâchement.
L'accès en essai est donc uniquement côté consommateur, jamais une perte
silencieuse d'événement côté callback. La file ne transporte ni audio ni texte.

Un drapeau atomique et `recv_timeout` bornent l'attente du worker. Le drapeau est
levé sur `RunEvent::Exit`, puis le worker est rejoint après la boucle. La boucle
utilise `App::run_return` : [la documentation Tauri](https://docs.rs/tauri/latest/tauri/struct.App.html#method.run)
précise que `run` termine directement le processus et ne permettrait pas ce
nettoyage après boucle. L'attente d'un verrou occupé vérifie aussi le drapeau :
la jonction du worker ne dépend pas d'une inscription encore bloquée sur un
thread UI déjà arrêté.

Deux tests de régression sont ajoutés dans le module indépendant : producteur
qui revient même avec consommateur bloqué, livraison FIFO appui/relâchement, et
arrêt du worker inactif sans déconnexion préalable du producteur. Ces nouveaux
tests restent à compiler/exécuter ; aucune réussite n'est revendiquée ici.
Commande prévue : `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features hotkey_relay`.
La recette de remplacement/appui concurrents sur Cinnamon/X11 reste nécessaire.
Le constat Important initial est donc corrigé dans le code, avec vérification
Rust et matérielle encore attendue.
