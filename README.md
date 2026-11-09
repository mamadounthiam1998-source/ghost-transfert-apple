# GHOST TRANSFERT APPLE

Logiciel de bureau (Windows et macOS) pour **migrer, cloner logiquement et sauvegarder** des appareils Apple,
construit avec **Tauri 2 + Rust + React/TypeScript**. Tout fonctionne en local : aucune donnée n'est envoyée sur internet.

> Votre univers Apple. Transféré en toute sécurité.

## État réel du projet (lisez ceci d'abord)

| Élément | État |
|---|---|
| Cœur Rust (crypto, sécurité, transfert, sauvegarde, historique, diagnostic) | **63 tests automatisés** exécutés dans la CI |
| Interface (11 écrans, thème sombre/clair, animations) | Compilée et testée, rendu vérifié par captures d'écran |
| Mode simulation (appareils fictifs, explicite) | Opérationnel, sert aux tests sans appareil |
| Communication avec de **vrais** iPhone/iPad | **Écrite mais non testée sur du matériel réel** (voir `docs/COMPATIBILITE_APPLE.md`) |
| Installateurs `.exe` / `.msi` / `.dmg` | Produits par GitHub Actions (artefacts du workflow) |
| Outils libimobiledevice embarqués | **Non fournis** : à installer ou déposer dans `tools/` (voir `docs/INSTALLATION.md`) |

Rien n'est annoncé comme fonctionnel sans test ou preuve technique : voir `docs/FONCTIONNALITES.md`.

## Démarrage rapide

```bash
npm ci
npm run tauri dev        # application en développement
cargo test -p ghost-core # tests du cœur
npm test                 # tests de l'interface
npm run tauri build      # installateurs pour la plateforme courante
```

## Documentation

- `docs/ARCHITECTURE.md` — architecture, choix techniques justifiés
- `docs/COMPATIBILITE_APPLE.md` — ce qu'Apple permet et ne permet pas
- `docs/INSTALLATION.md` — installation, dépendances, installateurs
- `docs/UTILISATION.md` — guide d'utilisation
- `docs/FONCTIONNALITES.md` — fonctionnalités opérationnelles et limitations restantes
