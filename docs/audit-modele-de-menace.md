# Modèle de menace — journal d'audit chaîné

Version 1 — 28/07/2026. Document destiné à être lu par un auditeur : il dit
ce que le dispositif garantit, contre qui, et ce qu'il NE garantit PAS.
Spécification technique : `docs/encodage-canonique.md`.

## Le dispositif

Fondation : l'encodage des octets hachés est **injectif** — prouvé par
round-trip (un décodeur indépendant rend l'original : inverse à gauche,
donc injectivité par construction) — et la spécification est **non
ambiguë** — prouvée par le différentiel Rust ↔ Python en CI (deux lecteurs
indépendants, mêmes octets, mêmes empreintes). La première propriété
interdit deux enregistrements distincts aux mêmes octets ; la seconde
permet à un auditeur tiers de vérifier la chaîne depuis la spécification
seule, sans notre code. Cadrage détaillé : `docs/encodage-canonique.md` §7.

Trois niveaux, chacun prouvé par des tests purs
(`crates/kollega-audit`) :

1. **Cohérence interne** (`verify`) : chaque entrée porte sa hauteur, le
   lien vers la précédente et son empreinte SHA-256 (qui couvre action,
   acteur, hauteur, organisation, charge utile et horodatage).
2. **Ancrage** (`verify_with_anchor`) : une ancre — (organisation, hauteur,
   empreinte de tête, horodatage) — publiée hors de la base.
3. **Publication** (`AnchorPublisher`, contrat en ajout seul, hauteur jamais
   en régression) : deux témoins prévus — un stockage objet en écriture
   seule avec rétention, ET la remise quotidienne au dirigeant de
   l'empreinte de tête de son journal. Le client est le témoin qu'on ne
   peut pas rétracter — c'est un argument de vente, et une contrainte qu'on
   s'impose. (Défaut retenu en session autonome, révocable tant que rien
   n'est en production.)

## Qui peut altérer quoi

| Attaquant | Capacité | Détection |
|---|---|---|
| Agent/outil via l'application | Écrit par le point de passage unique, sous RLS ; ne peut pas modifier une entrée existante (aucune API d'update/delete) | Sans objet : ajout seul |
| Rôle applicatif compromis (`kollega_app`) | INSERT dans sa propre organisation ; pas de DELETE/UPDATE si les GRANT sont posés comme prévu au jalon de persistance | Entrées mensongères possibles MAIS attribuées et chaînées ; réécriture impossible |
| Accès en écriture directe à la base (admin, vol de `kollega_migrate`, injection) | Altérer, supprimer, réordonner, tronquer, ou réécrire un suffixe entier en recalculant les hachages | Altération/réordonnancement/suppression au milieu/troncature de tête : `verify`. Troncature de queue et suffixe réécrit : `verify_with_anchor` UNIQUEMENT, jusqu'à la hauteur de la dernière ancre |
| Le même, AVEC contrôle du stockage d'ancres | Réécrire chaîne ET ancres du stockage objet | La remise au client : l'empreinte qu'il détient ne se rétracte pas. C'est pour cette raison qu'il y a DEUX témoins |
| Le même, AVEC la complicité du client (ou client seul) | Rien : le client ne détient que des empreintes, pas le pouvoir d'écrire la chaîne | Une organisation ne peut ni invalider ni s'approprier la chaîne d'une autre (organisation dans les octets hachés, tests dédiés) |

## Ce qui reste possible — dit sans détour

- **La fenêtre d'ancrage.** Ce qui est écrit APRÈS la dernière ancre publiée
  n'est protégé que par la cohérence interne : un attaquant en écriture peut
  réécrire ce suffixe-là sans détection jusqu'à la publication suivante. Le
  test `rewrite_after_anchor_is_invisible_until_next_anchor` le démontre au
  lieu de le cacher. Réduire la fenêtre = publier plus souvent ; le rythme
  (quotidien par défaut) est un paramètre d'exploitation.
- **L'oracle du contenu.** La chaîne prouve l'intégrité, pas la véracité :
  un composant compromis peut journaliser des mensonges bien formés. La
  chaîne garantit qu'on ne pourra pas les faire disparaître ensuite.
- **La destruction totale.** Un attaquant peut toujours détruire la base.
  La chaîne ne l'empêche pas ; elle rend la destruction ÉVIDENTE (les
  ancres témoignent qu'un journal existait, et jusqu'où).
- **Pas de signature.** Le dispositif est un hachage chaîné ancré, pas une
  signature cryptographique : il n'authentifie pas l'auteur d'une entrée
  au-delà du champ `actor` journalisé. Une signature par organisation est
  une évolution possible, non requise par le modèle actuel (l'ancrage
  externe couvre le risque de réécriture).

## Ce que `audit verify` devra faire (jalon de persistance)

`verify` seul ne suffit pas à l'invariant 4 tel que formulé (« détecte toute
altération ») : la commande d'exploitation devra TOUJOURS confronter la
chaîne à la dernière ancre (`verify_with_anchor`), et remonter l'âge de
cette ancre (une ancre vieille de dix jours = dix jours de fenêtre). Une
proposition de reformulation de l'invariant est dans
`docs/claude-md-corrections-proposees.md`.
