# ADR 0001 — Monolithe modulaire

Date : 2026-07-26 · Statut : accepté

## Contexte

Le produit est développé par une seule personne, à temps partiel. Il est vendu
à des PME industrielles sans DSI dédiée et s'exécute sur leur serveur : chaque
processus supplémentaire est un coût d'exploitation chez le client, pas chez
nous. Le calibre de référence est le démon `vibed` de VibeOS : un binaire
Rust/tokio, 77 tests, en production.

## Décision

Un seul binaire (`kollega`), découpé en dix crates aux frontières nettes,
reliées par un graphe de dépendances orienté et vérifié en intégration
continue :

`core` → `policy`, `audit`, `memory`, `tools`, `model` → `runtime` →
`store`, `api` → `cli`. Aucune flèche en sens inverse.

Le domaine (`kollega-core`) ne dépend d'aucune entrée-sortie (invariant 9) :
la frontière qu'un découpage en services ferait payer en réseau, on la fait
payer au compilateur, gratuitement.

## Conséquences

- Un artefact unique à livrer, sauvegarder, redémarrer chez le client.
- Les frontières entre crates sont des coutures : extraire un service plus
  tard reste possible si une contrainte réelle l'impose, sans réécriture du
  domaine.
- Toute la concurrence passe par tokio et PostgreSQL (`SKIP LOCKED`), pas par
  des processus.

## Alternatives écartées

- **Microservices** : suppose une équipe et une plateforme d'exploitation ;
  chez une PME sans DSI, chaque service est un point de panne de plus.
- **Modulith avec framework d'orchestration interne** : dépendance de plus,
  valeur nulle tant qu'il n'y a qu'un seul flux d'exécution.
- **Binaire unique sans crates** : perd les garanties de frontière à la
  compilation (invariants 5 et 9).
