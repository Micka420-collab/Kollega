# État de session — mis à jour après chaque bloc

Session en cours : nuit du 27 au 28/07/2026. Base PostgreSQL : ABSENTE
(port 5432 fermé, pas de psql, pas d'URL) → surface pure uniquement.
Python : ABSENT → test différentiel du BLOC 7 non exécutable cette nuit.
Remote : absent → commits locaux.

| Bloc | Statut | Tours d'approfondissement | Note |
|---|---|---|---|
| 0 — Reprise | terminé | — | Premier passage du prompt permanent ; fichier créé |
| 1 — Prouver M0 | abandonné (pas de base) | 0 | Reste LE jalon non prouvé ; exige PostgreSQL ou remote+CI |
| 2 — Retirer l'épinglage argon2 | non commencé | 0 | |
| 3 — Limite dure / seuil souple | non commencé | 0 | |
| 4 — Numéro de séquence haché | non commencé | 0 | |
| 5 — Ancre de chaîne, pur | non commencé | 0 | |
| 6 — Assemblage des segments (inv. 7) | non commencé | 0 | |
| 7 — Injectivité de l'encodeur | non commencé | 0 | Python absent : différentiel non exécutable |
| 8 — Propriétés surface pure | non commencé | 0 | |
| 9 — Plafond et crédit, noyau pur | non commencé | 0 | |
| 10 — Boucle d'agent, machine à états | non commencé | 0 | Seulement si 2-9 approfondis |
| 11 — Matrice invariant → test | non commencé | 0 | |
| 12 — Méthode de travail (document) | non commencé | 0 | |
| 13 — Corrections CLAUDE.md proposées | non commencé | 0 | |

Laissé en suspens par la session précédente (rapport du 27/07) : M0 non
prouvé (rls_isolation jamais exécuté), CI jamais exécutée, migration
login_identities volontairement non écrite (ADR-0005), question du contrat CI
(questions-nuit n°8), ancrage de la chaîne d'audit, deux corrections de
CLAUDE.md à proposer. CLAUDE.md porte une retouche utilisateur non commitée
(fin de fichier) : laissée telle quelle.
