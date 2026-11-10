# Recette X11 synthétique de T-07

Le test opt-in `src-tauri/tests/x11_output_smoke.rs` utilise un champ GTK et une zone de texte WebKit, sans microphone, modèle local ni appel Groq. Il injecte uniquement une phrase de test Unicode. Cette recette ne remplace pas une dictée réelle sur Cinnamon/X11.

Prérequis : Rust, `xdotool`, `xclip`, `Xvfb`, Python avec GTK3/GdkX11 et WebKit2 4.1 (`python3-gi`, `gir1.2-gtk-3.0`, `gir1.2-webkit2-4.1` sur Mint/Ubuntu). Les dépendances de compilation audio sont nécessaires si le feature `native-audio` est activé.

Dans un premier terminal, vérifier que le numéro :198 est libre, puis lancer le serveur isolé :

```sh
Xvfb :198 -screen 0 1024x768x24 -nolisten tcp -ac
```

Dans un second terminal, depuis la racine du projet :

```sh
LVF_ISOLATED_X11=1 DISPLAY=:198 XDG_SESSION_TYPE=x11 \
  cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked \
  --test x11_output_smoke -- --ignored --test-threads=1
```

La fixture refuse de fonctionner sur un autre affichage. Elle fixe explicitement le focus dans ses propres fenêtres. Le test compare exactement le texte obtenu dans les deux champs. Arrêter le serveur Xvfb de ce test après la recette ; ne jamais viser le serveur graphique de travail.

Les cas changement de focus, cible détruite, annulation, absence de consentement et nettoyage du propriétaire du presse-papiers sont couverts par les tests unitaires Rust utilisant des outils simulés. La validation utilisateur reste à faire dans un éditeur et un navigateur réels, avec parole consentie et vérification du fallback, selon R-01/R-02/R-05/R-06 du cahier des charges.
