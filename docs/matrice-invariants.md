# Matrice invariant → test

État au 28/07/2026. Aucune conclusion flatteuse : un invariant dont le test
existe mais n'a **jamais tourné** n'est PAS couvert, et c'est écrit tel quel.
Colonne « Exécuté » = le test a réellement tourné sur cette machine
(`cargo test`, sans base) ou reste en attente d'un environnement.

| # | Invariant (résumé) | Test | Fichier | Exécuté | Commentaire |
|---|---|---|---|---|---|
| 1 | Isolation par la base (RLS) | `tenant_isolation_holds_and_the_test_is_sensitive`, `every_tenant_table_has_forced_rls_and_a_policy` | `kollega-store/tests/rls_isolation.rs`, `rls_structural.rs` | **NON** — exige PostgreSQL | Le cœur du jalon M0. Écrit, jamais exécuté. La garde textuelle `set_config` (`sql_context_guard.rs`, exécutée) est une protection annexe, pas la preuve d'isolation. |
| 2 | Aucun appel d'outil sans moteur de politiques | `no_matching_rule_always_denies`, `unknown_tool_is_denied_by_default` (+ 18 tests policy) | `kollega-policy/…` | Oui | **Partiel** : la DÉCISION pure (refus par défaut) est prouvée. L'ENFORCEMENT (impossible d'exécuter un outil sans passer par `decide`) exige la boucle d'agent — non construite (BLOC 10 non fait). |
| 3 | Deux entrées d'audit par appel d'outil (intention + résultat) | — | — | **NON — AUCUN** | Exige la boucle d'agent et l'exécution d'outils (M2/M3). Rien à tester aujourd'hui. |
| 4 | Journal ajout seul, chaîné, ancré | `reference_vectors`, `chain_properties`, `canonical_injectivity`, tests `anchor` | `kollega-audit/…` | Oui | **Couvert pour la partie pure** : chaînage, injectivité, ancrage, détection à la bonne position. « Ajout seul » : la crate n'a AUCUNE API d'update/delete (vérifiable par lecture). La colonne SQL en ajout seul et `audit verify` opérationnel restent au jalon de persistance. |
| 5 | Crédit vérifié avant appel, débité atomiquement | `balance_never_negative_and_debits_are_conserved` (+ 8 tests budget) | `kollega-runtime/src/budget.rs` | Oui | **Partiel** : la règle comptable pure est prouvée (refus avant facturation, solde ≥ 0, conservation). L'ATOMICITÉ sous concurrence exige un verrou de base — spécifiée dans `docs/credits-concurrence.md`, non implémentée. |
| 6 | Toute tâche a un plafond de coût | `exact_ceiling_is_allowed_but_one_more_aborts`, `cost_ceiling_exceeded_is_strict` | `kollega-runtime/src/budget.rs`, `kollega-core` | Oui | **Partiel** : plafond pur prouvé (arrêt propre, distinct d'un échec, débordement i64 traité). L'application dans la boucle réelle reste au BLOC 10 / M3. |
| 7 | Contenu externe jamais concaténé aux instructions | `hostile_content_never_reaches_instruction_fields` (corpus 35 cas), `segment_*`, `no_manipulation_character_survives_neutralization` | `kollega-core/src/prompt.rs`, `tests/segment_assembly.rs` | Oui | **Couvert au niveau assemblage.** La NON-concaténation en aval (`ModelProvider` transportant des rôles distincts) est un contrat documenté (`docs/invariant-7-modele-de-menace.md`), à tester au M3. |
| 8 | Jetons OAuth chiffrés au repos | — | — | **NON — AUCUN** | Périmètre M2 (connecteurs). Non commencé. |
| 9 | Procédure métier validée avant mémoire (`validated_by`) | — | — | **NON — AUCUN** | Périmètre M5 (mémoire). Non commencé. |
| 10 | Sortie porte la mention d'origine IA | — | — | **NON — AUCUN** | Périmètre M6 (interface). Non commencé. |
| 11 | `kollega-core` sans entrée-sortie | `dependency_graph_is_respected` | `kollega-cli/tests/dependency_graph.rs` | Oui | **Couvert.** Liste blanche appliquée à toutes les sections (dependencies, dev, build, `[target.*]`). |
| 12 | Aucune suppression physique (effacement logique + purge tracée) | — | Schéma : `deleted_at` sur users/agents/documents | **NON — AUCUN test de code** | Le schéma porte `deleted_at` ; aucune logique de purge ni de garde contre le `DELETE` physique n'est écrite (RGPD, jalon ultérieur). |
| 13 | Toute migration réversible | Fichiers `.down.sql` présents | `migrations/*.down.sql` | **NON** — exige PostgreSQL | Les retours arrière sont écrits, jamais exécutés : la réversibilité n'est pas prouvée. |

## Lecture d'ensemble — sans complaisance

- **Prouvés aujourd'hui, sans réserve** : 4 (pur), 7 (assemblage), 11.
- **Prouvés en partie, le reste dépend de la base ou d'un jalon futur** :
  2, 5, 6.
- **Aucun test aujourd'hui** : 3, 8, 9, 10, 12 (jalons non commencés) et,
  crucialement, **1 et 13 ont des tests qui n'ont jamais tourné**.

L'invariant 1 est le plus important du produit (isolation multi-tenant) et
son test n'a **jamais été exécuté** faute de base. Tant que ce n'est pas
fait, l'affirmation « les données d'un client ne fuient pas vers un autre »
repose sur une relecture, pas sur une preuve. C'est la première dette à
solder — pousser sur un dépôt distant suffit à faire tourner la CI.
