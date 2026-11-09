# Modèles locaux

Le catalogue propose deux modèles Whisper multilingues (français et anglais) : **Tiny**, choix par défaut adapté au CPU et à la mémoire disponible, et **Base**, plus lourd. **Tiny est installé dans cette copie du projet ; Base reste facultatif et doit être installé explicitement.** L'application utilise le CPU en `int8`. La qualité relative doit être mesurée avec vos propres enregistrements consentis ; le test de silence ne mesure pas la précision.

Le catalogue versionné `config/models.json` épingle les révisions officielles Systran. Les poids, tokenizers et reçus restent dans `models/<id>/`, ignoré par Git. Aucun téléchargement n'est déclenché au démarrage ni pendant une dictée.

Installation explicite, depuis la racine du projet, avec les dépendances moteur installées :

```sh
.venv/bin/python scripts/install_models.py tiny
# Ajouter Base (environ 145 Mo de poids) :
.venv/bin/python scripts/install_models.py base
# Installer les deux : scripts/install_models.py sans argument.
# Vérification de Tiny sans réseau :
.venv/bin/python scripts/install_models.py tiny --verify-only
# Vérifier les modèles effectivement installés avec le moteur de production :
.venv/bin/python scripts/check_models.py
# Vérifier explicitement Tiny :
.venv/bin/python scripts/check_models.py --model tiny
```

L'installateur utilise l'accès public Hugging Face sans jeton (`token=False`), contrôle la révision, la taille des fichiers et le SHA-256 officiel des poids. Il calcule aussi les empreintes des autres fichiers et conserve un manifeste `installation.json`. Les transferts incomplets restent dans `.tiny.staging` ou `.base.staging`, pour reprise explicite ; seul un téléchargement vérifié est publié dans le répertoire final. Les liens symboliques sont refusés. Un dossier final existant sans reçu reconnu est conservé et l'installation échoue ; déplacez-le vous-même si nécessaire. Une installation reconnue est revérifiée et réutilisée sans accès réseau.

Si le transport Xet ne progresse pas sur votre connexion, interrompez uniquement l'installateur et relancez-le avec `HF_HUB_DISABLE_XET=1 .venv/bin/python scripts/install_models.py base` pour utiliser HTTPS. Le dossier final ne sera visible dans l'application qu'après vérification complète.

Dans l'application native, ouvrez **Modèle de transcription**, choisissez un modèle installé, puis cliquez sur **Préparer le moteur**. La liste ne montre que les installations complètes. Après une installation faite pendant que l'application est ouverte, utilisez **Actualiser les modèles**. Le libellé **Modèle actif** confirme le moteur chargé ; changer le choix seul ne le charge pas. L'option **Dossier personnalisé** accepte un autre modèle compatible. Le choix reste limité à la session actuelle.

Le contrôle hors ligne charge le vrai `Transcriber`, utilise un WAV de silence synthétique dans un dossier privé, et attend `NO_SPEECH`. Les connexions Python sont bloquées pendant ce contrôle et les bibliothèques utilisent uniquement les fichiers locaux. Aucun microphone n'est enregistré.

Le moteur définit aussi `ORT_DISABLE_TELEMETRY=1` avant le premier chargement des bibliothèques de transcription, pour désactiver la télémétrie native d'ONNX Runtime utilisée par la détection de parole. Cette protection suit la [documentation de confidentialité ONNX Runtime](https://github.com/microsoft/onnxruntime/blob/main/docs/Privacy.md). Le blocage des connexions Python dans le contrôle est une vérification supplémentaire, sans être un pare-feu système.

Les dépendances moteur épinglent PyAV 18.1.0 : faster-whisper 1.2.1 utilise encore l'argument `metadata_errors`, supprimé par PyAV 19. Si votre environnement avait été créé avant cette correction, réinstallez `engine/requirements.lock.txt` dans `.venv`. Le test `engine/tests/test_audio_dependency.py` détecte cette incompatibilité sans modèle ni réseau. Sources : [versions PyAV](https://github.com/PyAV-Org/PyAV/releases), [incident amont faster-whisper](https://github.com/SYSTRAN/faster-whisper/issues/1589).

Les modèles sont distribués sous licence MIT, déclarée par leurs fiches officielles ; les README amont sont inclus. Sources : [Tiny](https://huggingface.co/Systran/faster-whisper-tiny), [Base](https://huggingface.co/Systran/faster-whisper-base), [téléchargements Hugging Face](https://huggingface.co/docs/huggingface_hub/guides/download). Les dépôts ne contiennent pas de fichier `preprocessor_config.json` ni de fichier de licence séparé aux révisions épinglées.
