# ADR-0003 — PostgreSQL seul moteur

Date : 2026-07-26 · Statut : accepté

> **Relu contre le code le 29/07/2026 — quatre corrections.** Le titre
> annonçait « ADR 0002 » : le fichier avait été renuméroté au pivot sans
> que son en-tête suive. Le journal était nommé `audit_log`, table qui
> n'a jamais existé (voir « Décision »). La réversibilité des migrations
> était rattachée à l'invariant 12, alors que c'est le **13** — le 12
> interdit la suppression physique. Et le cadrage ci-dessous suppose une
> instance chez le client : c'est la prémisse d'AVANT le pivot, que
> [ADR-0001](0001-pivot-plateforme-multi-tenant.md) remplace par une
> plateforme multi-tenant hébergée. La décision technique, elle, tient
> sans changement — un seul moteur reste un seul moteur à sauvegarder,
> superviser et restaurer, que ce soit chez le client ou chez nous.

## Contexte

Le produit a besoin : d'une base transactionnelle, d'une file d'attente de
tâches, d'un journal d'audit en ajout seul, d'une recherche vectorielle
(embeddings) et d'une recherche plein texte. *(Cadrage d'origine, conservé
tel quel :)* l'instance s'exécute chez le client — chaque moteur
supplémentaire est un composant que son informaticien devra sauvegarder,
superviser et restaurer.

## Décision

PostgreSQL 16 + pgvector est le **seul** moteur de données.

- File d'attente : `SELECT … FOR UPDATE SKIP LOCKED` sur la table `tasks`.
- Journal d'audit : table en ajout seul, chaînée par hachage (`prev_hash`),
  sans bus d'événements. **Réalisé en DEUX tables** (`audit_chain` pour les
  attestations, `audit_content` pour les contenus, purgeable au titre du
  RGPD) — l'ADR d'origine écrivait `audit_log`, nom qui n'a jamais existé.
- Vecteurs : `pgvector` avec index HNSW ; plein texte : `tsvector`.
- Sauvegarde/restauration : un seul système à couvrir (jalon M6).

## Conséquences

- `podman compose up` démarre deux conteneurs : l'application et la base.
  Rien d'autre à exploiter chez le client.
- Les migrations SQL numérotées et réversibles sont l'unique mécanisme
  d'évolution du schéma (invariant **13** ; l'ADR d'origine citait le 12,
  qui interdit la suppression physique).
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
