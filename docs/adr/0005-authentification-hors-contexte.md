# ADR-0005 — Authentification hors contexte d'organisation

**Statut :** Acceptée (rédigée en session autonome du 27/07/2026 ; la
migration correspondante n'est PAS écrite — elle ne serait pas vérifiable
sans base, voir `docs/rapport-nuit-2026-07-27.md`)
**Date :** 27 juillet 2026

---

## Problème

À la connexion, l'utilisateur présente un email et un mot de passe. À cet
instant, l'application ne connaît pas encore son organisation — c'est
précisément ce qu'elle cherche à établir. Or la politique RLS du dépôt est
**fermée par défaut** : sans `app.current_org` posé, toute requête sur une
table tenant (`users` comprise) **échoue**. La connexion est donc impossible
par le chemin normal, et c'est voulu : il faut une sortie explicite, décidée
ici, plutôt qu'un affaiblissement discret du dispositif.

Deux sorties « évidentes » sont des pièges, et sont **INTERDITES** :

1. **Accorder `BYPASSRLS` à `kollega_app`.** Interdit. Le rôle applicatif
   passerait outre TOUTES les politiques, en permanence, pour toutes les
   requêtes : l'invariant 1 deviendrait une déclaration d'intention. C'est
   inscrit dans la constitution (« le rôle applicatif n'a pas BYPASSRLS »)
   et vérifié par le test d'isolation (témoin `rolbypassrls = false`).
2. **Élargir la politique avec
   `OR current_setting('app.current_org', true) IS NULL`.** Interdit. Toute
   requête émise hors contexte — un oubli, un chemin de code neuf, une fuite
   du point de passage unique — verrait alors TOUTES les lignes de TOUS les
   tenants. L'oubli deviendrait silencieusement une fuite globale, exactement
   ce que la forme fermée (erreur bruyante) existe pour empêcher.

## Décision — option (a) : table `login_identities` hors périmètre tenant

Une table dédiée à la seule résolution de connexion, **délibérément hors
RLS**, sans aucune donnée métier :

- Colonnes prévues : `email` (normalisé, unique), `user_id`, `org_id`,
  `password_hash` (chaîne PHC argon2id), plus l'horodatage de création et
  l'effacement logique. Rien d'autre : pas de nom, pas de rôle, pas de
  contenu client.
- `GRANT` minimal pour `kollega_app` : `SELECT` et l'écriture strictement
  nécessaire (création de compte, changement de mot de passe). Pas de
  lecture élargie, pas de jointure vers les tables métier.
- Flux de connexion : chercher `email` dans `login_identities` → vérifier le
  mot de passe (argon2id, `kollega-api::auth`) → si valide, ouvrir la session
  et poser `app.current_org = org_id` par le point de passage unique — le
  reste de la requête vit sous RLS comme tout le monde.
- La table est inscrite dans la liste blanche du filet structurel RLS
  (`crates/kollega-store/tests/rls_structural.rs`, constante
  `ALLOWED_WITHOUT_RLS`) **au moment où sa migration sera écrite**, avec
  référence à ce document. La liste est vide tant que la table n'existe pas.
- L'unicité d'`email` y est **globale** (contrairement à `users`, unique par
  organisation) : c'est la clé de résolution. Conséquence assumée : un même
  email ne peut appartenir qu'à une organisation dans la V1 — le
  rattachement multi-organisations sera un ADR distinct s'il devient une
  demande client réelle.

### Pourquoi c'est acceptable

Le périmètre exposé hors RLS est minuscule, énuméré, et sans donnée métier :
une correspondance identifiant → organisation, plus une empreinte de mot de
passe conçue pour être stockée. Le risque résiduel (énumération de comptes
par un attaquant ayant déjà accès SQL via `kollega_app`) est sans commune
mesure avec les deux alternatives interdites, et la surface est auditée par
un filet automatique qui refuse toute table hors RLS non justifiée.

## Option (b) écartée : fonction `SECURITY DEFINER`

Une fonction PostgreSQL `SECURITY DEFINER` (exécutée avec les droits de son
propriétaire) pourrait interroger `users` sous RLS et retourner la
correspondance. Écartée :

- **Plus difficile à auditer.** Le contournement vit dans du code SQL côté
  serveur, avec les droits du propriétaire du schéma ; son périmètre réel
  dépend du `search_path`, du propriétaire, et de chaque révision de la
  fonction. Une table nue aux colonnes énumérées se lit en une ligne de
  catalogue ; une fonction s'audite ligne à ligne, à chaque changement.
- **Contourne la RLS de l'intérieur.** Elle crée un précédent : « quand la
  RLS gêne, on écrit une fonction DEFINER ». La liste blanche de tables hors
  RLS est un mécanisme fermé et vérifiable ; les fonctions DEFINER sont une
  porte qu'on rouvre à chaque besoin.
- Les pièges connus de `SECURITY DEFINER` (search_path non épinglé,
  élévation involontaire) ajoutent une classe de vulnérabilités que le
  projet n'a pas besoin de posséder.

## Conséquences

- La migration `login_identities` (avec son `GRANT` minimal et son entrée
  justifiée dans la liste blanche) sera écrite au prochain jalon disposant
  d'une base pour la vérifier — pas avant.
- Le filet structurel RLS reste la seule voie d'exception : toute table hors
  RLS doit y être listée avec référence d'ADR, sinon la CI échoue.
- Les deux interdits (BYPASSRLS applicatif, politique élargie au contexte
  nul) sont permanents ; les lever exigerait un nouvel ADR qui remplace
  explicitement celui-ci.
