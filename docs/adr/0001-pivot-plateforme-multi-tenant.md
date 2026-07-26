# ADR-0001 — Pivot vers une plateforme SaaS multi-tenant

**Statut :** Acceptée
**Date :** 26 juillet 2026
**Décideur :** Micka (fondateur)
**Remplace :** la décision « une instance = un client » de la constitution v1

---

## Contexte

La première constitution (v1) définissait Kollega comme un runtime d'agents déployé sur l'infrastructure de chaque client, une instance par client, sans studio ni self-service. Cette orientation venait d'un audit stratégique dont la logique était : éviter la concurrence frontale avec Dust, et s'appuyer sur la compétence de déploiement contraint du fondateur (VibeOS).

Le fondateur arbitre différemment, en connaissance de cette analyse :

- La cible est le **dirigeant de TPE/PME**, pas la DSI d'une ETI industrielle.
- La promesse est **« ajoutez un agent IA à votre entreprise en quelques minutes »**, en libre-service, après connexion de ses outils.
- Le déploiement souverain on-premise (VibeOS) reste au programme, mais comme **complément d'écosystème**, activé quand la base client le justifiera — pas comme produit initial.

Cet ADR acte ce choix et en tire les conséquences techniques, qui ne sont pas cosmétiques.

## Décision

**Kollega est une plateforme SaaS multi-tenant hébergée en France, sur laquelle une entreprise s'inscrit, connecte ses outils, et active des agents à partir de modèles prêts à l'emploi.**

Conséquences directes sur l'architecture :

1. **Multi-tenant dès la première migration.** L'isolation est appliquée par la base de données (Row-Level Security PostgreSQL), pas seulement par le code applicatif.
2. **Connecteurs en libre-service** via OAuth : Microsoft 365 et Google Workspace en premier.
3. **Modèles d'agents** plutôt que studio générique : c'est ce qui rend les « quelques minutes » réalisables.
4. **Crédits prépayés et quotas par organisation**, vérifiés avant chaque appel de modèle.
5. **Facturation intégrée** (abonnement + consommation).
6. **Le fondateur exploite une production.** Sauvegardes, supervision, disponibilité, incidents.

## Ce qui ne change pas

- Les types de `kollega-core` (validés le 26/07/2026) : réutilisables tels quels.
- Rust + tokio + axum + sqlx ; PostgreSQL 16 + pgvector comme moteur unique.
- MCP comme protocole d'outils.
- Le moteur de politiques, le journal d'audit chaîné, le plafond de coût, la séparation typée instruction/contenu externe, la validation humaine.
- Les interdits de l'annexe III de l'AI Act (tri de CV, évaluation ou décision sur des personnes, scoring d'individus). **Non négociable, indépendant de ce pivot.**

## Conséquences

### Positives

- Cycle de vente court : essai en libre-service, pas de projet d'installation.
- Marché beaucoup plus large : les TPE/PME françaises non-tech sont mal servies par les acteurs existants, qui visent les scale-ups technologiques.
- Un seul environnement de production à maintenir, au lieu d'une instance par client.
- Les mises à jour bénéficient à tous les clients immédiatement.
- La gouvernance (audit, plafonds, traçabilité) devient un argument de vente sur un segment où personne ne la met en avant.

### Négatives, assumées

- **L'isolation devient le risque numéro un.** Une fuite inter-clients est un événement dont une jeune société ne se relève pas. D'où la Row-Level Security en défense en profondeur, dès la première migration : la rétrofiter plus tard est une opération à haut risque.
- **Le coût d'exécution est avancé par nous.** En libre-service, un client peut consommer avant de payer. D'où les crédits prépayés, non négociables.
- **Le besoin de financement augmente.** Le budget établi le 26/07/2026 (scénario S1, ~12 500 € pour l'année 1) supposait aucune infrastructure de production. Une plateforme avec production, sauvegardes, supervision, facturation et API avancée se situe entre les scénarios S1 et S2. Le classeur doit être remis à jour avant tout dossier bancaire.
- **Responsabilité de sous-traitant RGPD pour tous les clients.** Contrat de sous-traitance, registre des sous-traitants ultérieurs, procédure de notification de violation : exigibles dès le premier client payant, pas « plus tard ».
- **Astreinte de fait.** Une plateforme qui tombe un mardi matin doit être relevée le mardi matin.
- **Concurrence assumée avec Dust** sur la catégorie, en se différenciant par le segment (TPE/PME non-tech) et par la gouvernance, pas par les fonctionnalités.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| Runtime on-premise par client (constitution v1) | Cycle de vente long, pas de libre-service, incompatible avec la promesse « en quelques minutes » |
| SaaS vertical mono-processus | Marché jugé trop étroit par le fondateur pour l'ambition visée |
| Studio générique sans modèles | Un formulaire vide ne produit pas un agent utile en cinq minutes ; les modèles sont le mécanisme réel de la promesse |
| Isolation applicative seule (sans RLS) | Une clause `WHERE` oubliée suffit à provoquer une fuite inter-clients. Coût de la RLS : faible aujourd'hui, très élevé après coup |

## Position de VibeOS dans l'écosystème

VibeOS n'est pas abandonné. Il devient l'**offre souveraine** de l'écosystème, activée plus tard :

- **Aujourd'hui :** aucun code spécifique. La seule exigence est que le produit reste **empaquetable en un artefact unique** — pas de dépendance à un service managé propriétaire, pas d'état hors PostgreSQL et du stockage objet.
- **Plus tard :** la même image, déployée sur un serveur client, avec attestation d'exécution (le journal d'audit référence le digest de l'image signée).
- **Ce qui rend l'ajout possible :** le trait `ModelProvider` (bascule vers un modèle local), l'absence d'état en mémoire, et l'artefact OCI unique déjà produit par la chaîne d'intégration.

**Règle :** ne rien introduire qui rende cet empaquetage impossible. Concrètement — pas de dépendance dure à un service cloud propriétaire pour une fonction du cœur.

## Suivi

- La constitution v1 est remplacée par la v2 (`CLAUDE.md`).
- Les jalons M0–M6 sont remplacés par M0–M7 (`docs/jalons.md`).
- Le classeur budgétaire doit être révisé avant toute démarche de financement.
