# Revue indépendante T-07 / T-08 — 7 novembre 2026

## Périmètre et méthode

Lecture du CDC (F-02, F-07/F-08, focus, annulation), du plan lots 7/8, du contrat `plan-t07-t08.md`, des nouvelles unités Rust et du parcours React. Revue statique indépendante, sans modification de l’implémentation, sans lecture de `.env`, sans microphone, API de transcription ou interaction avec un vrai chat. Les emplacements ci-dessous désignent la version inspectée pendant le travail ; les agents corrigent en parallèle.

## Constats transmis pour correction

1. **P2 — Avertissement d’insertion partielle perdu après copie.** `dictation.rs`, `deliver` (vers 490–514), et `DictationPanel.tsx`, message de livraison : un échec après un bloc peut laisser du texte dans la cible. La copie consentie remplace `Fallback` par `Clipboard`, puis recommande simplement de coller. Le clic Copier manuel a le même effet. Conserver une indication d’insertion incertaine même après copie, et avertir avant de coller le texte entier.
2. **P2 — Restauration du raccourci absent.** `desktop.rs`, transaction `save_settings` : quand aucun raccourci n’était enregistré (conflit au lancement), un échec de publication après enregistrement du nouveau ne le supprimait pas. **Correction observée en relecture :** branche `None => manager.clear()`. Le diagnostic d’erreur initial est également remis à zéro après sauvegarde réussie.
3. **P2 — Commande de compatibilité hors garde de session.** `desktop.rs`, `replace_hotkey` (vers 145) : la commande ne passe pas par la sérialisation des réglages avec l’acteur. `held` protège l’enregistrement, mais pas la transcription après relâchement. Elle peut modifier le raccourci pendant une opération active. La faire passer par la même garde transactionnelle que Réglages.
4. **P2 — Jointure de helper non bornée avec descendants.** `insertion.rs`, `terminate` (85), `run` (156) : lorsque le parent est déjà terminé, le groupe n’est pas arrêté. Un descendant gardant les pipes ouverts peut bloquer les threads puis leur `join` au-delà du timeout. Cas reproductible à couvrir par un helper fictif qui lance un descendant et quitte. Risque réduit avec les binaires officiels, mais la garantie de temps borné annoncée dépasse le comportement actuel.
5. **P2 — Dernière vérification d’annulation avant frappe.** `insertion.rs`, boucle `insert` : la vérification de cible peut observer l’annulation après son dernier appel, puis la frappe est lancée avec un callback toujours faux. Vérifier explicitement l’annulation après la capture de cible et juste avant le lancement du bloc ; garder documentée la limite d’atomicité X11 et l’impossibilité de retirer une frappe déjà livrée.
6. **P2 — Raccourci affiché sans enregistrement actif.** `DictationPanel.tsx` (vers 21) : le fallback `Ctrl+Alt+Space` est présenté comme commande même si `hotkey.shortcut` vaut null. L’alerte située plus bas ne rend pas ces touches réellement actives. Réserver cet exemple à l’aperçu navigateur et afficher un état indisponible en natif.
7. **P3 — Message de récupération trompeur.** `SettingsPanel.tsx` : l’erreur de lecture indique qu’il faut corriger le fichier avant de pouvoir enregistrer ; le contrat et `SettingsStore.commit` autorisent au contraire un remplacement explicite. Expliquer cette action disponible.
8. **P3 — Test corruption/version insuffisant.** `settings.rs`, `corrupt_or_unknown_version_is_preserved` : les fichiers créés par `fs::write` ont normalement 0644, donc la validation s’arrête aux permissions. Le JSON version99 est de plus incomplet. Employer 0600 et un objet Settings complet de version inconnue pour exercer les chemins annoncés.

Autre limite : l’insertion synchrone occupe l’acteur. Les commandes Copier/Options arrivées pendant ce bloc sont traitées après son retour, éventuellement en état Success, plutôt que refusées à leur réception. L’UI désactive ces actions ; la garde de l’acteur protège l’état au moment de l’application, pas strictement celui de réception IPC.

## Points conformes observés

- Insertion reliée à une session et à une génération ; résultat tardif rejeté et pas de retry automatique après insertion incertaine.
- Fenêtre active et enfant X11 mémorisés puis revérifiés entre blocs ; pas de reprise forcée du focus. Limite de curseur dans une même fenêtre expliquée.
- Texte envoyé via stdin sans shell ; contrôles, retours et tabulations refusés avant la première frappe. Aucun envoi explicite de Return/Tab.
- Copie automatique désactivée par défaut ; action Copier sans texte fourni par le frontend. Effacement limité au processus propriétaire xclip, sans restauration d’une ancienne copie extérieure.
- Phase inserting incluse dans busy côté React/Rust et dans l’activation d’Échap ; annulation urgente et epoch consultées par la livraison.
- Réglages versionnés, publication atomique, permissions privées, ancien fichier conservé à la lecture invalide, restauration au démarrage et préparation sans capture automatique.
- Accueil allégé ; service/modèle/langue/microphone/raccourci déplacés vers Réglages. Consentement Groq visible avant enregistrement. Aperçu navigateur annoncé comme non fonctionnel.

## Vérifications et verdict provisoire

Le rapport `t07-output-report.md` annonce 10 tests insertion et 1 test focus réussis, avec helpers simulés. Ce sont des **tests rapportés**, pas relancés par cette revue. Les rapports backend/UI étaient encore absents lors de cette première passe. Aucune recette visuelle ou X11 n’a été réalisée par le relecteur. La recette GTK/WebKit sur Xvfb du parent devra être citée séparément de toute dictée réelle microphone.

L’architecture correspond aux invariants principaux ; validation finale réservée jusqu’aux corrections ci-dessus et aux résultats de vérification. Une recette synthétique X11 ne suffit pas à déclarer le jalon « dictée réelle » acquis.
