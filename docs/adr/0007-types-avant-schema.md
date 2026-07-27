# ADR-0007 — Le sens de dépendance : invariants → types → dépôts → SQL → migration

**Statut :** Acceptée (session autonome du 28/07/2026, bloc 4)
**Date :** 28 juillet 2026

## Contexte

La tranche verticale a cousu la machine pure à PostgreSQL. À chaque couture,
la même question : qui décide de la forme — le type ou la table ? La session
a tranché cas par cas (l'enveloppe versionnée impose sa forme au JSONB, la
PK `(org_id, height)` porte l'anti-fourche, les GRANT portent l'ajout seul) ;
cet ADR fixe la règle générale pour ne plus la retrancher à chaque fois.

## Décision

**Le sens de dépendance est : invariants → types → dépôts → SQL →
migration. Jamais l'inverse.** On écrit d'abord ce que le système promet
(l'invariant), puis le type qui rend sa violation inexprimable, puis la
forme du dépôt qui porte le cycle de vie (`append`/`read` sans retrait,
`purge_org` nommé), puis le SQL qui persiste, puis la migration qui le
crée. Une table n'impose jamais sa forme à un type ; un type ne se déforme
jamais pour épouser une colonne.

**La réserve sans laquelle la règle est fausse : certains invariants ne
peuvent pas vivre dans les types.** L'invariant 1 en est l'exemple —
`org_id` est dans le type, mais la garantie d'isolation vient de la
politique RLS : aucun type Rust ne peut empêcher une requête SQL de voir
une ligne. De même l'anti-fourche (unicité `(org_id, height)`) et l'ajout
seul (GRANT sans UPDATE/DELETE) vivent dans le schéma, pas dans les types.

Formulation exacte : **le schéma ne contredit jamais les types, et il porte
en propre les invariants que les types ne peuvent pas exprimer.**

## Conséquences

- Toute nouvelle table commence par la question : « quel invariant, quel
  type, quel dépôt ? » — le CREATE TABLE vient en dernier.
- La matrice des invariants porte une rubrique « où il vit » (type /
  contrainte de schéma / RLS / test / prose seulement) : un invariant en
  « prose seulement » est un invariant qu'aucun mécanisme n'applique, et
  cette rubrique existe pour le rendre visible.
- Les gardes textuelles (graphe de dépendances, frontière de stockage,
  forme des dépôts, contexte SQL) défendent le sens de dépendance là où le
  compilateur ne le peut pas.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| Schéma d'abord (« database-first ») | La forme des tables finirait par dicter les types ; les invariants deviendraient des commentaires. |
| Types seuls (« le compilateur suffit ») | Faux : RLS, unicité, GRANT — les garanties les plus dures du produit vivent dans PostgreSQL. La règle sans la réserve serait une croyance. |
