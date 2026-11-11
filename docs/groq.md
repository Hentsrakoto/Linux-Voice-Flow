# Choisir la transcription Groq

Le mode API nécessite aussi le Python du projet et ses dépendances : `python3 -m venv .venv`, puis `.venv/bin/python -m pip install -r engine/requirements.lock.txt`. La détection de parole reste locale. Après installation, cliquez sur **Enregistrer et préparer** et attendez **Prêt à dicter** avant de maintenir le raccourci.

Le mode local conserve le fonctionnement hors ligne. Le mode **Groq — API** envoie le WAV de la dictée au service Groq quand vous relâchez le raccourci. Il nécessite Internet et un compte Groq. Les quotas et la facturation dépendent de votre compte ; l’application ne garantit pas la gratuité du service.

## Configuration

Depuis la racine du projet, créez le fichier de configuration privé :

```sh
cp -n .env.exemple .env
chmod 600 .env
```

Ces commandes conservent un `.env` déjà présent. Ouvrez ensuite `.env` dans votre éditeur et renseignez :

```dotenv
GROQ_API_KEY=votre_cle_groq
GROQ_MODEL=whisper-large-v3
```

Pour privilégier la rapidité, vous pouvez utiliser `whisper-large-v3-turbo`. La clé se crée dans [la console Groq](https://console.groq.com/keys). `.env` est ignoré par Git ; `.env.exemple` ne contient aucun secret. N’utilisez pas de variable `VITE_` pour la clé : la configuration est lue par le moteur Python, sans passer par React ou les commandes IPC.

Les variables d’environnement `GROQ_API_KEY` et `GROQ_MODEL`, si elles sont déjà définies, ont priorité sur le fichier. Le fichier accepte des valeurs simples, éventuellement entre guillemets ; il n’est jamais exécuté comme un script. Les modifications sont relues à la prochaine préparation du moteur.

## Utilisation et vérifications

1. Lancez `npm run tauri dev`.
2. Sélectionnez le mode **Groq** dans le panneau de dictée, puis choisissez le microphone et la langue.
3. Dans **Réglages**, cliquez sur **Enregistrer et préparer** et vérifiez l’indication du mode actif.
4. Maintenez le raccourci pour dicter une courte phrase, puis relâchez-le. Vérifiez le résultat dans l’application.
5. Sélectionnez **Local**, choisissez un modèle installé et préparez à nouveau le moteur pour revenir au fonctionnement hors ligne.

Une sélection dans le formulaire ne remplace pas le moteur actif avant la préparation. Le mode API ne demande aucun poids Whisper local. La validation des WAV privés, la limite de 60 secondes, le VAD local, l’annulation et l’effacement du texte après 120 secondes sont conservés. Aucun basculement automatique vers Groq n’a lieu si le mode local échoue.

Une clé absente ou invalide, un quota atteint et une panne réseau donnent des messages contrôlés ; les réponses brutes du fournisseur ne sont pas affichées. Préparer le mode Groq vérifie sa configuration locale sans appeler l’API : la validité de la clé et le quota sont vérifiés par Groq lors de la première transcription.

Le client HTTP utilise un identifiant d’application explicite pour éviter le refus observé avec l’identifiant urllib par défaut. Une erreur 401 indique un refus d’authentification ; un 403 relève d’un refus d’accès et n’est pas présenté comme une clé invalide. Après une correction du moteur ou une modification de `.env`, préparez le moteur à nouveau. Un avertissement concernant Échap est indépendant de l’API : le bouton Annuler permet d’arrêter la dictée si le raccourci n’est pas disponible.

Annuler abandonne le résultat dans l’application et arrête le moteur si nécessaire. Cela ne peut pas retirer un audio déjà transmis au service. En choisissant Groq, la conservation de l’audio chez le fournisseur relève aussi de [ses règles de traitement des données](https://console.groq.com/docs/your-data).

## Architecture et tests

Le moteur conserve le protocole JSONL et le superviseur Rust existants. Le fournisseur est choisi lors de la préparation ; seule l’implémentation Groq peut appeler l’endpoint fixe de transcription. La clé reste dans le processus Python et n’est jamais transmise dans un argument de ligne de commande. Les tests automatisés remplacent le transport distant ; ils n’enregistrent aucun microphone et n’utilisent aucune vraie clé.

Références : [transcription Groq](https://console.groq.com/docs/speech-to-text), [limites du service](https://console.groq.com/docs/rate-limits).
