# Revue ciblée du contrôleur T-04 à T-06

Revue en lecture seule de `src-tauri/src/dictation.rs`, centrée sur capture, identité des réponses, annulation, nettoyage et rétention. Aucun changement du contrôleur ni nouvelle exécution des tests déjà validés pendant cette revue. L'insertion inter-applications T-07 est hors périmètre.

## Constats à corriger

### P1 — Une panne moteur pendant l'enregistrement laisse le microphone actif

Dans `Actor::engine_event`, les branches `EngineEvent::Error` non lié à une autre requête et `EngineEvent::Stopped | None` appellent `invalidate()` puis arrêtent le moteur. `invalidate()` efface l'identité et `recording_since`, mais n'appelle pas `audio.discard()`.

Si Python se termine pendant `Phase::Recording`, l'interface passe donc en erreur et la surveillance périodique de capture est désactivée, tandis que le flux microphone reste ouvert. Le relâchement est ensuite ignoré car la phase n'est plus `Recording`. Le garde-fou natif de 60 secondes finit par arrêter le microphone, mais ne remplace pas l'arrêt immédiat attendu après panne.

Correction minimale : appeler `audio.discard()` avant invalidation sur les chemins de panne moteur ; ou faire de `invalidate()` une invalidation complète incluant l'abandon audio, après vérification de tous ses appelants. Régression synthétique conseillée : audio simulé actif + événement moteur `Stopped`, puis erreur globale, vérifier `discard` et absence de capture active.

### P1 — L'annulation urgente ne neutralise pas un appui déjà en file

`DictationService::hotkey` positionne `urgent_cancel` lorsque `try_send` échoue, ce qui protège l'enregistrement déjà actif. Le booléen ne marque toutefois pas les événements déjà en file. `Actor::run` le consomme avant de vider cette file ; `cancel()` ne fait rien si l'acteur est encore `Ready`.

Scénario : l'acteur est prêt mais momentanément bloqué dans une énumération de microphones ; la file contient un `Pressed` puis assez de commandes pour la saturer ; le `Released` ne peut pas être ajouté et déclenche l'annulation urgente. À la reprise, cette annulation ne change rien en phase `Ready`, puis l'ancien `Pressed` démarre la capture alors que la touche est déjà relâchée. Aucun relâchement ne reste en file pour l'arrêter. La limite native borne la durée, mais un enregistrement au repos devient possible.

Correction proposée : associer aux commandes de raccourci une génération d'annulation partagée. Incrémenter cette génération sur perte d'événement et annulation explicite ; ignorer à la consommation les événements estampillés avant la génération courante. Conserver le drapeau urgent pour interrompre la session déjà active. Régression synthétique conseillée : acteur prêt, appui en file puis relâchement perdu par saturation, vérifier qu'aucun appel `audio.start()` n'est effectué après traitement de l'annulation.

## Points conformes observés

- Le démarrage exige un moteur prêt et une phase autorisée ; les appuis répétés pendant le traitement sont refusés.
- Le relâchement passe immédiatement en phase de transcription avant l'arrêt/conversion audio ; le flux natif est détruit avant rééchantillonnage et écriture WAV.
- Les erreurs audio et la limite appellent l'abandon avant invalidation ; aucune requête de transcription n'est construite sur ces chemins.
- Une réponse n'est acceptée qu'en phase `Transcribing` avec l'identifiant de session exact. Les réponses anciennes et dupliquées ne remplacent pas le résultat ; leur texte est effacé au mieux avec zeroize.
- L'annulation invalide l'identité avant d'attendre le moteur, conserve une identité distincte pour son acquittement et ne reconnaît pas les acquittements étrangers.
- Les WAV sont portés par un handle RAII et retirés après résultat, annulation, erreur et arrêt du service. Le résultat courant expire après 120 secondes et l'effacement manuel retire également le texte.
- La file de contrôle est bornée à 32 éléments. Le buffer natif et le WAV sont bornés indépendamment du délai du contrôleur.

Cette revue ne constitue pas une validation matérielle : aucun microphone n'a été ouvert pour la revue.
