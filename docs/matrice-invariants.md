# Matrice invariant → test

État au 28/07/2026, mis à jour après la première exécution de la CI
(dépôt distant github.com/Micka420-collab/Kollega, PostgreSQL réel en CI).
Aucune conclusion flatteuse : un invariant dont le test existe mais n'a
**jamais tourné** n'est PAS couvert, et c'est écrit tel quel. Colonne
« Exécuté » = le test a réellement tourné (sur cette machine ou en CI).

**Ce que vaut la colonne « Exécuté » (29/07).** Elle reposait sur une
confiance : sept tests exigent une base réelle et se sautent sans elle, et
cette base ne leur était fournie que par *une ligne* de `ci.yml`. La
retirer aurait rendu sept tests verts en ne prouvant plus rien, et six
lignes de cette matrice auraient continué d'afficher « OUI — CI » en
mentant. Depuis, **se sauter en intégration continue est un échec**
(`kollega-cli/tests/integration_tests_ran.rs`), et tout test qui se
conditionne doit le faire par une variable que cette garde couvre — sinon
un futur test se sauterait à jamais sous une variable non surveillée. Les
deux gardes sont vérifiées par sabotage, témoin positif compris.

## Sensibilité — quelles preuves ont été vues ÉCHOUER

Un test vert ne prouve rien tant qu'on ne l'a pas vu rouge : il peut
n'avoir jamais rien vérifié. Cette rubrique dit, invariant par invariant,
si sa preuve a été **falsifiée volontairement** et si elle a bien rougi.
C'est une distinction plus dure que la colonne « Exécuté » : un test peut
avoir tourné mille fois sans jamais pouvoir échouer.

| # | Sabotage joué | Résultat |
|---|---|---|
| 1 | Politique `tenant_isolation` de `users` retirée (branche jetable) | CI run 30223419721 **rouge** |
| 2 | (a) champs de `ExecutionPermit` forgés ; (b) **refus par défaut inversé en autorisation** | (a) le doctest `compile_fail` échoue ; (b) 3 tests rouges dont le proptest `no_matching_rule_always_denies` |
| 3 | Filtre de tâche retiré du chargement des effets | CI run 30230062721 **rouge** (fuite d'effets entre tâches) |
| 4 | (a) `DELETE FROM audit_chain` introduit ; (b) `canonical.py` saboté en hexadécimal majuscule ; (c) champs de `ChainedEntry` rendus publics | (a) garde SQL rouge ; (b) run 30230220434 rouge à l'étape du différentiel, **tous les tests Rust verts** — le différentiel est donc bien indépendant ; (c) le `compile_fail` échoue |
| 5 | **`Budget::refreshed` retiré** de `run_task_step` : la tâche facture contre l'instantané sérialisé | CI run n°68 **rouge** — `A=Succeeded B=Succeeded`, 2 tâches abouties au lieu d'une, 120 dépensés sur un solde de 100 |
| 6 | `consumed` non mis à jour dans le noyau comptable | 4 tests rouges dont le proptest de conservation |
| 7 | **Contenu externe concaténé à l'instruction système** dans `compile` | 6 tests rouges dans 3 binaires, dont le corpus hostile cité au README |
| 11 | (a) `tokio` ajouté au domaine ; (b) `new_v4` appelé dans `ids.rs` | (a) rouge en révélant **tokio, socket2 et mio** ; (b) rouge sur le déterminisme |
| 12 | **`REVOKE DELETE` sur `users` retiré** de la migration 0006 (branche jetable) | CI run n°65 **rouge** sur ce test seul, message exact, `reversibilite` restée verte |
| 13 | Migration sans descente, justification trop courte, versions homonymes, descente orpheline | 4 rouges + témoin positif vert (l'irréversibilité justifiée passe bien) |

**Non encore falsifiés** : la commande `audit verify` en CLI et la tranche
verticale prise dans son ensemble (son test d'idempotence l'a été). Ce
n'est pas une réserve de principe : ce sont les deux dernières preuves
dont on ignore encore si elles savent échouer.

| # | Invariant (résumé) | Test | Fichier | Exécuté | Commentaire |
|---|---|---|---|---|---|
| 1 | Isolation par la base (RLS) | `tenant_isolation_holds_and_the_test_is_sensitive`, `every_tenant_table_has_forced_rls_and_a_policy`, `in_ci_no_integration_test_may_skip` (protège le « OUI — CI » de cette ligne et de toutes les autres à base réelle) | `kollega-store/tests/rls_isolation.rs`, `rls_structural.rs` | **OUI — CI, 28/07/2026** | **Prouvé, sensibilité comprise** : run 30223145565 verte (politique en place), run 30223419721 ROUGE sur branche jetable avec la politique `tenant_isolation` de `users` retirée — échec à l'étape des tests, fmt/clippy verts. Réserves : les journaux bruts de CI sont inaccessibles sans jeton (impossible de distinguer lequel des deux tests RLS a produit le rouge — les deux détectent la politique manquante) ; la partie « y compris en recherche vectorielle » reste non couverte, aucune table vectorielle n'existe. |
| 2 | Aucun appel d'outil sans moteur de politiques | `no_matching_rule_always_denies`, `beyond_hard_limit_is_always_denied_never_approvable` (+ 22 tests policy) ; **`the_hard_limit_now_stops_the_call_from_inside_the_loop`** ; tranche verticale avec bornes réelles (CI) | `kollega-policy/…`, `kollega-runtime/src/machine.rs`, `kollega-store/tests/vertical_slice.rs` | **OUI — CI, 29/07/2026** | **Couvert dans la boucle** (nuit du 28 au 29) : le trait intermédiaire a été SUPPRIMÉ — il ne transportait que le nom de l'outil, ce qui rendait les bornes à deux étages inertes en production malgré leurs tests. `drive` appelle `kollega_policy::decide` directement avec la requête complète ; 500 destinataires contre une limite dure de 100 sont refusés depuis la boucle, rien n'est facturé. **Dernier chemin fermé le 29/07** : `ToolRunner::run` exige un `ExecutionPermit`, dont les champs sont privés — seule la boucle, APRÈS décision favorable, peut en délivrer un. Exécuter un outil sans être passé par la politique **ne compile pas** (doctest `compile_fail`, vérifié par sabotage). |
| 3 | Deux entrées d'audit par appel d'outil (intention + résultat) | `scenario_nominal`, `scenario_denied_by_policy` (pur) ; **`verify_org_sequence` sur la chaîne PERSISTÉE** + validateur asymétrique (`records.rs`, 7 cas) | `kollega-runtime/src/machine.rs`, `kollega-store/src/driver.rs`, `kollega-audit/src/records.rs` | **OUI — CI, 29/07/2026** | **Couvert jusqu'à la persistance** (nuit du 28 au 29) : le pont machine→`AuditRecord` existe, la colonne `tool_call_id` porte l'identité dérivée, et la séquence est validée sur les VRAIES données — clôture orpheline, double intention, double clôture, enregistrement après clôture. L'asymétrie tient : un appel ouvert (validation en attente, redémarrage) est une INFORMATION, pas une violation. Réserve : le contenu attesté reste l'événement de machine, pas encore la requête/réponse d'outil réelle (M2). |
| 4 | Journal ajout seul, chaîné, ancré | `reference_vectors`, `chain_properties`, `canonical_injectivity`, tests `anchor` ; **GRANT sans DELETE éprouvés en CI**, garde SQL anti-retrait, unicité d'attestation | `kollega-audit/…`, `migrations/0003`, `0005`, `kollega-cli/tests/repository_shape.rs` | **OUI — CI, 29/07/2026** | **Couvert au-delà du pur.** « Ajout seul » n'est plus une lecture de code : le rôle applicatif ne PEUT pas supprimer dans la chaîne (testé en CI, l'échec est asserté), la surface du dépôt ne l'exprime pas, et une garde textuelle échoue si un `DELETE`/`UPDATE` apparaît dans la persistance. Depuis la migration 0005, une DOUBLE attestation du même appel est impossible — le journal ne peut plus dire qu'un outil s'est exécuté deux fois quand il ne l'a fait qu'une. **`audit verify` existe en CLI depuis le 29/07** et dit vrai dans les deux sens : code 0 sur chaîne saine, code 1 avec message nommé sur chaîne altérée (testé en exécutant le binaire, pas la bibliothèque). Reste : l'ancrage opérationnel (où vit l'ancre de confiance — décision d'exploitation). |
| 5 | Crédit vérifié avant appel, débité atomiquement | `balance_never_negative_and_debits_are_conserved` (+ 8 tests budget) ; **`two_concurrent_tasks_never_overdraw_the_credit`** (deux tâches en parallèle sur une base réelle) | `kollega-runtime/src/budget.rs`, `kollega-store/tests/credits_concurrency.rs` | **OUI (débit) — CI, 29/07/2026** | **L'ATOMICITÉ est prouvée sous concurrence réelle** : deux tâches de la même organisation, chacune porteuse d'un instantané du solde à 100, tentent de dépenser 60 en parallèle — une seule passe, le solde finit à 40, jamais sous zéro. C'est ce que le verrou `FOR UPDATE` + `Budget::refreshed` achètent : sans rechargement, les deux instantanés auraient autorisé la même dépense deux fois. **Réserve maintenue** : « vérifié AVANT l'appel de modèle » n'est toujours pas tenu — le coût n'est connu qu'après. Décision de conception ouverte (estimation préalable ou réservation), consignée. |
| 6 | Toute tâche a un plafond de coût | `exact_ceiling_is_allowed_but_one_more_aborts`, `cost_ceiling_exceeded_is_strict` ; **`the_cost_ceiling_stops_the_task_cleanly_and_bills_nothing`** (base réelle) | `kollega-runtime/src/budget.rs`, `kollega-store/tests/credits_concurrency.rs` | **OUI — CI, 29/07/2026** | **Couvert de bout en bout** : le plafond arrête la tâche dans la boucle RÉELLE, rien n'est facturé, et le statut persisté vaut `aborted_cost_ceiling` — pas « échec ». C'est ce qui rend la promesse lisible par le dirigeant après coup : il voit que la tâche s'est arrêtée à SA borne, pas qu'elle a planté. Le crédit large du test sépare bien les deux protections (plafond de tâche ≠ solde d'organisation). |
| 7 | Contenu externe jamais concaténé aux instructions | `hostile_content_never_reaches_instruction_fields` (corpus 34 cas), `hostile_content_is_transported_intact` (verbatim), `compile_transports_external_content_verbatim` (proptest) | `kollega-core/src/prompt.rs`, `tests/segment_assembly.rs`, `tests/properties.rs` | Oui | **Couvert au niveau assemblage — CONFINEMENT seul depuis le 28/07 (modèle de menace v2)** : le contenu externe est transporté verbatim, plus aucune neutralisation. La NON-concaténation en aval reste un contrat documenté, à tester au M3 ; l'affichage sûr (bidi/invisibles) est transféré à M6. |
| 8 | Jetons OAuth chiffrés au repos | — | — | **NON — AUCUN** | Périmètre M2 (connecteurs). Non commencé. |
| 9 | Procédure métier validée avant mémoire (`validated_by`) | — | — | **NON — AUCUN** | Périmètre M5 (mémoire). Non commencé. |
| 10 | Sortie porte la mention d'origine IA | — | — | **NON — AUCUN** | Périmètre M6 (interface). Non commencé. |
| 11 | `kollega-core` sans entrée-sortie | `dependency_graph_is_respected`, **`core_transitive_closure_contains_no_io_crate`**, **`the_domain_never_draws_randomness`** | `kollega-cli/tests/dependency_graph.rs` | Oui | **Couvert, y compris par TRANSITIVITÉ depuis le 29/07.** La liste blanche des manifestes ne voyait que le DÉCLARÉ : une crate d'E/S arrivant par un intermédiaire anodin serait passée. La fermeture est désormais lue dans le graphe résolu (`Cargo.lock`) — sabotage : ajouter `tokio` révèle aussi `socket2` et `mio`. Le domaine ne tire par ailleurs aucune entropie ni horloge (`new_v4`, `getrandom`, `SystemTime::now` interdits) : la reprise dépend de son déterminisme. |
| 12 | Aucune suppression physique (effacement logique + purge tracée) | **`the_application_role_cannot_physically_delete_anything_but_purgeable_content`** | `kollega-store/tests/no_physical_deletion.rs`, `migrations/0006` | **OUI — CI, 29/07/2026** | **Porté par le RÔLE.** La migration 0006 retire le dernier `GRANT DELETE` (il subsistait sur `organizations` et `users`, en contradiction avec la constitution). Le test tente une suppression sur les SIX tables tenant et échoue partout. Seule exception, nommée : `audit_content`, purgeable — c'est la purge RGPD, tracée par une attestation, et la chaîne reste vérifiable après. **SENSIBILITÉ PROUVÉE le 29/07** — la seule preuve de cet invariant n'avait jamais été vue échouer : branche jetable où le `REVOKE DELETE` sur `users` est retiré de la migration 0006, run n°65 **ROUGE**, `verifications` en échec sur ce test seul, message exact « kollega_app ne doit pas pouvoir supprimer dans users », `reversibilite` restée verte. Aucun dégât collatéral : un test rouge, un seul. Branche supprimée après lecture. **Restent à écrire** : l'effacement logique lui-même (`deleted_at` n'est posé par aucun code) et l'export RGPD par organisation. |
| 13 | Toute migration réversible | Job CI `reversibilite` : up → down → diff avec l'état VIERGE → re-up → diff (schéma pg_dump normalisé, rôles, extensions, ACL effective), cluster dédié ; **`every_migration_is_reversible_or_says_why_not`** | `.github/workflows/ci.yml`, `kollega-cli/tests/migrations_shape.rs` | **OUI — CI, 28/07/2026, run n°15** | **Prouvé** : le down rend l'état vierge exact, le re-up reproduit l'état à l'identique. Cinq passages rouges d'abord — un réglage réel (comparer l'ACL EFFECTIVE : un `nspacl` matérialisé après GRANT+REVOKE n'est pas le texte du NULL initial) et un faux positif d'outillage (jeton aléatoire `\restrict` de pg_dump ≥ 18) ; dossier archivé sur la branche `ci-diagnostic`. Réserve : prouvé via psql — l'outillage applicatif (`sqlx::migrate!`) n'a toujours aucun chemin de descente. **Écart fermé le 29/07** : le job prouve que les descentes EXISTANTES ramènent au vierge, il ne prouvait pas qu'une migration en AIT une. Vérifié, pas supposé — `sqlx::migrate!` accepte une migration sans `.down.sql` même après recompilation forcée, et le job ne compare que schéma, rôles, extensions et ACL : une migration de DONNÉES (remplissage, correction de lignes) serait passée verte sans descente. La garde `migrations_shape.rs` l'exige désormais, rend enfin utilisable la seconde branche de l'invariant (irréversibilité assumée, marqueur + justification d'au moins 60 caractères — sinon le mot deviendrait un laissez-passer), et refuse en outre deux versions homonymes ou une descente sans montée. Quatre sabotages rouges, témoin positif vert. |

## Où vit chaque invariant (bloc 4 — colonne détachée pour lisibilité)

Valeurs : **type** / **contrainte de schéma** / **RLS** / **test** /
***prose seulement***. Un invariant en prose seulement est un invariant
qu'AUCUN mécanisme n'applique — cette rubrique existe pour le rendre
visible (ADR-0007).

| # | Où il vit |
|---|---|
| 1 | **RLS** (politique + FORCE, rôle sans BYPASSRLS) — le type porte `org_id`, la garantie vient du schéma |
| 2 | **type** — plus de trait intermédiaire (le vrai moteur voit la requête complète) ET permis d'exécution inconstructible hors de la boucle : exécuter sans décision ne compile pas |
| 3 | **type** (`AuditRecord` : Intent/Outcome/Abandoned) + **contrainte de schéma** (`tool_call_id`, unicité par appel et par action) + test sur données réelles |
| 4 | **contrainte de schéma** (GRANT INSERT+SELECT seuls sur `audit_chain` ; unicité d'attestation en 0005 — la double clôture est inexprimable) + **type** (dépôt sans méthode de retrait) + **test** (deux gardes textuelles) |
| 5 | **type** (`Budget::charge`, `Budget::refreshed`) + **contrainte de schéma** (`CHECK balance_cents >= 0`) |
| 6 | **type** (`Budget`, `CostCeiling`) + test |
| 7 | **type** (`Segment`/`CompiledPrompt`, confinement verbatim) + test (corpus + proptest) |
| 8 | ***prose seulement*** (M2 non commencé) — `ApiKey` expurgée existe déjà pour les clés de modèle |
| 9 | ***prose seulement*** (M5 non commencé) |
| 10 | ***prose seulement*** (M6 non commencé) |
| 11 | **test** (garde du graphe de dépendances : toutes sections des manifestes, **plus la fermeture transitive du graphe résolu**, plus l'interdiction d'entropie et d'horloge dans le domaine) |
| 12 | **contrainte de schéma** (aucun GRANT DELETE sur les six tables tenant depuis 0006 ; seul le contenu purgeable en garde un) + **test** — restent en *prose seulement* : l'effacement logique et l'export RGPD |
| 13 | **test**, en deux morceaux qui ne se recouvrent pas : le job CI `reversibilite` prouve que les descentes EXISTANTES ramènent au vierge ; la garde `migrations_shape.rs` prouve que chaque montée EN A une — ou justifie de ne pas en avoir |

## Lecture d'ensemble — sans complaisance

- **Prouvés aujourd'hui** : **1 (CI du 28/07/2026, sensibilité démontrée
  par branche jetable — réserve : le vectoriel n'existe pas encore)**,
  4 (pur, spec confirmée par différentiel Rust↔Python en CI),
  7 (assemblage en confinement, verbatim), 11,
  **13 (CI run n°15 : down rend le vierge, re-up reproduit l'état)**.
- **Promus dans la nuit du 28 au 29/07** : **2** (la boucle appelle le vrai
  moteur avec la requête complète — les bornes à deux étages étaient
  inertes jusque-là), **3** (la séquence d'appels est validée sur la chaîne
  PERSISTÉE, plus seulement en pur) et **4** (l'ajout seul est éprouvé en
  CI par le rôle, et la double attestation est devenue impossible).
- **6 également promu** : le plafond arrête la tâche dans la boucle réelle,
  statut distinct persisté, rien facturé.
- **Prouvé en partie** : **5** — le débit atomique l'est (concurrence
  réelle testée) ; « vérifié AVANT l'appel de modèle » ne l'est pas, et
  trois options sont posées dans `questions-nuit.md` sans être tranchées.
- **12 promu aussi** : aucune suppression physique n'est possible par le
  rôle applicatif, sur aucune table tenant (migration 0006 + test).
  Restent en prose : l'effacement logique et l'export RGPD.
- **Aucun test aujourd'hui** : 8, 9, 10 (jalons M2, M5, M6 non commencés).

Les deux dettes de preuve du socle (1 et 13) sont soldées par une CI qui a
montré qu'elle sait passer au rouge — dans les deux cas c'est le rouge qui
a fait la preuve. Dettes suivantes, dans l'ordre : les coutures de la
machine à états (2, 3, 5 : pont vers la chaîne d'audit et le vrai moteur
de politiques — options dans questions-nuit, arbitrage du propriétaire),
un chemin de descente dans l'outillage applicatif (sqlx ne descend pas),
et l'isolation vectorielle (M5).
