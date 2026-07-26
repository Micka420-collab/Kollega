# ADR-0002 — Row-Level Security dès la première migration

**Statut :** Acceptée
**Date :** 26 juillet 2026

---

## Contexte

[ADR-0001](0001-pivot-plateforme-multi-tenant.md) fait de Kollega une plateforme
SaaS multi-tenant. Une fuite de données entre deux clients est l'événement dont
une jeune société ne se relève pas : l'isolation est le risque numéro un du
produit. L'isolation purement applicative (une clause `WHERE org_id = …` dans
chaque requête) dépend de la discipline de chaque requête future — une seule
clause oubliée suffit.

## Décision

**L'isolation est appliquée par la base de données, dès la migration 0001, en
défense en profondeur :**

1. **Deux rôles PostgreSQL distincts.** `kollega_migrate` (propriétaire du
   schéma, applique les migrations) et `kollega_app` (rôle d'exécution, `LOGIN
   NOBYPASSRLS`, sans droit de modifier les politiques). L'application se
   connecte **uniquement** avec `kollega_app`.
2. **RLS activée ET forcée** (`ENABLE` + `FORCE ROW LEVEL SECURITY`) sur toute
   table portant `org_id` — `FORCE` soumet aussi le propriétaire de la table.
3. **Politique unique par table** : `tenant_isolation … USING (org_id =
   current_setting('app.current_org')::uuid)`. La forme sans `missing_ok` est
   volontaire : une requête exécutée hors contexte d'organisation **échoue**
   au lieu de retourner silencieusement des lignes — le système est fermé par
   défaut.
4. **Un point de passage unique dans le code** : `kollega_store::Db` garde le
   pool privé ; toute transaction s'ouvre par `Db::org_transaction(org_id)`,
   qui exécute `SET LOCAL app.current_org` avant toute requête. C'est ce point
   unique qu'on audite et qu'on teste.
5. **Un test d'isolation exécuté sur une base réelle** (invariant 1), écrit
   pour échouer si la RLS tombe : données de A et B insérées, le contexte de A
   ne voit que A ; la désactivation manuelle de la RLS fait apparaître la
   fuite, prouvant la sensibilité du test.

## Conséquences

- Toute nouvelle table portant `org_id` doit, dans la même migration, activer
  la RLS et déclarer sa politique — c'est dans la définition de terminé.
- Les migrations s'exécutent avec un rôle distinct de l'application ; le mot de
  passe de `kollega_app` est fourni par l'environnement, jamais dans une
  migration (pas de secret dans l'historique).
- Les requêtes internes hors tenant (santé, tâches d'exploitation) ne touchent
  aucune table portant `org_id` ; elles passent par des chemins dédiés et
  restreints de `Db`.
- Coût accepté : chaque requête paie l'évaluation de la politique. Négligeable
  devant le risque couvert.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| Isolation applicative seule (`WHERE org_id = …`) | Une clause oubliée = fuite inter-clients. Indétectable par relecture à mesure que le code grossit. |
| Une base de données par client | Reconstitue le coût d'exploitation « une instance par client » que le pivot supprime ; migrations et sauvegardes multipliées par le nombre de clients. |
| Un schéma PostgreSQL par client | Même dérive opérationnelle (N schémas à migrer), outillage sqlx inadapté, et la RLS couvre déjà le besoin. |
| RLS ajoutée « plus tard » | La rétrofiter exige de reprendre chaque table, chaque requête et chaque test sous charge de production. C'est l'opération la plus risquée d'un SaaS. |
