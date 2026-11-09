# Bilan Local / Groq — 4 novembre 2026

L’application permet de choisir la transcription locale ou Groq. Le choix reste limité à la session et doit être appliqué par « Préparer le moteur ». Aucun basculement automatique vers l’API. Cette extension du périmètre a été demandée explicitement ; le CDC et le protocole sont mis à jour.

## Étapes livrées

1. **Python** : adaptateur Groq sans dépendance supplémentaire, lecture backend de `.env`, transport multipart HTTPS borné, VAD local avant upload, erreurs contrôlées. Validation des WAV privés partagée avec le local. Voir `groq-python-report.md`.
2. **Rust** : fournisseur typé Local/Groq, propagation au processus et snapshot du service actif. Le mode Groq ne demande aucun poids Whisper local. Codes API synchronisés dans les contrats. Voir `groq-rust-report.md`.
3. **Interface** : sélecteur natif, configuration locale conditionnelle, information du transfert audio et distinction choix/service actif. Messages correctifs sans réponse brute ni champ secret. Voir `groq-ui-report.md`.
4. **Configuration et documentation** : `.env.exemple` versionné et `.env` initialisé sans clé, en 0600, sans écraser un fichier existant. `.env` reste ignoré ; la clé et le modèle sont lus par Python uniquement. Guide `docs/groq.md`, suivi et CDC mis à jour.

## Vérifications

- Python : **79 tests réussis, 1 test de parole réelle sauté, 77 sous-tests**. Transports Groq simulés, pas de vraie requête ni de micro enregistré. Le vrai moteur Tiny reste opérationnel hors ligne sur silence : chargement 0,925 s, contrôle total 1,007 s, `NO_SPEECH`.
- Interface : **30 tests réussis**, typecheck, lint sans avertissement, build et audit UI strict réussis. Nouvelle vérification complète après corrections de revue.
- Rust : **45 tests unitaires et 12 tests de processus**, formatage, compilation native et Clippy tous targets sans avertissement réussis. Compilation incrémentale revérifiée après correction d’un libellé.
- Revue indépendante des sources et contrats : pas de défaut important confirmé. Deux messages mineurs ont été corrigés : transcription neutre entre fournisseurs, et nécessité de préparer le moteur à nouveau après erreur distante. Voir `groq-review-report.md`.

## À vérifier sur le poste

Remplir `GROQ_API_KEY` et `GROQ_MODEL` dans `.env`, relancer Tauri, choisir Groq et préparer le moteur. Dicter une courte phrase consentie, vérifier le texte, puis revenir au local en le préparant explicitement. Une préparation Groq vérifie la configuration locale sans tester la clé à distance ; son authentification et son quota sont vérifiés lors de la première transcription.

Vérifier aussi l’annulation et la détection de langue avec le microphone réel. Annuler ne retire pas un audio déjà envoyé à Groq. Le compte, le quota et le réseau réels ne sont pas validés par les tests simulés. L’insertion dans les autres applications reste l’étape T-07.
