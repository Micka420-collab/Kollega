# Matrice invariant → test

État au 28/07/2026, mis à jour après la première exécution de la CI
(dépôt distant github.com/Micka420-collab/Kollega, PostgreSQL réel en CI).
Aucune conclusion flatteuse : un invariant dont le test existe mais n'a
**jamais tourné** n'est PAS couvert, et c'est écrit tel quel. Colonne
« Exécuté » = le test a réellement tourné (sur cette machine ou en CI).

| # | Invariant (résumé) | Test | Fichier | Exécuté | Commentaire |
|---|---|---|---|---|---|
| 1 | Isolation par la base (RLS) | `tenant_isolation_holds_and_the_test_is_sensitive`, `every_tenant_table_has_forced_rls_and_a_policy` | `kollega-store/tests/rls_isolation.rs`, `rls_structural.rs` | **OUI — CI, 28/07/2026** | **Prouvé, sensibilité comprise** : run 30223145565 verte (politique en place), run 30223419721 ROUGE sur branche jetable avec la politique `tenant_isolation` de `users` retirée — échec à l'étape des tests, fmt/clippy verts. Réserves : les journaux bruts de CI sont inaccessibles sans jeton (impossible de distinguer lequel des deux tests RLS a produit le rouge — les deux détectent la politique manquante) ; la partie « y compris en recherche vectorielle » reste non couverte, aucune table vectorielle n'existe. |
| 2 | Aucun appel d'outil sans moteur de politiques | `no_matching_rule_always_denies`, `unknown_tool_is_denied_by_default` (+ 18 tests policy) ; `scenario_denied_by_policy` | `kollega-policy/…`, `kollega-runtime/src/machine.rs` | Oui | **Partiel** : la DÉCISION pure (refus par défaut) est prouvée, et la machine à états (bloc 10) fait passer tout appel par `decide` — mais son `PolicyEngine` local ne transporte que le NOM d'outil (pas les bornes de kollega-policy), et rien n'empêche un appel hors `drive`. L'enforcement réel reste au M3. |
| 3 | Deux entrées d'audit par appel d'outil (intention + résultat) | `scenario_nominal` (paire intention/complétion), `scenario_denied_by_policy` (intention sans complétion) | `kollega-runtime/src/machine.rs` | Oui | **Partiel** : testé au niveau de la machine PURE (bloc 10) — le journal est un `Vec<AuditEvent>` local, PAS la chaîne `kollega-audit`. Le pont vers le journal chaîné n'existe pas (couture consignée). |
| 4 | Journal ajout seul, chaîné, ancré | `reference_vectors`, `chain_properties`, `canonical_injectivity`, tests `anchor` | `kollega-audit/…` | Oui | **Couvert pour la partie pure** : chaînage, injectivité, ancrage, détection à la bonne position. « Ajout seul » : la crate n'a AUCUNE API d'update/delete (vérifiable par lecture). La colonne SQL en ajout seul et `audit verify` opérationnel restent au jalon de persistance. |
| 5 | Crédit vérifié avant appel, débité atomiquement | `balance_never_negative_and_debits_are_conserved` (+ 8 tests budget) | `kollega-runtime/src/budget.rs` | Oui | **Partiel** : la règle comptable pure est prouvée (refus avant facturation, solde ≥ 0, conservation). L'ATOMICITÉ sous concurrence exige un verrou de base — spécifiée dans `docs/credits-concurrence.md`, non implémentée. |
| 6 | Toute tâche a un plafond de coût | `exact_ceiling_is_allowed_but_one_more_aborts`, `cost_ceiling_exceeded_is_strict` | `kollega-runtime/src/budget.rs`, `kollega-core` | Oui | **Partiel** : plafond pur prouvé (arrêt propre, distinct d'un échec, débordement i64 traité). L'application dans la boucle réelle reste au BLOC 10 / M3. |
| 7 | Contenu externe jamais concaténé aux instructions | `hostile_content_never_reaches_instruction_fields` (corpus 34 cas), `segment_*`, `no_manipulation_character_survives_neutralization` | `kollega-core/src/prompt.rs`, `tests/segment_assembly.rs` | Oui | **Couvert au niveau assemblage.** La NON-concaténation en aval (`ModelProvider` transportant des rôles distincts) est un contrat documenté (`docs/invariant-7-modele-de-menace.md`), à tester au M3. |
| 8 | Jetons OAuth chiffrés au repos | — | — | **NON — AUCUN** | Périmètre M2 (connecteurs). Non commencé. |
| 9 | Procédure métier validée avant mémoire (`validated_by`) | — | — | **NON — AUCUN** | Périmètre M5 (mémoire). Non commencé. |
| 10 | Sortie porte la mention d'origine IA | — | — | **NON — AUCUN** | Périmètre M6 (interface). Non commencé. |
| 11 | `kollega-core` sans entrée-sortie | `dependency_graph_is_respected` | `kollega-cli/tests/dependency_graph.rs` | Oui | **Couvert.** Liste blanche appliquée à toutes les sections (dependencies, dev, build, `[target.*]`). |
| 12 | Aucune suppression physique (effacement logique + purge tracée) | — | Schéma : `deleted_at` sur organizations/users (seules tables existantes) | **NON — AUCUN test de code** | Le schéma porte `deleted_at` ; aucune logique de purge ni de garde contre le `DELETE` physique n'est écrite — et `GRANT DELETE` est accordé à `kollega_app` (RGPD, jalon ultérieur). |
| 13 | Toute migration réversible | Fichiers `.down.sql` présents | `migrations/*.down.sql` | **NON — même après la CI verte** | La CI n'exécute PAS les `.down.sql` (aucune étape ne les joue, et aucun chemin de descente n'existe dans l'outillage : `sqlx::migrate!().run()` ne descend pas, pas de sqlx-cli). La réversibilité reste non prouvée. |

## Lecture d'ensemble — sans complaisance

- **Prouvés aujourd'hui** : **1 (CI du 28/07/2026, sensibilité démontrée
  par branche jetable — réserve : le vectoriel n'existe pas encore)**,
  4 (pur), 7 (assemblage), 11.
- **Prouvés en partie, le reste dépend d'un jalon futur** : 2, 3, 5, 6.
- **Aucun test aujourd'hui** : 8, 9, 10, 12 (jalons non commencés) et
  **13 : les `.down.sql` n'ont toujours jamais tourné — la CI actuelle ne
  les joue pas**.

La première dette (invariant 1 jamais exécuté) est soldée : l'isolation
est prouvée par une CI qui sait passer au rouge. Les dettes suivantes,
dans l'ordre : la réversibilité des migrations (13, exige un chemin de
descente dans l'outillage), la référence `canonical.py` jamais confrontée
au Rust (le runner CI a Python), et les coutures de la machine à états
(2, 3, 5 : pont vers la chaîne d'audit et le vrai moteur de politiques).
