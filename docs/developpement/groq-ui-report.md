# Choix Local / Groq — interface

## Résultat

`DictationPanel` propose un sélecteur natif « Service de transcription », initialement Local, avec l’option « Groq — API ». Le mode Groq masque les modèles et chemins locaux ; leur catalogue absent, en échec ou encore en chargement ne bloque pas la préparation Groq. Les choix locaux restent conservés quand on revient à Local.

Le bouton « Préparer le moteur » transmet explicitement `provider`, avec un chemin vide pour Groq. Aucun appel de transcription ou chargement n’est déclenché au seul changement du sélecteur. La clé et le nom du modèle API restent dans la configuration du moteur ; l’interface ne collecte aucune clé et n’affiche aucun secret.

Le service actif suit le snapshot natif, distinct du service sélectionné en attente. Quand Groq demeure actif alors que Local est choisi, l’interface rappelle que l’audio partira encore vers Groq jusqu’à préparation du moteur local. Les sélecteurs restent bloqués pendant une opération active, et l’annulation garde son parcours existant. Les erreurs d’action se réfèrent au service actif, tandis que les erreurs de préparation se réfèrent au choix demandé.

Les nouveaux codes de configuration, authentification, quota et indisponibilité Groq sont traduits en messages français constants et correctifs. Les erreurs brutes du moteur ne sont jamais affichées. Le shell `App` utilise désormais un texte neutre entre local et API ; les tokens, styles et contrôles existants sont réutilisés.

## Fichiers

- `src/features/dictation/DictationPanel.tsx` et son test.
- `src/shared/ipc/runtime.ts` : provider typé ; provider snapshot optionnel pour compatibilité des consommateurs existants.
- `src/app/App.tsx` : textes neutres ; tests App existants conservés.
- `DESIGN.md`, `UX-CONTRACT.md` : propriété native du sélecteur, distinction choix/actif, absence de champ secret et information du transfert audio.

## Vérification

Les quatre premiers nouveaux scénarios ont échoué avant implémentation sur le sélecteur absent, puis réussi. Un cinquième scénario a reproduit un message d’erreur d’annulation basé sur le choix local au lieu du service Groq actif, puis réussi après correction.

Derniers contrôles : `npm run typecheck`, `npm run lint` sans avertissement, `npm run test -- --run --maxWorkers=1` et `npm run build` réussis : **30 tests** au total, dont **20 tests du panneau**. La suite conserve la confidentialité des messages, les révisions monotones, les abonnements et les cas de sélection de modèle local. Le chargement du poste sous compilation Rust concurrente a allongé les tests, sans échec.

La recette dans la fenêtre native et la transcription avec une vraie clé restent à faire. Aucun appel Groq réel ni enregistrement du microphone n’a été effectué par les tests. L’audit statique UI est coordonné par l’agent principal ; il ne remplace pas une recette clavier/WebView.

Correctif de revue : les messages Groq de quota, indisponibilité et délai dépassé indiquent désormais de préparer le moteur à nouveau avant un nouvel essai, conformément à son arrêt par le contrôleur après ces erreurs. Le scénario de quota vérifie cette instruction et la disponibilité de l’action de préparation.
