# T-03 — Moteur Python local

Date : 3 novembre 2026. Python réellement utilisé : 3.12.3. Exigences : F-04,
F-05, F-06, F-10, NF-01, NF-06, NF-08, NF-10 ; annulation F-12 et fermeture
NF-11 à compléter par la supervision Rust T-04.

## Fonctionnement

`engine/transcriber.py` valide un répertoire modèle local absolu sans composant
symbolique et les trois ressources obligatoires `model.bin`, `config.json`,
`tokenizer.json`. Le constructeur utilise `local_files_only=True`, impose
`HF_HUB_OFFLINE=1`, désactive la télémétrie HF et ne propose aucun téléchargement.
Le format attendu est CTranslate2/faster-whisper, pas un fichier Whisper `.pt`.
Importer aussi `preprocessor_config.json` et les fichiers `vocabulary.*` fournis
par le modèle. Les dépendances et le modèle sont préparés explicitement avant
l'usage ; aucun gestionnaire de paquets n'est appelé pendant une dictée.

L'audio doit se trouver sous `--audio-root`, un répertoire de l'utilisateur en
0700. L'ouverture descend avec `openat`/`O_NOFOLLOW` sans suivre de lien, contrôle
un fichier régulier 0600, taille maximale 2 000 000 octets, WAV PCM 16 bits mono
16 kHz, durée positive au plus 60 secondes et contenu non tronqué. Le moteur lit
le descripteur validé, pas un chemin rouvert après validation. Rust conserve la
propriété et la suppression des fichiers. Le répertoire privé doit rester sous
le contrôle de l'application ; cette défense ne protège pas d'un processus
malveillant du même utilisateur qui possède déjà les mêmes accès.

`fr` et `en` sont transmis directement ; `auto` devient `None`. La VAD Silero
intégrée est activée. Les segments sont tous consommés avant une réponse ; un
texte vide devient `NO_SPEECH`. Une langue automatiquement détectée hors fr/en
est rejetée avec `NO_SPEECH` car le contrat v1 n'autorise que ces deux langues.
Cette erreur couvre donc également une langue hors du périmètre du MVP.

`engine/main.py` conserve stdin lisible par `selectors` pendant un unique worker
Python d'inférence. Les trames fragmentées sont réassemblées, avec une borne de
1 Mio et un refus d'EOF tronqué. `ready` arrive seulement après le chargement.
Un nouveau travail est refusé avec `ENGINE_BUSY` tant que le worker précédent
n'est pas arrêté, y compris après une annulation. Une retransmission du même ID
actif est ignorée pour éviter deux issues terminales pour ce travail.

`cancel` neutralise la session immédiatement, mais `cancelled` n'est publié
qu'après l'arrêt effectif du worker. Le calcul natif CTranslate2 courant n'est
pas interruptible par l'API publique utilisée. Si le worker ne rend pas la main,
Rust doit invalider l'ID puis terminer le processus après 3 secondes (T-04).
À EOF, la session est neutralisée et la boucle sort ; le worker daemon ne bloque
pas la fermeture Python. La récolte/terminaison native complète et les délais
restent la responsabilité du superviseur Rust. Aucun résultat n'est émis après
une annulation connue ou la fermeture du flux.

Le protocole utilise un descripteur stdout dédié. Les fd 1 et 2 ordinaires sont
redirigés vers `/dev/null` dans le CLI de production : des bibliothèques natives
peuvent écrire directement sur ces fd et contourner les filtres Python. Le
moteur publie seulement les erreurs normalisées, sans recopier les exceptions,
les chemins, l'audio ou le texte dans les journaux. Ce choix réduit les détails
techniques disponibles pour diagnostiquer un défaut natif.

## Commande

```sh
.venv/bin/python -m engine.main \
  --model-path /absolute/local/model \
  --audio-root /absolute/private/tempdir \
  --device cpu --compute-type int8
```

Le chemin modèle de chaque requête doit être identique au chemin canonique
chargé au démarrage. Le superviseur doit donc transmettre ce chemin canonique.
Les changements de modèle nécessitent un nouveau processus.

## Vérifications

Les tests ont été écrits avant les modules et un premier passage a constaté les
modules absents ; les tests CLI ont ensuite échoué sur l'absence de réponse de
démarrage avant l'implémentation de la boucle. La première tentative pytest était
bloquée par l'installation encore en cours ; le premier lancement RED a utilisé
les wheels pytest déjà téléchargées via PYTHONPATH, sans installation concurrente.

Commande exécutée : `.venv/bin/python -m pytest engine/tests -q` : **36 passed,
1 skipped, 69 subtests passed**. Le test sauté est `local_model`, faute des trois
variables explicites de modèle et fixtures autorisées. Couverture : modèle
absent/incomplet/corrompu simulé, absence de chargement de secours, VAD et langue,
silence simulé, erreur différée, WAV hors périmètre/liens/permissions/format/durée/
troncature, moteur occupé, annulation avant calcul et pendant un worker bloquant,
absence d'acquittement anticipé et de réponse tardive, cohérence du modèle,
fermeture, EOF tronqué, dépassement de trame/réponse, CLI par vrai processus, fragmentation et suppression des sorties
natives contenant des sentinelles sensibles. Les tests du protocole sont
réalisés par le lot T-02. Le test de régression du répertoire audio en 0777 a
d'abord échoué (`MODEL_LOAD_FAILED`), puis passe avec `PROTOCOL_ERROR` : cette
erreur désigne bien le paramétrage audio, pas le modèle.

Vérifications finales exécutées : `.venv/bin/python -m pytest -q` (même bilan),
`.venv/bin/python -m compileall -q engine` (succès), `.venv/bin/python -m pip check`
(`No broken requirements found` pour les dépendances effectivement installées).
Un contrôle `packaging` a vérifié les 23 versions runtime contre toutes les
contraintes transitives actives et `Requires-Python` pour 3.12.3 ; les 18 versions
dev correspondent à l'environnement installé et les dépendances communes ont
les mêmes versions.

## Dépendances : état réel et reproduction

`requirements.lock.txt` fixe les 23 dépendances transitives runtime pour CPython
3.12.3/Linux x86_64. `requirements-dev.lock.txt` fixe les 18 dépendances de test
et des outils, extraites de la fermeture transitive réellement installée via
`importlib.metadata` ; il inclut la contrainte du verrou runtime. Les paquets
runtime et dev restent séparés. Les verrous fixent des versions, sans hashes.

Deux tentatives réseau explicites ont été exécutées avec permission réseau :
`pip-compile --generate-hashes`, puis `pip-compile` sans hashes, et une résolution
`pip install --dry-run --ignore-installed --report`. Ces outils téléchargeaient
également les grosses wheels après leurs métadonnées. Elles ont été interrompues
pour éviter plusieurs dizaines de minutes de transfert ; le rapport pip complet
n'a donc pas été produit. Le verrou runtime a été construit depuis les
métadonnées PyPI déjà résolues et téléchargées, en parcourant récursivement
`Requires-Dist` avec les marqueurs de plateforme et en vérifiant chaque
contrainte avec `packaging`. Aucune version n'a été devinée.

La wheel faster-whisper 1.2.1 entièrement téléchargée a été inspectée avec
`zipfile` : elle contient `faster_whisper/assets/silero_vad_v6.onnx`. Aucun modèle
Whisper n'a été téléchargé. **L'installation/import de l'ensemble runtime et
l'exécution réelle de l'asset VAD ne sont pas vérifiés ici.** La suite rapide
injecte l'adaptateur natif et n'exige pas ces grosses dépendances.

Commandes de préparation à exécuter sur le poste cible, réseau autorisé
explicitement (elles ne sont pas revendiquées comme exécutées avec succès ici) :

```sh
python3 -m venv .venv
.venv/bin/python -m pip install -r engine/requirements.lock.txt
.venv/bin/python -m pip install -r engine/requirements-dev.lock.txt
.venv/bin/python -m pip check
.venv/bin/python -c 'from faster_whisper.vad import get_vad_model; get_vad_model()'
.venv/bin/python -m pytest
```

Pour régénérer le verrou après une mise à jour décidée :
`.venv/bin/pip-compile --output-file engine/requirements.lock.txt engine/requirements.in`,
puis régénérer le verrou dev dans un environnement propre. Terminer ces
préparations avant de couper le réseau pour la recette locale.

Voir `engine/tests/README.md` pour la provenance du silence généré et les variables
`LVF_TEST_MODEL_PATH`, `LVF_TEST_FR_WAV`, `LVF_TEST_EN_WAV`. Aucun modèle ni voix
n'a été téléchargé ou enregistré pour ces tests. Aucune précision linguistique,
latence réelle ou recette réseau coupé n'est revendiquée. Avant validation T-03,
préparer un modèle local complet, deux phrases fr/en autorisées et exécuter le
test `local_model` réseau coupé, puis vérifier le sens des phrases et mesurer le
temps sur un audio de 10 secondes. Bloquer les sockets Python dans le test ne
remplace pas une observation réseau système des bibliothèques natives.

## Sources primaires consultées

- [faster-whisper requirements](https://github.com/SYSTRAN/faster-whisper/blob/master/requirements.txt)
  et [setup.py](https://github.com/SYSTRAN/faster-whisper/blob/master/setup.py) :
  Python >= 3.9 ; dépendances natives à verrouiller sur la plateforme retenue.
- [Constructeur et transcription](https://github.com/SYSTRAN/faster-whisper/blob/master/faster_whisper/transcribe.py) :
  tokenizer absent peut provoquer un `from_pretrained` ; `local_files_only` seul
  ne remplace donc pas la validation du tokenizer. Les segments sont différés.
- [VAD](https://github.com/SYSTRAN/faster-whisper/blob/master/faster_whisper/vad.py)
  et [manifest](https://github.com/SYSTRAN/faster-whisper/blob/master/MANIFEST.in) :
  asset ONNX embarqué dans le paquet, pas de téléchargement VAD à la dictée.
- [API Whisper CTranslate2](https://opennmt.net/CTranslate2/python/ctranslate2.models.Whisper.html) :
  pas de paramètre d'annulation sur l'API de génération publique utilisée.

Les sources `master` évoluent ; le verrou runtime fixe les versions prévues
pour l’installation explicite. Le profil CPU initial évite les contraintes CUDA/cuDNN, à vérifier
séparément avant de proposer un profil GPU.
