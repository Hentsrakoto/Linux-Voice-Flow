# Raccourci sans effet — 10 novembre 2026

## Diagnostic

L’application desktop est lancée sous X11. Les réglages sauvegardés sélectionnent Groq et le raccourci canonique `control+alt+Space`. La configuration Groq est lisible, sans validation distante de la clé. En revanche, `.venv/bin/python` est absent et aucun moteur enfant ne tourne. Le backend ne démarre aucune capture tant que le moteur n’est pas prêt ; cette absence explique un blocage concret du parcours. La réception physique du raccourci et la dictée au microphone restent à vérifier par l’utilisateur après préparation.

## Corrections

- Création de `.venv` et préparation des 23 dépendances runtime verrouillées. Les téléchargements interrompus sur cette connexion sont repris avec curl ; les roues sont sélectionnées selon les tags CPython 3.12 du poste puis vérifiées par taille et SHA-256 publiés par PyPI. L’installation finale est locale depuis ces roues.
- Vue Dictée : expliquer la préparation nécessaire au lieu de demander de parler alors que le moteur est indisponible ; après un événement effectivement reçu, afficher « Raccourci reçu » à partir du compteur backend. Aucun faux événement généré.
- Rendu du raccourci : accepter la casse canonique du backend (`control`, `alt`, etc.) et afficher Ctrl/Alt/Espace.
- Erreur moteur : mentionner Python et la vérification des dépendances avant nouvelle préparation.
- README et guide Groq : expliciter que Python et le VAD local sont aussi requis en mode API.

## Vérifications

- Tests React : 31 réussis ; après l’ajout du message « Raccourci reçu », les 10 tests ciblés DictationPanel sont de nouveau réussis.
- Typecheck, lint et build frontend : réussis dans l’état final.
- `cargo check --manifest-path src-tauri/Cargo.toml --locked --offline` : compilation desktop complète réussie avec GTK/WebKit/CPAL.
- CLI de production avec le Python du projet, fournisseur Groq et EOF : message JSONL `ready`, sortie 0 et fermeture propre. Aucun microphone ni appel à l’endpoint audio.
- Installation des 23 bibliothèques runtime verrouillées : réussie. `pip check` : aucune dépendance cassée. Test du vrai GroqTranscriber/VAD sur un WAV de silence synthétique privé : `NO_SPEECH`, transport HTTP interdit et connexions Python bloquées ; fichier temporaire supprimé. CLI Groq de production revérifiée après installation : `ready`, fermeture propre. Aucun audio envoyé à Groq.

L’application déjà ouverte conserve son précédent échec : une fois les dépendances installées, utiliser **Réglages → Enregistrer et préparer**, attendre **Prêt à dicter**, puis maintenir le raccourci pendant la parole et le relâcher. Aucun modèle Whisper local n’est requis pour Groq. Aucun secret ni réglage utilisateur n’a été modifié.
