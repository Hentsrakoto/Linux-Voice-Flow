# Voice Flow — identité

Direction choisie : sobre, verte, arrondie ; un microphone accompagné d’une onde de voix. Le nom et les couleurs existantes sont conservés.

## Intention

Mission : permettre aux utilisateurs Linux de dicter dans leur application avec un raccourci explicite. Vision : rendre la saisie vocale aussi directe qu’une saisie clavier.

Valeurs et comportements :

- Contrôle : maintenir pour dicter ; relâcher pour transcrire ; permettre l’annulation.
- Clarté : distinguer raccourci reçu, capture et traitement ; annoncer une insertion incertaine.
- Confidentialité : expliquer le choix local/Groq ; garder les secrets hors interface et journaux.
- Sobriété : réserver les réglages à leur vue ; embarquer les icônes et polices système.

Personnalité : calme (mouvements lents), précise (états réels), accessible (libellés français), discrète (surfaces plates), pratique (erreurs accompagnées d’une action).

## Logo et fichiers

Le symbole réunit un micro arrondi et trois barres de voix. Le nom Voice Flow reste lisible à côté dans l’en-tête.

- Symbole transparent : `public/brand/voice-flow-mark.svg`.
- Icône sur fond clair : `public/brand/voice-flow-icon.svg` ; utiliser également cette version sur fond sombre.
- Icône native : `src-tauri/icons/icon.svg` et `icon.png` (512 px).
- Sources et export SVG Lab : `assets/brand/`.
- [Projet SVG Lab éditable](https://svglab.app/?project=37cb637a-70a1-4040-8b37-23807f86d6a3).

Taille minimale conseillée : symbole 24 px, impression 8 mm. Exception : favicon 16 px, sans texte. En communication, réserver autour du logo isolé un espace égal à sa hauteur ; l’en-tête compact utilise l’espacement du composant existant. Ne pas étirer, pivoter, changer les proportions des barres, ajouter ombre ou dégradé. Garder le fond clair de l’icône sur les supports sombres ou chargés.

Le SVG est dessiné à la main depuis une référence générée, sans vectorisation automatique. Différences assumées : vert uni de la marque, fond et fentes transparents, absence de grain raster et courbes régularisées. Les références ne font pas partie des ressources livrées.

## Couleurs et typographie

| Rôle | Couleur | Usage |
|---|---|---|
| Primaire / accent | `#176a57` | Logo, focus, capture active |
| Secondaire / texte | `#203932` | Titres et texte principal |
| Texte atténué | `#52665e` | Aides et métadonnées |
| Papier | `#f5f6f2` | Fond général et icône |
| Surface | `#ffffff` | Panneaux |
| Ligne | `#d5dfd8` | Bordures décoratives |

Les états utilisent aussi un libellé et une forme : vert seul ne signifie jamais succès ou disponibilité. Erreurs et avertissements restent des messages explicites, sans ajouter une palette décorative. Les jetons canoniques sont dans `src/styles.css`.

Sans système (Segoe UI/Noto Sans) pour le texte ; DejaVu Sans Mono pour les touches et métadonnées. Titres 28–38 px, texte 13–16 px, métadonnées au moins 12 px. Pas de police distante.

## Voix et langage visuel

Français direct, respectueux, concret et calme. Instructions brèves ; erreurs avec prochaine action. Dire « Micro actif » seulement pendant la capture ; « Traitement… » après celle-ci. Préférer « Ouvrir les réglages » à « Une erreur inconnue est survenue ». Ne pas promettre une confidentialité locale lorsque Groq est actif.

Icônes Lucide de contour embarquées ; pas d’émojis comme contrôles. Captures réelles pour documenter le produit ; illustrations simples, sans photographie promotionnelle dans l’utilitaire. Surfaces plates et arrondies, sans ombre.

L’impulsion confirme le raccourci. Les halos du micro suivent uniquement la phase native de capture, pas le volume audio. Le traitement remplace les halos par un indicateur distinct. Avec réduction des mouvements, conserver le texte et la couleur sans animation. Aucun démarrage fictif en aperçu navigateur.

## Applications et maintien

Numérique : symbole dans l’en-tête, version sur fond clair pour favicon et lanceur. Impression : utiliser le SVG, fond clair uni, contrôler la lisibilité sur épreuve ; aucune référence CMJN/Pantone n’est définie. Signalétique : agrandir proportionnellement le SVG et conserver son espace libre.

Toute évolution passe par les sources du logo, les jetons partagés et une vérification aux petites tailles, dans les deux vues et avec réduction des mouvements. Ce guide complète `DESIGN.md` et `UX-CONTRACT.md`.
