# Protocole JSONL v1 et état

Le contrat de données est [engine-v1.schema.json](../contracts/engine-v1.schema.json), dialecte [JSON Schema 2020-12](https://json-schema.org/draft/2020-12). Les fixtures [messages.json](../contracts/fixtures/messages.json) sont consommées par les tests Rust et Python. Pas de dépendance JSON Schema dans le moteur de production.

Chaque message est un objet UTF-8 sur une ligne terminée par LF. Maximum : 1 048 576 octets, **sans** le LF. Pas de CRLF, BOM, ligne vide, UTF-8 incorrect, clé dupliquée, valeur non finie, champ inconnu ou version inconnue. Les parseurs directs acceptent aussi une ligne sans son LF ; le lecteur de flux exige le terminateur, y compris à EOF. `protocol_version` et `duration_ms` exigent un entier JSON sans fraction ; Python refuse explicitement les booléens. Le schéma exprime les contraintes JSON sémantiques ; les contraintes d'encodage et de représentation numérique sont assurées par les parseurs.

Tous les messages possèdent `protocol_version: 1`, `request_id` (chaîne non vide) et `type`. `startup` est réservé au démarrage : `ready` doit l'utiliser, et `error` peut l'utiliser. Les autres messages exigent un identifiant différent. Aucun identifiant ne doit être réutilisé pendant la durée de vie du superviseur.

| Sens | Type | Champs supplémentaires obligatoires |
|---|---|---|
| Rust → Python | `transcribe` | `audio_path`, `model_path` : chemins Linux absolus sans NUL ; `language` : `fr`, `en`, `auto` ; `vad` : `true` |
| Rust → Python | `cancel` | Aucun ; même identifiant que la transcription |
| Python → Rust | `ready` | Aucun ; modèle chargé |
| Python → Rust | `result` | `text` non vide ; `language` : `fr` ou `en` ; `duration_ms` : entier entre 0 et 9 007 199 254 740 991 |
| Python → Rust | `error` | `code` normalisé ; `message` non vide |
| Python → Rust | `cancelled` | Aucun ; confirme l'annulation |

Python reçoit le modèle local par argument CLI explicite `--model-path /chemin/absolu` avant d'émettre `ready`. Chaque `transcribe.model_path` doit correspondre exactement au chemin chargé : le protocole n'autorise aucun changement implicite de modèle. Les vérifications d'existence, de permissions et de contenu du modèle relèvent du moteur. Rust conserve la propriété et le nettoyage du WAV temporaire. Ces chemins ne sont jamais envoyés à React.

Les erreurs du CDC sont toutes représentées : `MIC_UNAVAILABLE`, `NO_SPEECH`, `MODEL_NOT_FOUND`, `MODEL_LOAD_FAILED`, `ENGINE_BUSY`, `ENGINE_TIMEOUT`, `ENGINE_CRASHED`, `FOCUS_CHANGED`, `INJECTION_FAILED`, `CLIPBOARD_FAILED`, `CANCELLED`, `PROTOCOL_ERROR`. Une erreur de parsing renvoie un message constant sans recopier les octets reçus. L'UI doit convertir le code en message maîtrisé, sans afficher un message technique arbitraire du moteur ni le journaliser.

Les constantes des deux langages sont : démarrage 60 s, transcription 120 s, acquittement d'annulation 3 s. Le superviseur applique les délais, invalide l'identité avant récupération, puis termine/redémarre le moteur si nécessaire. Une seule transcription active ; le moteur refuse les autres avec `ENGINE_BUSY`. Une répétition portant exactement l'identité active ne doit pas provoquer deux résultats.

## Interfaces

Rust : `Request`/`EngineRequest`, `Response`/`EngineResponse`, `ErrorCode`, `Language`, `parse_request`, `parse_response`/`parse_engine_line`, `encode_request`, `encode_response`. Les encodeurs valident avant sérialisation et ajoutent LF. Les enums utilisent [le tag interne Serde](https://serde.rs/enum-representations.html) et [deny_unknown_fields](https://serde.rs/container-attrs.html). La désérialisation passe par [serde_json::from_slice](https://docs.rs/serde_json/latest/serde_json/fn.from_slice.html).

`JsonlDecoder::push` accepte les fragments et lignes concaténées ; son tampon ne dépasse jamais 1 Mio. Au dépassement il émet une erreur puis ignore les octets jusqu'au LF suivant, sans accumuler. `finish` refuse un fragment final. Le superviseur doit lire stdout par blocs de taille bornée, traiter chaque sortie sans journaliser son contenu et arrêter un moteur fautif ; la resynchronisation du décodeur n'autorise pas la poursuite d'un moteur compromis.

Python : `parse_request(bytes)`, `parse_response(bytes)` retournent des dictionnaires validés ; `encode_message(dict)` retourne les octets avec LF ; `read_message(BinaryIO)` lit une ligne bornée, renvoie `None` uniquement à EOF propre ; `validate_model_path(request, loaded_path)` vérifie l'identité du modèle. `ProtocolError` est une `ValueError` avec `code = PROTOCOL_ERROR` et un message constant.

## Machine à états pure

`transition(DictationState, Event) -> Result<(DictationState, Vec<Effect>), AppError>` ne touche aucun périphérique. L'adaptateur exécute les effets une fois, dans l'ordre. Les identités sont des entiers `SessionId` croissants, jamais réutilisés ; rester dans la plage des entiers sûrs JavaScript côté UI.

Parcours nominal : `initializing → ready → recording → transcribing → inserting → success → ready`. Seul un résultat correspondant à la session en transcription émet `Insert` ; le passage immédiat à `inserting` empêche toute insertion doublée. Une annulation en attente d'insertion passe à `cancelled` et nettoie. L'adaptateur doit appeler `can_insert(&state, session_id)` immédiatement avant l'effet externe, sous le même verrou que l'application des transitions ; annuler invalide donc aussi un effet `Insert` encore en file. Le contrôleur conserve le texte en mémoire séparément, vérifie le focus et applique le fallback avant `Inserted`.

Annulation de capture : `cancelled` et nettoyage. Annulation de transcription : `cancelling`, émission de `CancelEngine` et nettoyage ; un résultat tardif ne produit aucun effet. L'acquittement passe à `cancelled` ; le délai d'annulation dépassé demande un redémarrage. Le timeout de transcription passe à `error` et émet nettoyage puis redémarrage. `NO_SPEECH` revient à `ready` avec nettoyage, sans insertion. `Reset` récupère depuis succès, annulation ou erreur. Les événements asynchrones inconnus, tardifs et dupliqués sont ignorés ; une transition de commande interdite renvoie `ENGINE_BUSY`.

Ces modules ne constituent pas encore la capture, le superviseur, l'injection X11 ou une preuve matérielle. Le contrôleur devra exécuter les effets et garantir la génération des identités.

`InitializationFailed` expose une erreur récupérable avec l’identité réservée `0` ; les sessions de dictée exigent un identifiant positif dans la plage sûre JavaScript.

## Extension Groq : fournisseur de transcription

La configuration Tauri ajoute `provider: "local" | "groq"`, avec `local` par défaut pour les anciens appels. Le snapshot expose le fournisseur du moteur préparé. La clé et le nom du modèle distant restent dans le moteur Python, configuré par `.env` ; ils ne sont pas ajoutés aux messages JSONL. Le processus reçoit uniquement `--provider`, l’identité absolue `--model-path`, le répertoire audio et les paramètres CPU existants.

Les champs JSONL v1 restent identiques. Pour le local, `model_path` continue de désigner le répertoire chargé. Pour Groq, il désigne le répertoire canonique du projet comme identité de session, et non un modèle à charger. Rust et Python contrôlent toujours son égalité sur chaque requête. Les résultats restent limités au français et à l’anglais. Les messages `ready` ne prouvent pas qu’une clé distante est valide : préparer Groq ne fait aucun appel API.

Quatre codes contrôlés complètent le protocole : `API_CONFIG_ERROR` (configuration absente ou invalide), `API_AUTH_ERROR` (authentification refusée), `API_RATE_LIMIT` (quota/limite) et `API_UNAVAILABLE` (transport, service ou réponse incorrecte). Les délais réseau utilisent `ENGINE_TIMEOUT`. Les détails HTTP, corps d’erreur et secrets ne sont jamais recopiés dans le protocole.

Le VAD local filtre les silences avant un envoi Groq. Le transport utilise un endpoint HTTPS fixe, sans redirection ni nouvelle tentative automatique, un délai réseau de 45 secondes et une réponse bornée à 1 Mio. Le superviseur Rust conserve son délai global de 120 secondes et son délai d’annulation de 3 secondes avant arrêt forcé. Annuler invalide le résultat côté application ; cela ne rappelle pas l’audio déjà reçu par le fournisseur.
