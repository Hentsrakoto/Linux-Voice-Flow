# Notice Échap affichée deux fois — 4 novembre 2026

Le backend natif émet déjà une notice complète avec l’instruction d’utiliser le bouton Annuler. Le panneau React ajoutait systématiquement une deuxième instruction équivalente. Le panneau affiche désormais la notice native sans suffixe ; son message de secours en cas d’échec d’abonnement conserve une instruction autonome.

Fichiers modifiés : `src/features/dictation/DictationPanel.tsx` et son test. Le test existant utilise maintenant le message réellement émis par Rust, vérifie son affichage sans ajout et conserve les vérifications du bouton Annuler actif et de la libération des deux abonnements. Échec rouge observé sur le doublon, puis suite complète du panneau verte : **20 tests réussis**. Lint réussi sans avertissement.

Ce correctif concerne uniquement le doublon d’affichage. Aucun changement à l’inscription native d’Échap n’est effectué, et sa disponibilité sous le bureau de l’utilisateur reste à vérifier. Le bouton Annuler conserve son fonctionnement existant. Aucun fichier `.env` privé lu, aucune requête Groq ni capture microphone réalisée.
