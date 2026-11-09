# T-01 — Socle frontend

État au 3 novembre 2026 : React/TypeScript strict, Vite, Tailwind v4 via plugin Vite, ESLint et Vitest/Testing Library initialisés. Les versions directes sont fixées dans `package.json`.

## Périmètre

Écran français avec état « Configuration requise ». La commande Rust `get_app_info` alimente le nom et la version. La réponse confirme seulement la communication desktop. Aucun contrôle d’enregistrement ou de transcription simulé. Les états chargement, erreur récupérable et aperçu navigateur sont explicites; un bouton natif permet de réessayer. Les erreurs techniques ne sont pas rendues ni journalisées.

Les tests `src/app/App.test.tsx` couvrent chargement/réponse, erreur/réessai et navigateur sans appel IPC. Seule la frontière `@tauri-apps/api/core` est simulée. Les tests ont été écrits avant l’implémentation; le premier lancement était bloqué par Vitest absent, donc aucune preuve rouge fonctionnelle n’a été obtenue à cette étape.

## Contexte design

`DESIGN.md` documente le registre utilitaire, les couleurs, fontes système hors ligne et la propriété des tokens. `src/styles.css` constitue la source canonique d’exécution. Écran unique sans formulaire ni CRUD : pas de contrat multiscreen anticipé. Audit statique premium exécuté avec Python 3 : zéro constat (rapport `/tmp/voice-flow-premium-audit.json`). Il ne prouve pas l’accessibilité en exécution.

## Vérifications et limites

Node v24.14.0, npm 11.9.0. Installation npm initiale sans élévation interrompue après absence de progression réseau; seconde tentative réseau autorisée. Les résultats finaux des contrôles sont complétés ci-dessous. Le lint DESIGN.md via `npx -p @google/design.md designmd lint DESIGN.md` retourne zéro erreur et zéro avertissement.

Contrôles manuels à exécuter dans la fenêtre desktop réelle : réponse Rust visible; navigation Tab jusqu’au lien et au bouton de réessai en cas d’erreur; focus visible; lecture à 200 %; taille étroite 320px et desktop 1040px. Aucun de ces contrôles visuels ou matériels n’est revendiqué sur la seule base du code. Aucun navigateur système ni WebKitGTK n’a été confirmé disponible par cet agent.

## Résultats exécutés

- `npm run typecheck` : code 0.
- `npm run lint` : code 0.
- `npm run test -- --run` : 2 fichiers, 7 tests réussis.
- `npm run build` : code 0, Vite 7.2.2, 33 modules, JS 200,27 ko (63,13 ko gzip), CSS 8,99 ko.
- Audit premium strict après ajout du prototype : code 0, zéro constat.
- Lint `DESIGN.md` : zéro erreur, zéro avertissement.

Extension T-06 demandée pendant le lot : disclosure natif `HotkeyProbe`, visible uniquement après connexion desktop, statut Rust réel, compteurs appui/relâchement, formulaire de remplacement avec erreurs conservant le raccourci actif, abonnement sûr et désabonnement même après démontage. Les tests vérifient aussi qu’un événement récent ne soit pas écrasé par une réponse initiale tardive. Aucun enregistrement audio.

Installation : le réseau imposait environ neuf minutes pour le premier `npm install`; reprise IPv4 et `--legacy-peer-deps --ignore-scripts --no-audit --no-fund` pour éviter les adaptateurs optionnels non utilisés de Vite. La dépendance pair Testing Library DOM est explicite. Le build confirme les binaires locaux nécessaires. npm signale ESLint 9.39.1 désormais hors support; c’est un outil de développement, une mise à jour majeure reste à prévoir. Aucun audit de vulnérabilités npm n’est revendiqué (`--no-audit`).

Verrou final : `npm install --package-lock-only --ignore-scripts --no-audit --no-fund` sans option legacy résout le graphe standard après la préparation lente. Le verrou contient explicitement `@testing-library/dom@10.4.1`; ne pas imposer `--legacy-peer-deps` aux installations usuelles. Le contrôle `npm ci --dry-run --offline --ignore-scripts --no-audit --no-fund` vérifie ensuite la cohérence manifeste/verrou sans prétendre avoir réalisé une réinstallation propre. Les contrôles frontend ont été exécutés sur les dépendances installées.

Sources officielles consultées par l’agent coordinateur avant implémentation : [Vite et Tauri](https://v2.tauri.app/start/frontend/vite/), [commandes Rust](https://v2.tauri.app/develop/calling-rust/), [capacités Tauri](https://v2.tauri.app/security/capabilities/), [Tailwind avec Vite](https://tailwindcss.com/docs/installation/using-vite). Les skills frontend-design et frontend-design-premium ont été lus pour le registre visuel et l’audit; les références canonical-ui-resolution, design-context-lifecycle et token-mapping ont guidé la propriété des tokens.

Vérification finale après ajustement des états : `typecheck`, `lint` et `build` retournent tous code 0; les 7 tests passent à nouveau. `npm ci --dry-run` standard retourne code 0. Le serveur Vite de contrôle est disponible sur `http://127.0.0.1:1420/`; la validation visuelle est à consigner par le coordinateur.

## Correction après revue : ordre des réponses X11

La revue a identifié qu’une réponse tardive de `replace_hotkey` pouvait écraser un événement physique plus récent. Régression reproduite avant correction : le nouveau test échouait en affichant « Relâché » et `0 / 0` après un événement « Appuyé », `1 / 0`. Tous les snapshots possèdent désormais `revision:number` monotone fourni par Rust. Les trois entrées (événement, réponse initiale, remplacement) utilisent un setter fonctionnel qui accepte seulement une révision supérieure ou égale à la révision affichée. La réponse ancienne ne remplace donc ni l’état ni les compteurs récents. Le nouveau test passe et la suite complète compte **8 tests réussis**. L’interface précise aussi que le raccourci du prototype ne persiste pas après redémarrage; la sauvegarde relève de T-08.


## Complément du coordinateur — rendu et backend

L’aperçu navigateur a été réellement inspecté avec le navigateur intégré : fenêtre large puis étroite, contenu 375 px pour 375 px disponibles sans débordement horizontal. Les états de la fenêtre native restent à vérifier. Rust 1.99.1 et Cargo sont préparés temporairement ; les 18 tests métier passent, y compris les métadonnées, le raccourci et le relais. Le diagnostic Tauri constate Mint 22.2/Cinnamon/X11 mais WebKitGTK 4.1 et rsvg2 absents. Le build desktop complet et le lancement natif ne sont pas validés. Voir bilan consolidé pour les commandes exécutées et les contrôles manuels.
