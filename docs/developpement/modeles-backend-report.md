# Catalogue local des modèles — backend

Le catalogue embarqué lit `config/models.json` à la compilation. La commande Tauri `list_local_models` retourne exclusivement les installations Tiny et Base complètes dans `models/<id>`, avec identifiant, libellé, description, chemin absolu, taille des quatre fichiers requis et indicateur de modèle par défaut. Le répertoire projet est partagé avec l’initialisation native via `CARGO_MANIFEST_DIR`, sans dépendance au dossier courant.

La découverte ne crée rien et n’utilise aucun accès réseau. Un répertoire absent donne une liste vide. Un catalogue invalide, une racine invalide ou inaccessible donnent une erreur contrôlée ; aucun détail d’erreur système n’est transmis à l’interface. Les liens symboliques dans la racine, ses parents, les répertoires de modèles et les quatre fichiers requis sont refusés. Les dossiers incomplets et les identifiants inconnus ne sont pas proposés. La taille additionne uniquement `model.bin`, `config.json`, `tokenizer.json`, `vocabulary.txt`, avec contrôle de dépassement.

Cette vérification est structurelle : elle ne revalide pas les empreintes des poids à chaque affichage et ne garantit pas qu’un fichier non vide est un modèle valide. L’installateur vérifie les empreintes ; le chargement du moteur reste la validation opérationnelle. Les reçus d’installation ne sont pas obligatoires pour conserver la compatibilité des chemins personnalisés.

`DictationSnapshot.model_id` est nul au démarrage ou pour un chemin personnalisé, et vaut `tiny` ou `base` pour une sélection installée reconnue. Il est défini dès le chargement et conservé dans l’état prêt. Aucun chargement automatique ni préférence persistée n’est ajouté. L’interdiction existante de changer de modèle pendant chargement, capture, transcription et annulation demeure.

Validation : suite Rust hors ligne sans fonctionnalités natives, 42 tests unitaires et 12 tests d’intégration réussis ; puis nouveau test ciblé Tiny → Base réussi, avec faux moteur Python, contrôle de l’identité pendant chargement et après préparation. Tests catalogue : dossiers complets/incomplets, racine absente, liste blanche, taille, liens symboliques et identifiants de traversée. Aucun téléchargement ni vrai modèle nécessaire pour ces tests.

La compilation native `cargo check --offline --locked` a également réussi avec le sysroot ALSA fourni ; les permissions de commande Tauri et le gestionnaire d’invocation sont donc vérifiés à la compilation.
