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
| 1 — CI (priorité absolue) | terminé | 0 | **Invariant 1 PROUVÉ 28/07/2026** : run 30223145565 verte, run 30223419721 ROUGE (branche jetable, politique users retirée, supprimée après lecture). Réserve : logs bruts inaccessibles (403 sans jeton). **Différentiel canonical.py : VERT en run n°4** — 12 014 encodages + 2 000 empreintes, zéro divergence ; seul défaut de premier passage : bloc `__main__` avant `_from_json` (NameError), corrigé avant exécution. Runs main n°1/3/4 vertes |
| 2 — Retirer la neutralisation (inv. 7) | terminé | 1 | Confinement seul : contenu externe VERBATIM (ni substitution, ni normalisation CRLF, ni marqueur injecté — le drapeau `truncated` suffit) ; champ `neutralized` supprimé ; modèle de menace v2 avec les 3 questions tranchées ; devoir d'affichage transféré à M6. Tour 1 : proptest verbatim ajouté (le corpus n'avait pas de `\r`), sabotage `\r`→`\n` attrapé par le test unitaire ET le proptest (contre-exemple minimal `"\r"`), bornes multi-octets et borne exacte testées |
| 3 — Argon2 : plafond 64 Mio + sémaphore | terminé | 1 | Plafond 65 536 KiB testé aux deux bords (64 Mio exact accepté+rehash, 128 Mio refusé) ; sémaphore Mutex+Condvar à 4 permis (pire cas 4×64 Mio = 256 Mio, courant ≈76 Mio), permis pris APRÈS les bornes (une chaîne forgée ne consomme pas de place) ; ADR-0006 amendée. **Tour 1 — vraie trouvaille : le test de porte passait SOUS SABOTAGE** (argon2 à 19 Mio > 300 ms sur cette machine, la lenteur masquait la porte cassée) → profil rapide (8 Mio, t=1) + 500 ms, ré-échoue sous sabotage, re-vert après |
| 4 — Bornes à deux étages | terminé | 1 | `Bound` à champs PRIVÉS validé à la construction : `hard(limite)` ou `two_tier(seuil, limite)` avec seuil ≤ limite exigé — le « souple sans plafond » n'est plus représentable. Trois zones testées (montant et destinataires, bornes exactes comprises), raisons nommant le niveau, proptest « au-delà de la limite dure = refus quel que soit requires_approval ». Chemins restés à un niveau (pas d'ordre → pas d'étages), justifié en doc ; au passage, fail-open du préfixe vide FERMÉ (violation de protocole + défense en profondeur dans path_is_under). Tour 1 : sabotage (limite dure → validation) attrapé par 6 tests |
| 5 — Version dans l'enveloppe d'état | terminé | 1 | `TaskStateEnvelope` (champs privés, `seal`/`into_state`, `TASK_STATE_FORMAT_VERSION = 1`) avec désérialisation VALIDANTE : la version est vérifiée AVANT d'interpréter l'état (intermédiaire `serde_json::Value` — serde_json passe en dépendance réelle de runtime, pur). Version inconnue → erreur nommant trouvée/supportée ; enveloppe sans version → refus ; test de reprise passe par l'enveloppe. Tour 1 : sabotage (contrôle débranché) → l'état futur est interprété avec le schéma courant et échoue en « missing field » trompeur — exactement le danger fermé, test le détecte |
| 6 — Cadrage preuve d'injectivité (doc) | terminé | 0 | §7 ajouté à encodage-canonique.md + fondation du modèle de menace : le round-trip Rust PROUVE l'injectivité (inverse à gauche, par construction) ; canonical.py prouve la NON-AMBIGUÏTÉ de la spec (deux lecteurs indépendants, mêmes octets — ce qu'un auditeur tiers exige), pas l'injectivité. Bloc documentaire, pas de tour de sabotage applicable |
| 7 — Méthode de travail : six corrections | terminé | 0 | Les 6 appliquées : palier sur (mandat, catégorie) écrit comme décision de schéma + backlog M4 ; palier 1 réécrit sur l'HISTORIQUE (une séance, comparaison au réellement-fait) ; abandon silencieux nommé, mesuré, traité en palier (expiration propre des actions, reconduction exigeant revue) ; cliquet tranché (rétrogradation JAMAIS automatique — signalée/proposée/décidée ; seule la suspension de sécurité est automatique et n'est pas une rétrogradation) ; promotion par déclaration de catégorie (le compteur ne fait que suggérer) ; questions : +11/12 (qui fait, que payez-vous), q7 reformulée (dernière erreur et son coût), annexe III resserrée sur la formulation CLAUDE.md, +13-16 depuis taches-delegables-analyse.md. Document, pas de sabotage applicable |
| 8 — Coutures (AuditEvent, policy réel) | terminé (sans code, conforme au brief) | 0 | Les deux exigent de modifier le graphe de dépendances gelé (runtime→audit, runtime→policy) = décision d'architecture : options recommandées écrites en détail dans questions-nuit.md, rien câblé |
| 9 — Réversibilité des migrations en CI | en cours | 1 | Job `reversibilite` (cluster vierge dédié, up→down→diff vierge→re-up→diff). **Run 10 : ROUGE au premier passage** — étape aller-retour ; jobs verifications et image verts. Cause diagnostiquée sans les logs (jeton requis) : comparaison d'ACL brute — nspacl NULL (jamais touché) vs matérialisé aux entrées par défaut après GRANT+REVOKE = même état de droits, texte différent. Correctif : comparaison de l'ACL EFFECTIVE (aclexplode+acldefault) + versions affichées + set -x pour rendre le prochain rouge lisible. Verdict attendu |
| 10 — Matrice invariant → test à jour | non commencé | 0 | |
| 11 — Approfondissement libre | non commencé | 0 | |

Suspens hérités du rapport du 28/07 : M0 non prouvé (→ bloc 1 en cours de
preuve) ; contrat d'URL du test d'isolation laissé ouvert le 27 (questions
n°8) ; canonical.py jamais exécuté ; .down.sql jamais appliqués (→ bloc 9) ;
retouche utilisateur de CLAUDE.md (fin de fichier) toujours non commitée,
laissée telle quelle.
