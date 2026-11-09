# Correction du faux refus de clé Groq — 4 novembre 2026

Le fichier `.env` a été contrôlé sans afficher sa clé : configuration valide, fichier privé et aucune variable d’environnement remplaçant la valeur lue sur le poste de diagnostic. La clé n’a pas été modifiée.

Une comparaison sur l’endpoint officiel de liste des modèles a reproduit le problème : la requête urllib avec son identifiant de client par défaut recevait HTTP 403, tandis que la même requête avec `User-Agent: LinuxVoiceFlow/0.1` recevait HTTP 200 et proposait le modèle configuré. Le refus était lié au filtrage du client HTTP, pas à une clé invalide.

Le transport de transcription envoie désormais cet identifiant explicite. La conversion des erreurs a également été corrigée : seul HTTP 401 produit `API_AUTH_ERROR`, tandis que 403 utilise `API_UNAVAILABLE` sans exposer de réponse distante. La [documentation officielle Groq](https://console.groq.com/docs/errors) distingue bien authentification 401 et restrictions d’accès 403. Les régressions multipart et HTTP ont été observées rouges avant correction, puis vertes.

La notice Échap était répétée par le panneau, qui ajoutait une instruction déjà présente dans le message natif. Elle est affichée une seule fois ; le message de secours contient sa propre instruction. L’inscription native d’Échap n’a pas été changée, et son éventuelle indisponibilité sur le bureau reste distincte de l’accès Groq. Le bouton Annuler reste le recours. Voir `escape-notice-fix.md`.

Vérification finale : requête réelle sur l’endpoint de transcription avec une seconde de silence synthétique, HTTP 200 et réponse interprétable par l’adaptateur de production. Le VAD a été remplacé uniquement dans ce processus de diagnostic pour permettre l’envoi de ce silence ; le VAD de production reste actif. Aucun microphone ni audio personnel n’a été utilisé. Ce test ne mesure pas la qualité de reconnaissance de parole.

Suite Python complète : **79 réussis, 1 sauté, 77 sous-tests**. Interface : **30 tests réussis**, typecheck, lint et build réussis. La clé et les corps de réponse n’ont pas été imprimés. Relancer Tauri et préparer Groq à nouveau avant une vraie dictée.
