# Kollega

Plateforme SaaS d'agents IA **gouvernés** pour TPE et PME françaises (5–200 salariés, sans DSI) : l'entreprise s'inscrit, connecte Microsoft 365 ou Google Workspace, choisit un agent prêt à l'emploi dans un catalogue, et l'agent travaille — journal d'audit inaltérable, coût affiché avant et après, validation humaine par seuil, hébergement français, RGPD natif.

Ce n'est pas un chatbot ni un studio générique. La constitution du projet (produit, invariants, architecture, discipline) est dans [`CLAUDE.md`](CLAUDE.md) ; les décisions sont tracées dans [`docs/adr/`](docs/adr/).

> **Statut : pré-produit, en construction. Un développeur, à temps partiel.**
> Ce README suit l'avancement **réel** du code. Ce qu'aucun test exécuté ne prouve est marqué comme tel — voir la [matrice invariant → test](docs/matrice-invariants.md), tenue sans complaisance.

---

## État réel au 28/07/2026

`cargo test --workspace` : **142 tests, 0 échec** en local, et désormais **exécutés en CI sur un PostgreSQL réel** (GitHub Actions, service pgvector/pg16).

**L'invariant 1 (isolation multi-tenant par RLS) est prouvé depuis le 28/07/2026** : CI run 30223145565 verte avec la politique en place, puis run 30223419721 **rouge** sur une branche jetable où la politique `tenant_isolation` avait été volontairement retirée — la preuve que le test sait échouer. La branche de sabotage a été supprimée après lecture du rouge.

### Construit et prouvé par des tests exécutés

| Crate | Contenu réel |
|---|---|
| `kollega-core` | Types du domaine validés à la construction (`Cents(i64)`, plafond de coût, statuts de tâche…) ; assemblage de prompt à trois origines `SystemInstruction` / `UserRequest` / `ExternalContent` rendu non-confondable par les types (invariant 7), corpus hostile de 34 cas ; proptest. |
| `kollega-audit` | Chaîne de hachage **pure** par organisation : encodage canonique injectif (spécifié dans [`docs/encodage-canonique.md`](docs/encodage-canonique.md), proptest ~4000 cas sans collision), hauteur de chaîne incluse dans les octets hachés, ancre de confiance monotone ; [modèle de menace écrit](docs/audit-modele-de-menace.md) qui dit aussi ce que la chaîne ne prouve **pas**. |
| `kollega-policy` | Moteur de décision **pur** : refus par défaut (outil sans règle = refusé), limite dure / seuil souple distingués sur chaque borne, la limite dure gagne toujours. |
| `kollega-runtime` | Noyau crédit + plafond **pur** : refus avant facturation, solde jamais négatif, conservation des débits, débordement `i64` traité, arrêt propre `aborted_cost_ceiling` distinct d'un échec ; machine à états d'agent pure et **reprise-compatible** (rejouer après sérialisation JSON = parcours direct, 6 scénarios). |
| `kollega-api` | Hachage de mots de passe argon2id, vérification avec les paramètres stockés, bornes plancher/plafond ([ADR-0006](docs/adr/0006-verification-des-mots-de-passe.md)). Pas encore de serveur HTTP qui tourne. |
| `kollega-store` | Point de passage unique du contexte d'organisation (`SET LOCAL app.current_org`) + garde textuelle exécutée. |
| Garde-fou global | Test du graphe de dépendances qui **échoue** si `sqlx`, `reqwest` ou `tokio` entre dans `kollega-core` (invariant 11), liste blanche sur toutes les sections de Cargo.toml. |

### Écrit mais jamais exécuté — donc non prouvé

- **Les migrations de retour arrière** (`migrations/*.down.sql`, invariant 13) : écrites, jamais appliquées. La CI actuelle ne les joue pas, et aucun chemin de descente n'existe encore dans l'outillage (`sqlx::migrate!` ne descend pas).
- **La référence Python** de l'encodage canonique (`tools/reference/canonical.py`) : jamais confrontée à l'implémentation Rust — le runner CI a Python, le test différentiel reste à brancher.

### Pas encore construit

- Aucun binaire qui tourne en continu : pas de serveur HTTP servi, pas de connexion base branchée, pas d'appel de modèle, pas de client MCP.
- `kollega-memory`, `kollega-model`, `kollega-tools` : squelettes (9 lignes chacun).
- Connecteurs OAuth, persistance de l'audit et des crédits, interface, facturation Stripe : jalons M2–M6, non commencés.

### Jalons ([détail](docs/jalons.md))

| Jalon | État |
|---|---|
| M0 — Socle multi-tenant (RLS, rôles, CI, image) | **Prouvé le 28/07/2026** : test d'isolation exécuté sur PostgreSQL réel en CI (run 30223145565), sensibilité démontrée (run 30223419721 rouge, politique retirée sur branche jetable), image OCI signée cosign + SBOM publiés. Reste hors preuve : les `.down.sql` (voir ci-dessus). |
| M1 — Identité, audit, politiques | **Surface pure faite en avance** (types, chaîne d'audit, moteur de politiques, argon2id) ; la persistance PostgreSQL reste à brancher. |
| M3 — Runtime et crédits | **Noyau pur fait en avance** (budget, machine à états) ; la concurrence sur le crédit est [spécifiée](docs/credits-concurrence.md), non implémentée. |
| M2, M4–M7 | Non commencés. |

### Invariants — 13, état résumé

**Prouvés** : **1 (isolation RLS — CI du 28/07/2026, sensibilité comprise ; la partie vectorielle attend M5)**, 4 (chaîne d'audit, partie pure), 7 (assemblage, corpus adversarial de 34 cas), 11 (core sans entrée-sortie). **Prouvés en partie** (le niveau pur l'est, l'application réelle dépend d'un jalon futur) : 2, 3, 5, 6. **Sans test aujourd'hui** : 8, 9, 10, 12 (jalons non commencés) — et **13 a des retours arrière écrits mais jamais exécutés**. Détail ligne par ligne : [`docs/matrice-invariants.md`](docs/matrice-invariants.md).

---

## Architecture (décisions tranchées, ne pas rouvrir sans ADR)

Monolithe modulaire Rust (un binaire, des crates aux frontières nettes) ; PostgreSQL 16 + pgvector, seul moteur, isolation par **Row-Level Security** posée dès la migration 0001 (rôle applicatif sans `BYPASSRLS`) ; Python uniquement en serveurs MCP (il transforme, il ne décide jamais) ; modèles via API externe derrière un trait `ModelProvider` ; interface web rendue serveur ; image OCI unique signée (cosign) avec SBOM ; hébergement cloud français. Raisons : [`CLAUDE.md`](CLAUDE.md) §5 et [`docs/adr/`](docs/adr/).

## Développement

```sh
cargo test --workspace              # tests purs : passent sans base
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

Le test d'isolation RLS ne s'active que si `TEST_MIGRATE_DATABASE_URL` pointe vers un PostgreSQL (voir `ci.yml`). Migrations dans [`migrations/`](migrations/), conteneur dans [`deploy/`](deploy/).

---

*Documentation en français ; code, types et identifiants SQL en anglais. Ce README est mis à jour à chaque changement d'état réel du code.*
