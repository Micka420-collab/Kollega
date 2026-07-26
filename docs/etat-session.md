# État de session — mis à jour après chaque bloc

Session en cours : 28/07/2026 (reprise après la nuit du 27 au 28).
Environnement : **remote GitHub actif** (origin = Micka420-collab/Kollega,
poussé, CI opérationnelle) ; PostgreSQL local ABSENT (la base réelle n'existe
qu'en CI) ; Python local ABSENT (présent sur le runner CI).

Faits nouveaux depuis le dernier rapport : README.md créé et poussé
(commit d582a6c) ; Cargo.lock resynchronisé (3ea9d8b) ; **CI run n°1 VERTE**
(run 30223145565, jobs verifications + image, image signée et SBOM publiés).
Sensibilité du test RLS non encore prouvée par exécution → en cours.

| Bloc | Statut | Tours d'approfondissement | Note |
|---|---|---|---|
| 0 — Reprise | terminé | — | Aucun bloc INTERROMPU EN COURS hérité ; suspens du 28/07 repris ci-dessous |
| 1 — CI (priorité absolue) | en cours | 0 | **Invariant 1 PROUVÉ 28/07/2026** : run 30223145565 verte, run 30223419721 ROUGE (branche jetable `ci-sensibilite-rls`, politique users retirée, supprimée après lecture). Réserve : logs bruts inaccessibles (403 sans jeton) — impossible de distinguer lequel des deux tests RLS a rougi. Consigné dans matrice + README + en-têtes. Reste du bloc : test différentiel canonical.py (le runner a Python) |
| 2 — Retirer la neutralisation (inv. 7) | non commencé | 0 | |
| 3 — Argon2 : plafond 64 Mio + sémaphore | non commencé | 0 | |
| 4 — Bornes à deux étages | non commencé | 0 | |
| 5 — Version dans l'enveloppe d'état | non commencé | 0 | |
| 6 — Cadrage preuve d'injectivité (doc) | non commencé | 0 | |
| 7 — Méthode de travail : six corrections | non commencé | 0 | |
| 8 — Coutures (AuditEvent, policy réel) | non commencé | 0 | |
| 9 — Réversibilité des migrations en CI | non commencé | 0 | |
| 10 — Matrice invariant → test à jour | non commencé | 0 | |
| 11 — Approfondissement libre | non commencé | 0 | |

Suspens hérités du rapport du 28/07 : M0 non prouvé (→ bloc 1 en cours de
preuve) ; contrat d'URL du test d'isolation laissé ouvert le 27 (questions
n°8) ; canonical.py jamais exécuté ; .down.sql jamais appliqués (→ bloc 9) ;
retouche utilisateur de CLAUDE.md (fin de fichier) toujours non commitée,
laissée telle quelle.
