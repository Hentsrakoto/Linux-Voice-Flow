# Revue de la sélection des modèles

Revue indépendante des sources par un agent : catalogue Rust, commande et permissions Tauri, identité du modèle dans les snapshots, sélection React et installateur Python. Aucun défaut bloquant concret relevé. Les cinq tests de l’installateur passent.

La validation du catalogue est structurelle ; elle ne recalcule pas les empreintes à chaque affichage. L’installateur contrôle les empreintes et le moteur vérifie le chargement réel. Les contrôles sur silence ne mesurent pas la précision en français ou en anglais.

À la reprise, 43 tests unitaires Rust et 12 tests d’intégration ont été réexécutés avec succès depuis les binaires compilés après les modifications Rust. Le compilateur temporaire ayant disparu, ces exécutions ne constituent pas une nouvelle compilation. Le rapport backend conserve les résultats de compilation de l’implémentation initiale. Aucun fichier Rust n’a été modifié pendant cette reprise.

Le premier contrôle du vrai moteur a révélé une incompatibilité de dépendances que les simulations ne détectaient pas : PyAV 19 supprime `metadata_errors`, utilisé par faster-whisper 1.2.1. Un test de décodage PCM sans poids reproduit le défaut ; PyAV est verrouillé en 18.1.0 avec une contrainte `<19` dans les entrées. Voir [l’incident amont](https://github.com/SYSTRAN/faster-whisper/issues/1589).

Le chargement réel a également montré l’initialisation de la télémétrie ONNX Runtime. `ORT_DISABLE_TELEMETRY=1` est désormais défini avant l’import natif, suivant [la documentation officielle](https://github.com/microsoft/onnxruntime/blob/main/docs/Privacy.md), avec une régression unitaire rouge puis verte. La contre-revue confirme le placement en production ; les tests qui importent directement le runtime appliquent également cette variable avant l’import. Les connexions Python bloquées pendant un test ne constituent pas à elles seules une preuve d’isolation réseau de toutes les bibliothèques natives.
