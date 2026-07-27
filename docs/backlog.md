# Backlog — ce qu'on refuse de coder maintenant

Une fonctionnalité s'écrit quand trois clients payants l'ont demandée
(constitution v2). Avant ça, elle vit ici.

| Date | Idée | Origine | Compteur de demandes client payantes |
|---|---|---|---|
| 2026-07-26 | « Plateforme innovante / architecture pour se démarquer » — reformulé : toute généralisation au-delà du runtime gouverné V0 | Conversation de démarrage | 0 |
| ~~2026-07-26~~ | ~~Durcir le test du graphe de dépendances : vérifier le graphe résolu plutôt que le TOML brut~~ — **FAIT le 29/07** : la fermeture transitive est lue dans `Cargo.lock` (pas besoin d'invoquer `cargo metadata` depuis un test). Vérifié par sabotage : ajouter `tokio` au domaine fait apparaître **tokio, socket2 ET mio** — les deux derniers par transitivité, invisibles pour l'ancienne garde. | Revue M0 | fait |
| 2026-07-28 | Remplacer le modèle M4 « comptes rendus » par « relance client et suivi des impayés » (et re-prioriser les trois modèles) | `docs/taches-delegables-analyse.md` §6 — recommandation du propriétaire, décision produit à trancher par lui | s/o (décision produit, pas une demande client) |
| 2026-07-28 | Schéma M4 : le palier de confiance vit sur le couple (mandat, catégorie d'action), pas sur l'agent — la table des mandats porte un palier PAR catégorie | `docs/methode-de-travail.md` (bloc 7, correction 1) — décision de schéma gratuite aujourd'hui, coûteuse après la première migration de mandats | s/o (dette de conception) |
