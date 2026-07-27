# État de session — mis à jour après chaque bloc

Session en cours : 28/07/2026, deuxième session de jour (suite directe).
Brief : LA TRANCHE VERTICALE D'ABORD — rien d'autre ne commence avant
qu'une tâche traverse réellement (créée → politique → exécutée → débitée →
journalisée dans PostgreSQL, attestation et contenu séparés → interrompue →
reprise depuis la base → même résultat).

Environnement : remote + CI opérationnels (runs 1-15, invariants 1 et 13
prouvés) ; PostgreSQL local ABSENT → toute l'intégration se prouve en CI,
itération par push. Pas de clé d'API dans l'environnement (à vérifier au
bloc 2).

Suspens hérité de la session précédente : rien d'interrompu ; bloc 11
partiel (gisement documenté : org_balance sérialisé, crédit vérifié après
plan — traités par la tranche) ; décisions propriétaire pendantes sur les
arêtes du graphe — LE PRÉSENT BRIEF ARBITRE : la tranche exige le câblage.

| Bloc | Statut | Tours | Note |
|---|---|---|---|
| 0 — Reprise | terminé | — | CI 15 verte intégrale, 16 (docs) en cours ; graine proptest-regressions à versionner |
| 1 — La tranche traverse | **TERMINÉ — run n°17 verte du premier coup** (verifications + reversibilite + image) | 0 | Migration 0003 + pilote `driver.rs` + test `vertical_slice.rs` (8 étapes : suspension→interruption→reprise→même résultat, fourche 23505, ajout seul par GRANT testé, isolation témoin, purge RGPD avec chaîne intacte). Trouvailles d'intégration accumulées : TRUNCATE→CASCADE (FK 0003), garde SET v1 morte au premier UPDATE…SET (prédite — refondue en liste d'interdits), sqlx sans feature json → casts `::jsonb`, verrou credits FOR UPDATE sérialise les pas d'une même org (le réessai de fourche devient défense en profondeur), le rejeu de pas ré-appellerait un vrai modèle (dette idempotence rendue concrète), graphe déjà prêt (store rang UPPER) |
| 2 — Coût réel / ModelProvider d'échec | terminé (voie sans clé) | 0 | **BLOCAGE (une ligne) : aucune clé d'API dans l'environnement — aucun appel réel, aucun coût mesuré, pas d'economie-unitaire.md inventé.** Voie prescrite appliquée : contrat réel `kollega-model::ModelProvider` (faillible, facturé en jetons réels) + `ScriptedProvider` rejouant les 4 modes d'échec (limite de débit, délai à effet inconnu, réponse tronquée FACTURÉE, facture ≠ estimation) + `ApiKey` à Debug/Display expurgés, test de non-fuite (formatage, Debug dérivé d'une config, erreurs) |
| 3 — Types porteurs (a-f) | **a✓ b✓ c✓ d✓ e✓ f✓ — complet** (nuit du 28 au 29/07 : c et f achevés sur autorisation de Micka, chacun vérifié par sabotage) | 1 |
| 3 (détail v1, conservé pour l'historique) | — | 0 | a : ContentDigest::of seul constructeur + from_storage sous feature + garde textuelle d'atteignabilité. b : Timestamp tronque à la construction (euclidien, saturé), pilote câblé dessus. c : AuditContent avec digest-MÉTHODE fait ; la refonte de ChainedEntry (hash privé) NON faite. d : Intent/Outcome/Abandoned + branché sur le rejeu réel (attestation step_abandoned avant rejeu). e : validateur asymétrique à rapport, table du brief testée ligne à ligne. f : traits append/read et put/read/purge_org + garde anti-retrait ; conformance LITTÉRALE du pilote aux traits à câbler |
| 6 — Approfondissement par sabotage | en cours | 3 | Sabotages VÉRIFIÉS : (a) champs de `ChainedEntry` rendus publics → le doctest `compile_fail` échoue avec « Test compiled successfully, but it's marked compile_fail » ; (b) `DELETE FROM audit_chain` introduit → la garde SQL de la persistance rougit ; (c) filtre de tâche retiré du chargement des effets → CI ROUGE (run 30230062721, `verifications` en échec, branche jetable supprimée) — le test de fuite d'effets sait donc échouer. (d) en cours : différentiel Rust↔Python, saboté CÔTÉ PYTHON (hex majuscule) pour isoler — aucun test Rust ne doit bouger, seul le différentiel doit rougir |
| 4 — Documents | terminé | 0 | ADR-0007 (invariants→types→dépôts→SQL→migration, avec la réserve : le schéma porte en propre ce que les types ne peuvent pas exprimer) ; matrice : rubrique « où il vit » (13 lignes, 3 invariants en prose seulement rendus visibles) ; README : hébergement français et RGPD reformulés en décisions prises NON réalisées (ADR-0001, M7) |
