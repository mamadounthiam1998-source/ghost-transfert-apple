# Guide d'installation

## Obtenir les installateurs
Les installateurs sont générés par le workflow GitHub **Tests et installateurs** (onglet *Actions* → dernière exécution → *Artifacts*) :
`.exe` (NSIS) et `.msi` pour Windows, `.dmg` pour macOS (Apple Silicon et Intel).

> Les installateurs ne sont **pas signés** (aucun certificat n'est configuré). Windows SmartScreen et macOS Gatekeeper afficheront un avertissement.
> Pour signer/notariser : ajoutez vos certificats aux secrets GitHub et complétez le workflow.

## Dépendances pour appareils réels
**Windows** : installer « Appareils Apple » (Microsoft Store) ou iTunes (pilotes USB Apple). Les outils libimobiledevice (`idevice_id`, `ideviceinfo`, `idevicebackup2`, `idevicepair`) sont **embarqués dans l'installateur** par le workflow GitHub (paquet MSYS2) et leur démarrage est vérifié à chaque build. Ils sont distribués sous leurs licences LGPL/GPL (libimobiledevice).
**macOS** : `brew install libimobiledevice`.
Le **Diagnostic** de l'application indique ce qui manque.

## Désinstallation
Windows : Paramètres → Applications. macOS : glisser l'application dans la corbeille. Les sauvegardes dans le dossier de l'application ne sont pas supprimées automatiquement.
