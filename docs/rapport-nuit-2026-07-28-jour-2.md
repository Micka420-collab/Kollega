# Rapport de session autonome — 28/07/2026, deuxième session de jour

Suspens hérité (une ligne) : rien d'interrompu ; restaient les décisions
propriétaire sur les coutures — le brief de cette session les a arbitrées
en ordonnant la tranche.

## 1. La tranche traverse-t-elle ?

**OUI, de bout en bout, run CI n°17, verte du premier coup.** Une tâche est
créée, soumise au vrai moteur de politiques, suspendue en validation,
INTERROMPUE (connexions fermées), reprise depuis la base, exécutée, débitée
du crédit, journalisée dans PostgreSQL — attestations (empreintes) dans
`audit_chain`, contenus dans `audit_content`, séparés — et terminée avec le
même résultat que le parcours direct. La chaîne se vérifie de bout en bout
depuis les colonnes. Elle ne bloque nulle part.

## 2. Ce que l'intégration a révélé que les tests purs ne voyaient pas

- **`TRUNCATE users, organizations` du test RLS est mort au moment où la
  migration 0003 a posé ses clés étrangères** — passé en `CASCADE`. Un test
  d'intégration voisin est un consommateur de schéma comme un autre.
- **La garde textuelle « SET sans LOCAL » est morte au premier
  `UPDATE … SET` légitime** — prédit par la revue, advenu à la lettre.
  Refondue en liste d'interdits ciblant la menace réelle (GUC pointées,
  `SESSION`, `ROLE`, `search_path`), avec ses propres tests.
- **sqlx sans la feature `json` ne lie pas `serde_json::Value`** : liaisons
  TEXT + casts `::jsonb` dans le SQL. Invisible depuis le pur.
- **Le verrou `FOR UPDATE` sur la ligne de crédits sérialise TOUS les pas
  d'une même organisation** : la fourche de chaîne ne peut pas se produire
  par le pilote — le réessai d'unicité est une défense en profondeur
  derrière un verrou qui fait déjà le travail. Découvert en écrivant le
  test de concurrence, prouvé par sonde manuelle (23505 obtenu).
- **Le rejeu de pas après conflit ré-appellerait un vrai modèle et
  ré-exécuterait un vrai outil** : la dette d'idempotence de
  `credits-concurrence.md` n'est plus théorique — elle a une ligne de code
  où s'écrire. En attendant, le pas rejoué est attesté `step_abandoned`.
- **La sonde de fourche a aussi prouvé un point non prévu** : une entrée
  forgée insérée au bon endroit fait ÉCHOUER `verify` (hachage faux), et le
  rôle applicatif ne peut PAS la retirer (GRANT sans DELETE) — l'ajout seul
  et la détection se sont testés l'un l'autre.
- Cargo exécute les binaires de test SÉQUENTIELLEMENT — deux tests
  d'intégration partageant la base ne se marchent pas dessus, mais l'ordre
  est arbitraire : chaque test doit nettoyer et semer ses propres données.

## 3. Les quatre points de vigilance

- **Troncature à la microseconde : ÉVITÉE PAR SCHÉMA** — l'horodatage est
  stocké en BIGINT microsecondes, jamais en `timestamptz` ; depuis le
  bloc 3b, le type `Timestamp` tronque à la construction : l'écart est
  inexprimable. Le piège était réel, il n'a pas eu lieu.
- **Fourche de chaîne : CONFIRMÉE ET FERMÉE** — `PRIMARY KEY (org_id,
  height)` + réessai ; violation 23505 provoquée et observée en CI. Nuance
  honnête : par le pilote, le verrou de crédits sérialise avant.
- **Clé du contenu sur `(org_id, digest)` : APPLIQUÉE PAR SCHÉMA** (PK
  composite) — jamais l'empreinte seule.
- **Purge puis vérification : PROUVÉE** — purge RGPD du contenu, chaîne
  d'attestations toujours verte, seule la trace de purge demeure en
  contenu.

## 4. Le coût réel

**Aucune clé d'API dans l'environnement — aucun appel réel, aucun chiffre
mesuré, aucune extrapolation inventée** (des chiffres extrapolés d'une
mesure qui n'a pas eu lieu seraient de la fausse donnée ; ils serviraient
un PRIX). Voie prescrite appliquée : contrat `kollega-model` réel
(faillible, facturé, `is_external`), `ScriptedProvider` rejouant limite de
débit, délai à effet inconnu, réponse tronquée FACTURÉE, facture ≠
estimation ; `ApiKey` expurgée par le type, non-fuite testée. Les trois
extrapolations (30/90/200 relances) attendent UNE mesure réelle — c'est un
appel unique sur le modèle le moins cher, dès qu'une clé existe.

## 5. Blocs 3 et 4

Bloc 3 : **a fait** (ContentDigest, frontière `from_storage` + garde),
**b fait** (Timestamp), **c partiel** (AuditContent à empreinte-méthode
fait ; refonte de ChainedEntry — hash privé, construction vérifiée — NON
faite), **d fait** (trois variantes + Abandoned branché sur le rejeu
réel), **e fait** (validateur asymétrique à rapport, table testée ligne à
ligne), **f partiel** (traits + garde anti-retrait ; la conformance
littérale du pilote aux traits reste à câbler). Bloc 4 : **fait** (ADR-0007
avec sa réserve, rubrique « où il vit » — trois invariants en prose
seulement rendus visibles —, README débarrassé de ses deux affirmations
fausses).

## 6. Décisions prises seul — à réexaminer

- Le pilote vit dans `kollega-store` (le graphe le permettait déjà : rang
  UPPER) — les coutures se cousent au-dessus du point de passage unique.
- `Budget::refreshed` ajouté au runtime (anti-solde-périmé).
- Frontière `from_storage` par feature + garde textuelle (l'unification
  des features rend la feature seule insuffisante — dit en face).
- La garde SET v2 : liste d'interdits au lieu du bannissement générique.
- La purge RGPD accorde DELETE à `kollega_app` sur le SEUL contenu.
- `rls_isolation` passé à `TRUNCATE … CASCADE`.

## 7. Inquiétudes

- **L'idempotence du rejeu est maintenant la dette n°1 du chemin réel** :
  avec un vrai modèle et un vrai outil, rejouer un pas = double appel,
  double envoi possible. `step_abandoned` atteste, il n'empêche pas.
  À régler AVANT le premier outil à effet externe (clé d'idempotence par
  tool_call_id, réservation avant exécution).
- La machine n'a toujours pas de `ToolCallId` : les `AuditRecord` du
  bloc 3d sont prêts, mais le pont machine→records attend que la machine
  identifie ses appels. C'est la prochaine couture naturelle.
- `verify_org_chain` recharge toute la chaîne en mémoire — très bien pour
  la tranche, à repenser en flux avant des chaînes longues (déjà consigné
  côté kollega-audit).
- La CI reste sur des références flottantes (actions sans SHA, images sans
  digest, pas de `--locked`) — inchangé, toujours vrai, toujours pas fait.
