# Architecture

## Choix technologiques (justifiés)

| Choix | Raison |
|---|---|
| **Tauri 2** (plutôt qu'Electron) | Binaire léger, WebView native, modèle de permissions strict, sécurité par défaut (CSP, commandes explicites). |
| **Rust** pour tout le moteur | Sûreté mémoire pour manipuler des fichiers et des archives non fiables, bonnes performances d'E/S, bibliothèques cryptographiques mûres (RustCrypto). |
| **React + TypeScript** | Interface déclarative, typage des échanges avec le moteur. |
| **Framer Motion** | Transitions de pages, anneaux de progression et micro-interactions fluides ; respect de `prefers-reduced-motion`. |
| **libimobiledevice** (`idevice_id`, `ideviceinfo`, `idevicebackup2`) | Seule implémentation ouverte et maintenue du protocole de sauvegarde/restauration Apple, multiplateforme. |
| **AES-256-GCM + Argon2id + SHA-256** | Standards actuels : chiffrement authentifié, dérivation résistante au GPU, empreintes d'intégrité. |

## Séparation des responsabilités

```
crates/ghost-core/            Aucune dépendance à l'interface
  tools.rs        lancement des outils externes (sans shell, délais, annulation)
  devices.rs      détection, compatibilité, backends : réel (libimobiledevice) et simulation
  analysis.rs     classification et statistiques par catégorie
  transfer.rs     copie atomique, SHA-256, reprise, doublons, conflits, espace disque
  backup.rs       sauvegardes, manifestes, versions, vérification, export/import chiffrés
  crypto.rs       AES-256-GCM par blocs + Argon2id
  security.rs     validation des chemins/identifiants, caviardage, suppression sécurisée
  progress.rs     progression réelle (octets écrits), annulation
  history.rs      historique local   logging.rs  journaux anonymisés
  diagnostics.rs  état des composants + auto-test de sécurité
  updates.rs      vérification SHA-256 + signature Ed25519 des paquets
src-tauri/        Fine couche de commandes Tauri : valide, délègue, journalise (aucune logique critique)
src/              Interface React (pages, composants, API typée)
```

Aucune logique de transfert ne dépend du frontend : l'interface n'appelle que des commandes qui délèguent à `ghost-core`.

## Flux « Ghost Clone »

1. Contrôle de compatibilité (appareils connectés ET approuvés, iOS destination ≥ source, capacité, même appareil refusé).
2. Confirmation explicite : la destination sera remplacée.
3. Sauvegarde officielle de la source (`idevicebackup2 backup`), manifeste SHA-256.
4. Vérification de la sauvegarde ; en cas d'écart, **la destination n'est pas touchée**.
5. Restauration officielle sur la destination (`idevicebackup2 restore -s <source>`).
6. Rapport + historique. La relecture du contenu de la destination n'est pas possible pour un logiciel tiers (limite Apple), ce qui est dit dans le rapport.

## Sécurité

Voir l'onglet Sécurité du logiciel. Points clés : chemins validés (`safe_join`), identifiants validés avant tout appel d'outil, aucun shell, archives importées validées avant toute écriture, aucun lien symbolique suivi, journaux caviardés, CSP Tauri sans connexion externe, permissions Tauri limitées aux boîtes de dialogue.
