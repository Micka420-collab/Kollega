# ADR 0002 — PostgreSQL seul moteur

Date : 2026-07-26 · Statut : accepté

## Contexte

Le produit a besoin : d'une base transactionnelle, d'une file d'attente de
tâches, d'un journal d'audit en ajout seul, d'une recherche vectorielle
(embeddings) et d'une recherche plein texte. L'instance s'exécute chez le
client : chaque moteur supplémentaire est un composant que son informaticien
devra sauvegarder, superviser et restaurer.

## Décision

PostgreSQL 16 + pgvector est le **seul** moteur de données.

- File d'attente : `SELECT … FOR UPDATE SKIP LOCKED` sur la table `tasks`.
- Journal d'audit : table `audit_log` en ajout seul, chaînée par hachage
  (`prev_hash`), sans bus d'événements.
- Vecteurs : `pgvector` avec index HNSW ; plein texte : `tsvector`.
- Sauvegarde/restauration : un seul système à couvrir (jalon M6).

## Conséquences

- `podman compose up` démarre deux conteneurs : l'application et la base.
  Rien d'autre à exploiter chez le client.
- Les migrations SQL numérotées et réversibles sont l'unique mécanisme
  d'évolution du schéma (invariant 12).
- Si un jour la charge vectorielle dépasse réellement pgvector, la décision
  sera rouverte par un ADR, avec des mesures.

## Alternatives écartées

- **Redis / RabbitMQ / NATS pour la file** : un moteur de plus à exploiter
  pour un besoin que `SKIP LOCKED` couvre à notre échelle (interdit explicite
  de la constitution sans demande client payée deux fois).
- **Base vectorielle dédiée (Qdrant, Weaviate…)** : double la surface de
  sauvegarde et de panne pour des corpus de PME (< 10⁶ fragments).
- **SQLite** : pas de `SKIP LOCKED` ni de pgvector équivalents, et la
  concurrence d'écriture de la boucle d'agent le disqualifie.
