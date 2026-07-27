# Kollega

Plateforme SaaS d'agents IA **gouvernés** pour TPE et PME françaises (5–200 salariés, sans DSI) : l'entreprise s'inscrit, connecte Microsoft 365 ou Google Workspace, choisit un agent prêt à l'emploi dans un catalogue, et l'agent travaille — journal d'audit inaltérable, coût affiché avant et après, validation humaine par seuil. L'hébergement sur cloud français et la conformité RGPD de sous-traitant sont des **décisions d'architecture prises** ([ADR-0001](docs/adr/0001-pivot-plateforme-multi-tenant.md)), **non réalisées** : rien n'est hébergé nulle part aujourd'hui, et aucune obligation de sous-traitant n'est en place — elles arrivent avec M7.

Ce n'est pas un chatbot ni un studio générique. La constitution du projet (produit, invariants, architecture, discipline) est dans [`CLAUDE.md`](CLAUDE.md) ; les décisions sont tracées dans [`docs/adr/`](docs/adr/).

> **Statut : pré-produit, en construction. Un développeur, à temps partiel.**
> Ce README suit l'avancement **réel** du code. Ce qu'aucun test exécuté ne prouve est marqué comme tel — voir la [matrice invariant → test](docs/matrice-invariants.md), tenue sans complaisance.

---

## État réel au 29/07/2026

`cargo test --workspace` : **195 tests, 0 échec** en local, et **exécutés en CI sur un PostgreSQL réel** (GitHub Actions, service pgvector/pg16).

Ce nombre a **baissé** le 29/07, et c'est un progrès : cinq `crate_compiles() {}` — un par crate, corps vide — comptaient comme des tests verts sans rien prouver de plus que le compilateur, qui compile la crate de toute façon. Ils sont supprimés, et une garde refuse désormais tout test au corps vide. Un chiffre qu'on gonfle est un chiffre auquel on ne peut plus se fier.

**Et l'on sait qu'ils ont tourné.** Sept tests exigent une base réelle et se sautent poliment sans elle — commodité en local, mais le plus beau trou du dispositif de preuve : la base ne leur est fournie que par *une ligne* de `ci.yml`, dont la disparition rendrait sept tests verts en ne prouvant plus rien. Depuis le 29/07, se sauter **en intégration continue est un échec**, et tout test qui se conditionne doit le faire par une variable que cette garde couvre — sans quoi un futur test se sauterait à jamais sous une variable que personne ne surveille. Un vert de complaisance est pire qu'un rouge : il s'affiche comme une preuve.

**Toute preuve existante a été vue ÉCHOUER, volontairement.** Un test vert ne prouve rien tant qu'on ne l'a pas vu rouge : il peut n'avoir jamais rien vérifié. Chaque invariant marqué prouvé a donc été falsifié exprès pour voir si sa preuve le remarquait — refus par défaut inversé en autorisation, contenu externe concaténé à l'instruction, rechargement du solde retiré, `REVOKE DELETE` ôté d'une migration, `tokio` introduit dans le domaine. Toutes ont rougi, et le détail est dans la [rubrique « Sensibilité » de la matrice](docs/matrice-invariants.md). Un piège y est consigné plutôt que tu : un premier essai a conclu à tort qu'une preuve était insensible, alors que la CI s'était arrêtée avant de l'exécuter — un test qui n'a pas tourné ressemble en tout point à un test qui n'a rien vu. La CI passe donc en `--no-fail-fast`, non par confort de diagnostic, mais parce que sans cela un sabotage peut mentir dans le sens rassurant.

Le journal ne se contente pas d'être intact : **sa séquence est vérifiée sur les données réelles** — pas de clôture sans intention, pas d'intention en double, rien après une clôture. Un appel resté ouvert (validation en attente, redémarrage) est signalé comme *information*, jamais comme corruption : sans cette asymétrie, une panne banale ressemblerait à une falsification.

**La tranche verticale traverse** (CI n°17 et suivantes) : une tâche est créée, soumise à la politique, exécutée, débitée du crédit, journalisée dans PostgreSQL (attestations chaînées et contenus séparés), **interrompue, reprise depuis la base**, et terminée avec le même résultat qu'un parcours direct. **Les effets d'outils sont idempotents** : un pas rejoué après une panne ne renvoie pas un second mail — l'effet déjà réalisé est reconnu par une identité dérivée de `(task_id, iteration)`, et prouvé par un test qui reconstitue le scénario (effet accompli, état revenu en arrière). Les deux pannes silencieuses inverses sont testées aussi : l'identité ne peut être partagée ni entre deux tâches, ni entre deux itérations, et les effets d'une tâche ne fuient pas vers une autre — sans quoi un appel jamais exécuté passerait pour déjà fait, et le mail ne partirait **jamais**.

**L'invariant 1 (isolation multi-tenant par RLS) est prouvé depuis le 28/07/2026** : CI run 30223145565 verte avec la politique en place, puis run 30223419721 **rouge** sur une branche jetable où la politique `tenant_isolation` avait été volontairement retirée — la preuve que le test sait échouer. La branche de sabotage a été supprimée après lecture du rouge.

### Construit et prouvé par des tests exécutés

| Crate | Contenu réel |
|---|---|
| `kollega-core` | Types du domaine validés à la construction (`Cents(i64)`, plafond de coût, statuts de tâche…) ; assemblage de prompt à trois origines `SystemInstruction` / `UserRequest` / `ExternalContent` rendu non-confondable par les types (invariant 7) — **confinement pur : le contenu externe est transporté verbatim**, corpus hostile de 34 cas + proptest ([modèle de menace v2](docs/invariant-7-modele-de-menace.md)). |
| `kollega-audit` | Chaîne de hachage **pure** par organisation : encodage canonique injectif (round-trip par décodeur indépendant), hauteur incluse dans les octets hachés, ancre de confiance monotone ; **spécification confirmée par différentiel Rust ↔ Python en CI** (14 014 vecteurs, zéro divergence). Deux types distincts portent la règle : une entrée **produite** par le domaine ne peut pas mentir (champs privés, empreinte calculée — forger ne compile pas), une entrée **relue du stockage** le peut, et c'est nécessaire : rendre la corruption inreprésentable la rendrait indétectable. [Modèle de menace](docs/audit-modele-de-menace.md) qui dit aussi ce que la chaîne ne prouve **pas**. |
| `kollega-policy` | Moteur de décision **pur** : refus par défaut (outil sans règle = refusé), **bornes à deux étages** — seuil de validation puis limite dure au-dessus, que nulle validation ne lève (le « souple sans plafond » n'est plus représentable) ; fail-open du préfixe vide fermé. **La boucle l'appelle directement**, avec l'appel complet : les bornes ne sont plus inertes. |
| `kollega-runtime` | Noyau crédit + plafond **pur** : refus avant facturation, solde jamais négatif, arrêt propre `aborted_cost_ceiling` distinct d'un échec ; machine à états **reprise-compatible** persistée via une **enveloppe versionnée** (`TASK_STATE_FORMAT_VERSION` = 3 — une enveloppe d'une autre version est refusée net, jamais mal relue) ; chaque appel d'outil porte son itération, ce qui rend son identité **dérivable** — prérequis de l'idempotence. |
| `kollega-api` | Hachage argon2id, vérification aux paramètres stockés bornés (plancher 8 Mio, **plafond 64 Mio**) + **sémaphore de concurrence** (au-delà de la borne, les vérifications attendent, elles n'échouent pas) — [ADR-0006 amendée](docs/adr/0006-verification-des-mots-de-passe.md). **Le serveur démarre pour de vrai depuis le 29/07** : un test ouvre une vraie socket, lance `axum::serve` et parle HTTP dessus — `GET /health` répond `200 ok` après un `SELECT 1` réel sur PostgreSQL. C'était un angle mort : `serve` est la commande par défaut de l'image publiée et signée, et aucun test ne l'avait jamais démarrée (la crate ne contenait qu'un `crate_compiles()`, retiré). Une seule route existe ; la branche dégradée (503) n'est pas couverte, et c'est écrit dans le test plutôt que masqué. |
| `kollega-store` | Point de passage unique du contexte d'organisation (`SET LOCAL app.current_org`) + garde textuelle exécutée. Le pilote de la tranche parle à la chaîne d'audit **uniquement par des dépôts** dont la surface n'a que `append`/`read` : retirer une preuve n'y est pas exprimable, et deux gardes textuelles (forme du trait, SQL de la persistance) échouent si ça change. |
| Garde-fou global | Test du graphe de dépendances qui **échoue** si une crate d'entrée-sortie entre dans `kollega-core` (invariant 11) — liste blanche sur toutes les sections de Cargo.toml, **et fermeture transitive lue dans le graphe résolu** depuis le 29/07 : le contrôle des manifestes ne voyait que le *déclaré*, une dépendance arrivant par un intermédiaire anodin passait inaperçue (sabotage : ajouter `tokio` au domaine révèle **tokio, socket2 et mio**). Le domaine ne tire par ailleurs **ni entropie ni horloge** — la reprise repose sur des identités dérivables, un `new_v4` y casserait l'idempotence, et l'unification des features de cargo empêche le compilateur de défendre cette frontière. |

### Prouvé aussi, depuis le 28/07/2026

- **La réversibilité des migrations** (invariant 13) : le job CI `reversibilite` joue up → down → diff avec l'état vierge → re-up → diff (schéma, rôles, extensions, ACL effective) sur un cluster dédié — **vert à la run n°15**, après cinq rouges instructifs (réglage de la comparaison d'ACL, puis jeton aléatoire `\restrict` de pg_dump ≥ 18). Dossier de preuve archivé sur la branche `ci-diagnostic`. Réserve : prouvé via psql — l'outillage applicatif (`sqlx::migrate!`) n'a pas encore de chemin de descente.
- **Et depuis le 29/07, que chaque migration en ait une.** Le job prouvait que les descentes *existantes* ramènent au vierge, jamais qu'une migration en possède une. L'écart n'était pas théorique : `sqlx::migrate!` accepte une migration sans `.down.sql` (vérifié, recompilation forcée à l'appui), et le job ne compare que schéma, rôles, extensions et ACL — une migration de **données** serait passée verte sans descente, l'invariant 13 violé en silence tout en restant marqué « prouvé ». Une garde l'exige maintenant, et rend enfin utilisable la seconde branche de l'invariant : l'irréversibilité *assumée*, qui réclame un marqueur **et** une justification écrite dans le fichier.

### Pas encore construit

- Aucun binaire qui tourne en continu, au sens d'un service exploité. Le serveur, lui, existe et démarre : `kollega serve` se connecte à PostgreSQL et sert `GET /health` (prouvé par un test qui l'écoute vraiment). Mais il n'expose **aucune route métier**, aucune authentification de requête, aucune session — et rien n'est déployé nulle part. Pas d'appel de modèle, pas de client MCP.
- `kollega-memory`, `kollega-tools` : squelettes vides — et depuis le 29/07 ils n'ont plus aucun test, ce qui est exact : il n'y a rien à couvrir. Le `crate_compiles()` qui y siégeait laissait croire l'inverse.
- `kollega-model` **existe mais n'est branché nulle part** (près de 300 lignes, **zéro dépendant** — vérifié par une garde, pas affirmé) : le contrat `ModelProvider` réel, la clé d'API expurgée et les quatre modes d'échec sont écrits et testés, mais la boucle passe par son propre port, qui ne transporte qu'un numéro d'itération. Ses tests verts ne prouvent donc rien du produit. Deux conséquences qu'il serait malhonnête de taire : en aval de l'assemblage, l'invariant 7 n'a aujourd'hui **aucun chemin réel** à protéger ; et l'invariant 5 ne pourra devenir « vérifié *avant* » que le jour où la boucle recevra l'estimation de jetons que `ModelRequest` porte déjà. Le branchement engage la conception de la boucle d'agent (M3/M4) : il appartient au propriétaire, il est consigné dans [`docs/questions-nuit.md`](docs/questions-nuit.md).
- Connecteurs OAuth, persistance de l'audit et des crédits, interface, facturation Stripe : jalons M2–M6, non commencés.

### Jalons ([détail](docs/jalons.md))

| Jalon | État |
|---|---|
| M0 — Socle multi-tenant (RLS, rôles, CI, image) | **Prouvé le 28/07/2026, intégralement** : isolation RLS sur PostgreSQL réel (run 30223145565), sensibilité démontrée (run 30223419721 rouge, politique retirée), réversibilité des migrations jouée et vérifiée (run n°15), image OCI signée cosign + SBOM publiés — et **la signature est vérifiée depuis le 29/07** (run n°58), contre une identité épinglée : jusque-là on prouvait seulement que `cosign sign` rendait 0, jamais que quiconque pouvait authentifier l'image. |
| M1 — Identité, audit, politiques | **Surface pure faite en avance** (types, chaîne d'audit, moteur de politiques, argon2id) ; la persistance PostgreSQL reste à brancher. |
| M3 — Runtime et crédits | **Noyau pur fait en avance** (budget, machine à états) ; la concurrence sur le crédit est [spécifiée](docs/credits-concurrence.md), non implémentée. |
| M2, M4–M7 | Non commencés. |

### Invariants — 13, état résumé

**Prouvés** : **1** (isolation RLS, sensibilité comprise ; le vectoriel attend M5), **2** (tout appel passe par le vrai moteur avec la requête complète, et exécuter un outil sans décision **ne compile pas** — l'exécuteur exige un permis que seule la boucle délivre), **3** (deux entrées par appel — validées sur la chaîne *persistée*), **4** (chaîne d'audit : ajout seul éprouvé par le rôle en CI, double attestation rendue impossible, spec confirmée par différentiel indépendant), **7** (confinement, corpus 34 cas + proptest), **11** (core sans entrée-sortie, **par transitivité** et non plus seulement au vu des manifestes), **13** (réversibilité des migrations). **6** : le plafond arrête la tâche dans la boucle réelle, rien n'est facturé, et le statut persisté vaut `aborted_cost_ceiling` — le dirigeant lit que la tâche s'est arrêtée à *sa* borne, pas qu'elle a planté. **Prouvé en partie** : **5** — le débit est atomique et prouvé sous concurrence réelle (deux tâches parallèles ne mettent jamais le solde à découvert), **sensibilité prouvée le 29/07** — en retirant le rechargement du solde, la CI n°68 devient rouge avec deux tâches abouties au lieu d'une, soit 120 dépensés sur un solde de 100, mais la vérification *avant* l'appel de modèle n'est pas tenue : le coût n'est connu qu'après. Décision ouverte, consignée dans [`docs/questions-nuit.md`](docs/questions-nuit.md). **12** (**sensibilité prouvée le 29/07** : sur une branche jetable où le `REVOKE DELETE` de la migration 0006 est retiré, la CI n°65 est **rouge** sur ce test-là et lui seul, avec le message attendu — la preuve savait donc échouer) : aucune suppression physique n'est possible par le rôle applicatif sur aucune table tenant — seul le contenu d'audit est purgeable, par un acte nommé et tracé qui laisse la chaîne vérifiable. Restent à écrire : l'effacement logique lui-même et l'export RGPD. **Sans test aujourd'hui** : 8, 9, 10 (jalons M2, M5, M6 non commencés). Détail et réserves : [`docs/matrice-invariants.md`](docs/matrice-invariants.md).

---

## Architecture (décisions tranchées, ne pas rouvrir sans ADR)

Monolithe modulaire Rust (un binaire, des crates aux frontières nettes) ; PostgreSQL 16 + pgvector, seul moteur, isolation par **Row-Level Security** posée dès la migration 0001 (rôle applicatif sans `BYPASSRLS`) ; Python uniquement en serveurs MCP (il transforme, il ne décide jamais) ; modèles via API externe derrière un trait `ModelProvider` ; interface web rendue serveur ; image OCI unique signée (cosign) avec SBOM ; hébergement cloud français. Raisons : [`CLAUDE.md`](CLAUDE.md) §5 et [`docs/adr/`](docs/adr/).

## Développement

Un exploitant peut vérifier le journal d'une organisation sans nous :

```sh
kollega audit verify --org <uuid>   # code 0 = intègre, 1 = rompue (message nommé)
```

```sh
cargo test --workspace              # tests purs : passent sans base
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

Les sept tests exigeant une base ne s'activent que si `TEST_MIGRATE_DATABASE_URL` pointe vers un PostgreSQL (voir `ci.yml`) : sans base, ils se sautent en le disant. **En intégration continue, ce saut est un échec** — `CI` positionnée sans la variable fait rougir la garde `integration_tests_ran`. Migrations dans [`migrations/`](migrations/), conteneur dans [`deploy/`](deploy/).

---

*Documentation en français ; code, types et identifiants SQL en anglais. Ce README est mis à jour à chaque changement d'état réel du code.*
