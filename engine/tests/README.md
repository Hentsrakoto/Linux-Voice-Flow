# Fixtures et tests moteur

Les WAV de silence sont générés par les tests avec `wave` : PCM signé 16 bits,
mono, 16 kHz, échantillons nuls. Aucun enregistrement vocal ni donnée personnelle
n'est inclus. Les répertoires sont privés (0700), les WAV 0600, dans le répertoire
temporaire pytest et sont détruits avec celui-ci.

`test_local_model.py` ne s'exécute qu'avec les trois variables explicites :
`LVF_TEST_MODEL_PATH`, `LVF_TEST_FR_WAV`, `LVF_TEST_EN_WAV`. Fournir un modèle
faster-whisper/CTranslate2 complet et deux WAV dans ce format, de 1 à 60 secondes,
avec provenance, licence ou consentement explicite. Les voix restent locales et
ne sont jamais ajoutées au dépôt par le test. Ne pas utiliser de voix d'un tiers
sans autorisation. Le test copie les WAV dans son répertoire privé et bloque les
connexions réseau Python. Une recette réseau coupé au niveau système reste
nécessaire pour observer les éventuelles connexions natives.

Exemple après préparation explicite des fichiers :

```sh
LVF_TEST_MODEL_PATH=/absolute/model LVF_TEST_FR_WAV=/absolute/fr.wav \
LVF_TEST_EN_WAV=/absolute/en.wav .venv/bin/python -m pytest -m local_model
```

Vérifier manuellement le sens des phrases françaises/anglaises et mesurer la
latence (objectif : 10 s d'audio transcrites en moins de 15 s). Le test automatique
n'établit pas la qualité linguistique, seulement un résultat non vide et la langue
attendue. Aucun modèle n'est téléchargé par ce test.
