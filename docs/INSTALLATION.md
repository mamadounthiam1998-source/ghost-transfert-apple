# Guide d'installation

## Obtenir les installateurs
Les installateurs sont générés par le workflow GitHub **Tests et installateurs** (onglet *Actions* → dernière exécution → *Artifacts*) :
`.exe` (NSIS) et `.msi` pour Windows, `.dmg` pour macOS (Apple Silicon et Intel).

> Les installateurs ne sont **pas signés** (aucun certificat n'est configuré). Windows SmartScreen et macOS Gatekeeper afficheront un avertissement.
> Pour signer/notariser : ajoutez vos certificats aux secrets GitHub et complétez le workflow.

## Dépendances pour appareils réels
**Windows** : installer « Appareils Apple » (Microsoft Store) ; placer `idevice_id.exe`, `ideviceinfo.exe`, `idevicebackup2.exe` et leurs DLL dans le dossier `tools` à côté de l'application (ou dans le PATH).
**macOS** : `brew install libimobiledevice`.
Le **Diagnostic** de l'application indique ce qui manque.

## Désinstallation
Windows : Paramètres → Applications. macOS : glisser l'application dans la corbeille. Les sauvegardes dans le dossier de l'application ne sont pas supprimées automatiquement.
