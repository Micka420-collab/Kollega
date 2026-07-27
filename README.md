# Kollega

Plateforme SaaS d'agents IA **gouvernés** pour TPE et PME françaises (5–200 salariés, sans DSI) : l'entreprise s'inscrit, connecte Microsoft 365 ou Google Workspace, choisit un agent prêt à l'emploi dans un catalogue, et l'agent travaille — journal d'audit inaltérable, coût affiché avant et après, validation humaine par seuil. L'hébergement sur cloud français et la conformité RGPD de sous-traitant sont des **décisions d'architecture prises** ([ADR-0001](docs/adr/0001-pivot-plateforme-multi-tenant.md)), **non réalisées** : rien n'est hébergé nulle part aujourd'hui, et aucune obligation de sous-traitant n'est en place — elles arrivent avec M7.

Ce n'est pas un chatbot ni un studio générique. La constitution du projet (produit, invariants, architecture, discipline) est dans [`CLAUDE.md`](CLAUDE.md) ; les décisions sont tracées dans [`docs/adr/`](docs/adr/).

> **Statut : pré-produit, en construction. Un développeur, à temps partiel.**
> Ce README suit l'avancement **réel** du code. Ce qu'aucun test exécuté ne prouve est marqué comme tel — voir la [matrice invariant → test](docs/matrice-invariants.md), tenue sans complaisance.

---

## État réel au 28/07/2026

`cargo test --workspace` : **155 tests, 0 échec** en local, et **exécutés en CI sur un PostgreSQL réel** (GitHub Actions, service pgvector/pg16).

Le journal ne se contente pas d'être intact : **sa séquence est vérifiée sur les données réelles** — pas de clôture sans intention, pas d'intention en double, rien après une clôture. Un appel resté ouvert (validation en attente, redémarrage) est signalé comme *information*, jamais comme corruption : sans cette asymétrie, une panne banale ressemblerait à une falsification.

**La tranche verticale traverse** (CI n°17 et suivantes) : une tâche est créée, soumise à la politique, exécutée, débitée du crédit, journalisée dans PostgreSQL (attestations chaînées et contenus séparés), **interrompue, reprise depuis la base**, et terminée avec le même résultat qu'un parcours direct. **Les effets d'outils sont idempotents** : un pas rejoué après une panne ne renvoie pas un second mail — l'effet déjà réalisé est reconnu par une identité dérivée de `(task_id, iteration)`, et prouvé par un test qui reconstitue le scénario (effet accompli, état revenu en arrière). Les deux pannes silencieuses inverses sont testées aussi : l'identité ne peut être partagée ni entre deux tâches, ni entre deux itérations, et les effets d'une tâche ne fuient pas vers une autre — sans quoi un appel jamais exécuté passerait pour déjà fait, et le mail ne partirait **jamais**.

**L'invariant 1 (isolation multi-tenant par RLS) est prouvé depuis le 28/07/2026** : CI run 30223145565 verte avec la politique en place, puis run 30223419721 **rouge** sur une branche jetable où la politique `tenant_isolation` avait été volontairement retirée — la preuve que le test sait échouer. La branche de sabotage a été supprimée après lecture du rouge.

### Construit et prouvé par des tests exécutés

| Crate | Contenu réel |
|---|---|
| `kollega-core` | Types du domaine validés à la construction (`Cents(i64)`, plafond de coût, statuts de tâche…) ; assemblage de prompt à trois origines `SystemInstruction` / `UserRequest` / `ExternalContent` rendu non-confondable par les types (invariant 7) — **confinement pur : le contenu externe est transporté verbatim**, corpus hostile de 34 cas + proptest ([modèle de menace v2](docs/invariant-7-modele-de-menace.md)). |
| `kollega-audit` | Chaîne de hachage **pure** par organisation : encodage canonique injectif (round-trip par décodeur indépendant), hauteur incluse dans les octets hachés, ancre de confiance monotone ; **spécification confirmée par différentiel Rust ↔ Python en CI** (14 014 vecteurs, zéro divergence). Deux types distincts portent la règle : une entrée **produite** par le domaine ne peut pas mentir (champs privés, empreinte calculée — forger ne compile pas), une entrée **relue du stockage** le peut, et c'est nécessaire : rendre la corruption inreprésentable la rendrait indétectable. [Modèle de menace](docs/audit-modele-de-menace.md) qui dit aussi ce que la chaîne ne prouve **pas**. |
| `kollega-policy` | Moteur de décision **pur** : refus par défaut (outil sans règle = refusé), **bornes à deux étages** — seuil de validation puis limite dure au-dessus, que nulle validation ne lève (le « souple sans plafond » n'est plus représentable) ; fail-open du préfixe vide fermé. **La boucle l'appelle directement**, avec l'appel complet : les bornes ne sont plus inertes. |
| `kollega-runtime` | Noyau crédit + plafond **pur** : refus avant facturation, solde jamais négatif, arrêt propre `aborted_cost_ceiling` distinct d'un échec ; machine à états **reprise-compatible** persistée via une **enveloppe versionnée** (v2 — une enveloppe d'une autre version est refusée net, jamais mal relue) ; chaque appel d'outil porte son itération, ce qui rend son identité **dérivable** — prérequis de l'idempotence. |
| `kollega-api` | Hachage argon2id, vérification aux paramètres stockés bornés (plancher 8 Mio, **plafond 64 Mio**) + **sémaphore de concurrence** (au-delà de la borne, les vérifications attendent, elles n'échouent pas) — [ADR-0006 amendée](docs/adr/0006-verification-des-mots-de-passe.md). Pas encore de serveur HTTP qui tourne. |
| `kollega-store` | Point de passage unique du contexte d'organisation (`SET LOCAL app.current_org`) + garde textuelle exécutée. Le pilote de la tranche parle à la chaîne d'audit **uniquement par des dépôts** dont la surface n'a que `append`/`read` : retirer une preuve n'y est pas exprimable, et deux gardes textuelles (forme du trait, SQL de la persistance) échouent si ça change. |
| Garde-fou global | Test du graphe de dépendances qui **échoue** si `sqlx`, `reqwest` ou `tokio` entre dans `kollega-core` (invariant 11), liste blanche sur toutes les sections de Cargo.toml. |

### Prouvé aussi, depuis le 28/07/2026

- **La réversibilité des migrations** (invariant 13) : le job CI `reversibilite` joue up → down → diff avec l'état vierge → re-up → diff (schéma, rôles, extensions, ACL effective) sur un cluster dédié — **vert à la run n°15**, après cinq rouges instructifs (réglage de la comparaison d'ACL, puis jeton aléatoire `\restrict` de pg_dump ≥ 18). Dossier de preuve archivé sur la branche `ci-diagnostic`. Réserve : prouvé via psql — l'outillage applicatif (`sqlx::migrate!`) n'a pas encore de chemin de descente.

### Pas encore construit

- Aucun binaire qui tourne en continu : pas de serveur HTTP servi, pas de connexion base branchée, pas d'appel de modèle, pas de client MCP.
- `kollega-memory`, `kollega-model`, `kollega-tools` : squelettes (9 lignes chacun).
- Connecteurs OAuth, persistance de l'audit et des crédits, interface, facturation Stripe : jalons M2–M6, non commencés.

### Jalons ([détail](docs/jalons.md))

| Jalon | État |
|---|---|
| M0 — Socle multi-tenant (RLS, rôles, CI, image) | **Prouvé le 28/07/2026, intégralement** : isolation RLS sur PostgreSQL réel (run 30223145565), sensibilité démontrée (run 30223419721 rouge, politique retirée), réversibilité des migrations jouée et vérifiée (run n°15), image OCI signée cosign + SBOM publiés. |
| M1 — Identité, audit, politiques | **Surface pure faite en avance** (types, chaîne d'audit, moteur de politiques, argon2id) ; la persistance PostgreSQL reste à brancher. |
| M3 — Runtime et crédits | **Noyau pur fait en avance** (budget, machine à états) ; la concurrence sur le crédit est [spécifiée](docs/credits-concurrence.md), non implémentée. |
| M2, M4–M7 | Non commencés. |

### Invariants — 13, état résumé

**Prouvés** : **1** (isolation RLS, sensibilité comprise ; le vectoriel attend M5), **2** (tout appel passe par le vrai moteur, requête complète — une limite dure arrête l'appel depuis la boucle), **3** (deux entrées par appel — validées sur la chaîne *persistée*), **4** (chaîne d'audit : ajout seul éprouvé par le rôle en CI, double attestation rendue impossible, spec confirmée par différentiel indépendant), **7** (confinement, corpus 34 cas + proptest), **11** (core sans entrée-sortie), **13** (réversibilité des migrations). **6** : le plafond arrête la tâche dans la boucle réelle, rien n'est facturé, et le statut persisté vaut `aborted_cost_ceiling` — le dirigeant lit que la tâche s'est arrêtée à *sa* borne, pas qu'elle a planté. **Prouvé en partie** : **5** — le débit est atomique et prouvé sous concurrence réelle (deux tâches parallèles ne mettent jamais le solde à découvert), mais la vérification *avant* l'appel de modèle n'est pas tenue : le coût n'est connu qu'après. Décision ouverte, consignée dans [`docs/questions-nuit.md`](docs/questions-nuit.md). **Sans test aujourd'hui** : 8, 9, 10, 12 (jalons non commencés). Détail et réserves : [`docs/matrice-invariants.md`](docs/matrice-invariants.md).

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
