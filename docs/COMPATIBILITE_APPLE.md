# Rapport de compatibilité Apple

## Ce que les interfaces publiques permettent
- Détecter un appareil connecté en USB, lire son nom, modèle, version, stockage (après appairage « Se fier »).
- Créer une sauvegarde locale (chiffrée ou non) et la restaurer sur un appareil (même modèle de protocole que Finder / Appareils Apple).
- Restaurer sur un appareil **différent** (nouvel iPhone) : iOS de la destination doit être **au moins aussi récent** que celui de la sauvegarde.

## Ce qu'Apple n'autorise pas (non contourné, non promis)
- Cloner bit à bit un iPhone.
- Écrire librement dans un iPhone depuis un PC (hors restauration).
- Transférer : Face ID / Touch ID, cartes Apple Pay, eSIM, les applications elles-mêmes (à retélécharger), photos présentes uniquement dans iCloud, certains éléments sous protection iOS.
- Lire le contenu de l'appareil de destination après restauration pour le vérifier : la vérification SHA-256 porte donc sur la sauvegarde source.
- Android → iPhone.

## iPhone ↔ iPad
Possible via sauvegarde/restauration mais certaines données propres à un type d'appareil ne seront pas restaurées : un avertissement est affiché.

## Plateformes
- **Windows 10/11** : exige les pilotes Apple (application « Appareils Apple » du Microsoft Store, ou iTunes) + binaires libimobiledevice.
- **macOS 11+** (Apple Silicon et Intel) : pilotes intégrés ; installer libimobiledevice (`brew install libimobiledevice`).

## Statut de test
Aucun test n'a été réalisé sur du matériel Apple réel avec cette version. La logique de dialogue avec `idevicebackup2` (arguments, lecture de la progression, codes de retour) est écrite d'après la documentation de l'outil et doit être validée sur vos appareils ; ne la considérez pas comme éprouvée.
