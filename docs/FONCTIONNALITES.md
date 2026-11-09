# Fonctionnalités réellement opérationnelles et limitations

Légende : ✅ prouvé par test automatisé · 🔧 écrit, validé seulement en simulation / par compilation · ❌ non réalisé

## Prouvé par les tests automatisés (CI)
- ✅ Chiffrement AES-256-GCM par blocs + Argon2id : aller-retour, mauvais mot de passe, altération, troncature, en-tête modifié, données parasites, paramètres hostiles.
- ✅ Empreintes SHA-256, vérification des fichiers transférés et des sauvegardes (altération et suppression détectées).
- ✅ Transfert : copie atomique, dates conservées, reprise, fichiers partiels, doublons, conflits (ignorer / renommer / écraser avec confirmation), filtre de catégories, plan falsifié avec traversée de répertoire bloqué, source illisible signalée, espace insuffisant refusé avant copie, annulation sans fichier partiel, progression monotone atteignant 100 %.
- ✅ Sauvegardes : création, versions, incrémental, vérification, export/import chiffrés, archive malveillante refusée, suppression sécurisée, identifiants hostiles refusés.
- ✅ Sécurité : `safe_join`, validation des UDID, caviardage des journaux, outils lancés sans shell (injection impossible), annulation/délai des processus.
- ✅ Compatibilité entre appareils (iOS plus ancien, capacité, non approuvé, même appareil).
- ✅ Vérification de paquets de mise à jour (SHA-256 + Ed25519).
- ✅ Historique anonymisé, journaux caviardés, auto-test de sécurité.

## Écrit mais non éprouvé sur matériel réel
- 🔧 Détection d'iPhone/iPad (`idevice_id`, `ideviceinfo`), sauvegarde et restauration (`idevicebackup2`), activation du chiffrement des sauvegardes.
- 🔧 Interface complète : compilée et rendue (captures), parcours exercés avec un pont Tauri simulé, **non testée de bout en bout dans l'application Tauri**.

## Détails de l'iPhone et écran
- 🔧 Détails complets (modèle, iOS, build, couleur, MAC, identifiants masqués par défaut) et **batterie** (niveau, charge) via `ideviceinfo` ; **cycles de charge et capacité maximale estimée** via `idevicediagnostics` si l'appareil l'expose. Analyse des sorties testée sur exemples, **non vérifiée sur un vrai iPhone**.
- ⚠ **Écran réel de l'iPhone** : `idevicescreenshot` exige l'« image développeur » montée sur l'appareil (iOS 16 et moins, mode développeur). **iOS 17 et plus n'est pas pris en charge** par libimobiledevice 1.3 : l'application l'indique au lieu d'afficher un faux écran.

## Limitations restantes
- ⚠ Outils libimobiledevice : embarqués dans l'installateur Windows (démarrage vérifié en CI) ; **non embarqués sous macOS** (`brew install libimobiledevice`).
- ❌ Installateurs non signés ni notarisés.
- ❌ Mise à jour automatique : seule la **vérification** d'un paquet est fournie (pas de serveur de mises à jour ni de téléchargement).
- ❌ Choix par catégorie lors d'un Ghost Clone : la restauration Apple est « tout ou rien » ; le choix par catégorie existe dans Transfert intelligent (dossiers).
- ⚠ Volume par catégorie lu sur l'appareil (`com.apple.disk_usage` : photos, applications, calendrier…) : disponible dès l'approbation, **non vérifié sur un vrai appareil**. Le **nombre** de photos/contacts/messages n'est pas accessible directement : il est fourni après une sauvegarde (Analyse).
- ❌ Retour à une version précédente de l'application, tests d'accessibilité automatisés, tests de performance chiffrés, tests de bout en bout sur appareil.
- ⚠ Le mot de passe de chiffrement de sauvegarde est transmis à `idevicebackup2` en argument de ligne de commande (visible brièvement des autres processus de la machine).
- ⚠ Suppression sécurisée : écrasement puis effacement ; l'effacement physique n'est pas garanti sur SSD/APFS.
