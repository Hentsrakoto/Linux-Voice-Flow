# T-07 — sortie X11 et presse-papiers

## Livré

`focus.rs` expose `FocusTarget` et valide les identifiants X11. La capture mémorise fenêtre active et vrai enfant ayant le focus (`getwindowfocus -f`), vérifie leur existence et leur stabilité, et refuse notre propre application par son PID. Aucun titre de fenêtre n’est lu. Une cible sans PID vérifiable est refusée de manière conservative.

`insertion.rs` expose `DesktopOutput`, `OutputDiagnostics`, `NoopOutput` et `X11Output::new(project_root)`, avec l’API convenue dans le plan. Les diagnostics sont `x11`, `xdotool_available`, `xclip_available`. Les chemins d’outils sont résolus depuis `/usr/bin`, puis depuis l’extraction locale fixe de xdotool prévue par le plan. Aucun chemin d’exécutable ne provient de l’interface.

L’insertion utilise des appels xdotool sans shell. La transcription UTF-8 passe uniquement sur stdin, jamais dans les arguments ou les logs. Une validation de tout le texte précède la première frappe : texte vide, trop long, caractères de contrôle et séparateurs de ligne sont refusés, ce qui empêche d’envoyer Entrée/Tab ou de soumettre un chat. Le résultat original reste disponible pour le fallback manuel. Taille maximale : 64 Kio.

Le focus et les drapeaux d’annulation sont contrôlés entre blocs de 16 caractères. Chaque appel a un délai de 700 ms au maximum et l’insertion un délai global de 10 s. Les appels de frappe à délai nul terminent leur petit bloc pour laisser xdotool restaurer les modificateurs ; aucun bloc suivant n’est envoyé après annulation. Aucun retry automatique n’est effectué. Une frappe déjà livrée à X11 n’est pas rétractable : une annulation ou un changement de focus durant le dernier bloc peut laisser une insertion partielle. La position du curseur dans une même fenêtre reste une autre limite X11.

Le presse-papiers emploie xclip en premier plan (`-quiet`), conservé dans un `Child` possédé par le module. Une lecture de contrôle bornée confirme l’acquisition, sans journaliser son contenu. Effacer et Drop terminent et récoltent uniquement cet enfant ; aucune ancienne valeur n’est restaurée et aucune nouvelle copie extérieure n’est écrasée. Un gestionnaire de presse-papiers tiers peut conserver sa propre copie après transfert : ce module ne peut pas effacer l’historique d’un autre logiciel.

Les enfants sont dans leur propre groupe de processus ; les opérations bloquées sont arrêtées et récoltées. stdin et stdout sont traités par threads joints, avec tailles bornées ; stderr est jeté. Les copies mémoire intermédiaires du texte et la lecture de contrôle du presse-papiers sont remises à zéro.

## Vérifications

Les premiers tests ont été écrits avant les types et fonctions : compilation rouge sur leurs symboles absents. Après implémentation, les tests ont été verts ; des cas complémentaires couvrent les processus et le presse-papiers.

- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --offline --locked insertion::tests -- --test-threads=1` : **10 réussis**, notamment Unicode par stdin, focus changé/détruit, contrôle et annulation sans frappe, arrêt entre blocs sans retry, refus de notre fenêtre, timeout avec PID récolté, conservation d’un propriétaire précédent si copie échouée, clear et Drop du propriétaire de presse-papiers.
- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --offline --locked focus::tests -- --test-threads=1` : **1 réussi**, identifiants stricts.
- `rustfmt --edition 2021 src-tauri/src/insertion.rs src-tauri/src/focus.rs` : appliqué.

Les outils utilisés par ces tests sont des scripts isolés dans des dossiers temporaires. Aucun microphone, aucune API et aucune fenêtre utilisateur n’a été touché. La recette X11 réelle sur un serveur isolé est laissée à l’agent principal ; le présent rapport ne la revendique pas. Les tests et la compilation générale du contrôleur relèvent du lot B.
