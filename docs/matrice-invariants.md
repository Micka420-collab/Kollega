# Matrice invariant → test

État au 28/07/2026, mis à jour après la première exécution de la CI
(dépôt distant github.com/Micka420-collab/Kollega, PostgreSQL réel en CI).
Aucune conclusion flatteuse : un invariant dont le test existe mais n'a
**jamais tourné** n'est PAS couvert, et c'est écrit tel quel. Colonne
« Exécuté » = le test a réellement tourné (sur cette machine ou en CI).

| # | Invariant (résumé) | Test | Fichier | Exécuté | Commentaire |
|---|---|---|---|---|---|
| 1 | Isolation par la base (RLS) | `tenant_isolation_holds_and_the_test_is_sensitive`, `every_tenant_table_has_forced_rls_and_a_policy` | `kollega-store/tests/rls_isolation.rs`, `rls_structural.rs` | **OUI — CI, 28/07/2026** | **Prouvé, sensibilité comprise** : run 30223145565 verte (politique en place), run 30223419721 ROUGE sur branche jetable avec la politique `tenant_isolation` de `users` retirée — échec à l'étape des tests, fmt/clippy verts. Réserves : les journaux bruts de CI sont inaccessibles sans jeton (impossible de distinguer lequel des deux tests RLS a produit le rouge — les deux détectent la politique manquante) ; la partie « y compris en recherche vectorielle » reste non couverte, aucune table vectorielle n'existe. |
| 2 | Aucun appel d'outil sans moteur de politiques | `no_matching_rule_always_denies`, `beyond_hard_limit_is_always_denied_never_approvable` (+ 22 tests policy) ; **`the_hard_limit_now_stops_the_call_from_inside_the_loop`** ; tranche verticale avec bornes réelles (CI) | `kollega-policy/…`, `kollega-runtime/src/machine.rs`, `kollega-store/tests/vertical_slice.rs` | **OUI — CI, 29/07/2026** | **Couvert dans la boucle** (nuit du 28 au 29) : le trait intermédiaire a été SUPPRIMÉ — il ne transportait que le nom de l'outil, ce qui rendait les bornes à deux étages inertes en production malgré leurs tests. `drive` appelle `kollega_policy::decide` directement avec la requête complète ; 500 destinataires contre une limite dure de 100 sont refusés depuis la boucle, rien n'est facturé. Réserve maintenue et honnête : `ToolRunner` reste un trait public — rien n'empêche *techniquement* un appel hors `drive`. Un type témoin délivré par le moteur fermerait ce dernier chemin. |
| 3 | Deux entrées d'audit par appel d'outil (intention + résultat) | `scenario_nominal`, `scenario_denied_by_policy` (pur) ; **`verify_org_sequence` sur la chaîne PERSISTÉE** + validateur asymétrique (`records.rs`, 7 cas) | `kollega-runtime/src/machine.rs`, `kollega-store/src/driver.rs`, `kollega-audit/src/records.rs` | **OUI — CI, 29/07/2026** | **Couvert jusqu'à la persistance** (nuit du 28 au 29) : le pont machine→`AuditRecord` existe, la colonne `tool_call_id` porte l'identité dérivée, et la séquence est validée sur les VRAIES données — clôture orpheline, double intention, double clôture, enregistrement après clôture. L'asymétrie tient : un appel ouvert (validation en attente, redémarrage) est une INFORMATION, pas une violation. Réserve : le contenu attesté reste l'événement de machine, pas encore la requête/réponse d'outil réelle (M2). |
| 4 | Journal ajout seul, chaîné, ancré | `reference_vectors`, `chain_properties`, `canonical_injectivity`, tests `anchor` ; **GRANT sans DELETE éprouvés en CI**, garde SQL anti-retrait, unicité d'attestation | `kollega-audit/…`, `migrations/0003`, `0005`, `kollega-cli/tests/repository_shape.rs` | **OUI — CI, 29/07/2026** | **Couvert au-delà du pur.** « Ajout seul » n'est plus une lecture de code : le rôle applicatif ne PEUT pas supprimer dans la chaîne (testé en CI, l'échec est asserté), la surface du dépôt ne l'exprime pas, et une garde textuelle échoue si un `DELETE`/`UPDATE` apparaît dans la persistance. Depuis la migration 0005, une DOUBLE attestation du même appel est impossible — le journal ne peut plus dire qu'un outil s'est exécuté deux fois quand il ne l'a fait qu'une. Restent : `audit verify` en CLI, et l'ancrage opérationnel. |
| 5 | Crédit vérifié avant appel, débité atomiquement | `balance_never_negative_and_debits_are_conserved` (+ 8 tests budget) ; **`two_concurrent_tasks_never_overdraw_the_credit`** (deux tâches en parallèle sur une base réelle) | `kollega-runtime/src/budget.rs`, `kollega-store/tests/credits_concurrency.rs` | **OUI (débit) — CI, 29/07/2026** | **L'ATOMICITÉ est prouvée sous concurrence réelle** : deux tâches de la même organisation, chacune porteuse d'un instantané du solde à 100, tentent de dépenser 60 en parallèle — une seule passe, le solde finit à 40, jamais sous zéro. C'est ce que le verrou `FOR UPDATE` + `Budget::refreshed` achètent : sans rechargement, les deux instantanés auraient autorisé la même dépense deux fois. **Réserve maintenue** : « vérifié AVANT l'appel de modèle » n'est toujours pas tenu — le coût n'est connu qu'après. Décision de conception ouverte (estimation préalable ou réservation), consignée. |
| 6 | Toute tâche a un plafond de coût | `exact_ceiling_is_allowed_but_one_more_aborts`, `cost_ceiling_exceeded_is_strict` | `kollega-runtime/src/budget.rs`, `kollega-core` | Oui | **Partiel** : plafond pur prouvé (arrêt propre, distinct d'un échec, débordement i64 traité). L'application dans la boucle réelle reste au BLOC 10 / M3. |
| 7 | Contenu externe jamais concaténé aux instructions | `hostile_content_never_reaches_instruction_fields` (corpus 34 cas), `hostile_content_is_transported_intact` (verbatim), `compile_transports_external_content_verbatim` (proptest) | `kollega-core/src/prompt.rs`, `tests/segment_assembly.rs`, `tests/properties.rs` | Oui | **Couvert au niveau assemblage — CONFINEMENT seul depuis le 28/07 (modèle de menace v2)** : le contenu externe est transporté verbatim, plus aucune neutralisation. La NON-concaténation en aval reste un contrat documenté, à tester au M3 ; l'affichage sûr (bidi/invisibles) est transféré à M6. |
| 8 | Jetons OAuth chiffrés au repos | — | — | **NON — AUCUN** | Périmètre M2 (connecteurs). Non commencé. |
| 9 | Procédure métier validée avant mémoire (`validated_by`) | — | — | **NON — AUCUN** | Périmètre M5 (mémoire). Non commencé. |
| 10 | Sortie porte la mention d'origine IA | — | — | **NON — AUCUN** | Périmètre M6 (interface). Non commencé. |
| 11 | `kollega-core` sans entrée-sortie | `dependency_graph_is_respected` | `kollega-cli/tests/dependency_graph.rs` | Oui | **Couvert.** Liste blanche appliquée à toutes les sections (dependencies, dev, build, `[target.*]`). |
| 12 | Aucune suppression physique (effacement logique + purge tracée) | — | Schéma : `deleted_at` sur organizations/users (seules tables existantes) | **NON — AUCUN test de code** | Le schéma porte `deleted_at` ; aucune logique de purge ni de garde contre le `DELETE` physique n'est écrite — et `GRANT DELETE` est accordé à `kollega_app` (RGPD, jalon ultérieur). |
| 13 | Toute migration réversible | Job CI `reversibilite` : up → down → diff avec l'état VIERGE → re-up → diff (schéma pg_dump normalisé, rôles, extensions, ACL effective), cluster dédié | `.github/workflows/ci.yml` | **OUI — CI, 28/07/2026, run n°15** | **Prouvé** : le down rend l'état vierge exact, le re-up reproduit l'état à l'identique. Cinq passages rouges d'abord — un réglage réel (comparer l'ACL EFFECTIVE : un `nspacl` matérialisé après GRANT+REVOKE n'est pas le texte du NULL initial) et un faux positif d'outillage (jeton aléatoire `\restrict` de pg_dump ≥ 18) ; dossier archivé sur la branche `ci-diagnostic`. Réserve : prouvé via psql — l'outillage applicatif (`sqlx::migrate!`) n'a toujours aucun chemin de descente. |

## Où vit chaque invariant (bloc 4 — colonne détachée pour lisibilité)

Valeurs : **type** / **contrainte de schéma** / **RLS** / **test** /
***prose seulement***. Un invariant en prose seulement est un invariant
qu'AUCUN mécanisme n'applique — cette rubrique existe pour le rendre
visible (ADR-0007).

| # | Où il vit |
|---|---|
| 1 | **RLS** (politique + FORCE, rôle sans BYPASSRLS) — le type porte `org_id`, la garantie vient du schéma |
| 2 | **type** (plus de trait intermédiaire : `drive` ne peut appeler que le vrai moteur, avec la requête complète) + **test** — reste hors couverture : un appel de `ToolRunner` hors de la boucle |
| 3 | **type** (`AuditRecord` : Intent/Outcome/Abandoned) + **contrainte de schéma** (`tool_call_id`, unicité par appel et par action) + test sur données réelles |
| 4 | **contrainte de schéma** (GRANT INSERT+SELECT seuls sur `audit_chain` ; unicité d'attestation en 0005 — la double clôture est inexprimable) + **type** (dépôt sans méthode de retrait) + **test** (deux gardes textuelles) |
| 5 | **type** (`Budget::charge`, `Budget::refreshed`) + **contrainte de schéma** (`CHECK balance_cents >= 0`) |
| 6 | **type** (`Budget`, `CostCeiling`) + test |
| 7 | **type** (`Segment`/`CompiledPrompt`, confinement verbatim) + test (corpus + proptest) |
| 8 | ***prose seulement*** (M2 non commencé) — `ApiKey` expurgée existe déjà pour les clés de modèle |
| 9 | ***prose seulement*** (M5 non commencé) |
| 10 | ***prose seulement*** (M6 non commencé) |
| 11 | **test** (garde du graphe de dépendances, toutes sections) |
| 12 | **contrainte de schéma** (pas de GRANT DELETE sur `tasks` ni `audit_chain` ; DELETE sur le seul contenu purgeable) + purge nommée `purge_org` — la purge TRACÉE reste partielle |
| 13 | **test** (job CI `reversibilite` : down rend le vierge, re-up reproduit) |

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
- **Prouvés en partie, le reste dépend d'un jalon futur** : 5, 6.
- **Aucun test aujourd'hui** : 8, 9, 10, 12 (jalons non commencés).

Les deux dettes de preuve du socle (1 et 13) sont soldées par une CI qui a
montré qu'elle sait passer au rouge — dans les deux cas c'est le rouge qui
a fait la preuve. Dettes suivantes, dans l'ordre : les coutures de la
machine à états (2, 3, 5 : pont vers la chaîne d'audit et le vrai moteur
de politiques — options dans questions-nuit, arbitrage du propriétaire),
un chemin de descente dans l'outillage applicatif (sqlx ne descend pas),
et l'isolation vectorielle (M5).
