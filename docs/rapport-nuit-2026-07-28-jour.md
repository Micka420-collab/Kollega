# Rapport de session autonome — 28/07/2026, session de jour

> Nom de fichier : `rapport-nuit-2026-07-28.md` était pris par la session
> précédente (nuit du 27 au 28) ; celui-ci porte le suffixe `-jour`.

Environnement : **remote GitHub actif pour la première fois**
(github.com/Micka420-collab/Kollega), CI opérationnelle avec PostgreSQL
réel ; toujours pas de PostgreSQL ni de Python locaux. 15 commits cette
session, tous poussés. `cargo fmt` / `clippy -D warnings` /
`cargo test --workspace` verts avant chaque commit (155 tests en fin de
session, contre 142 au début).

## 2 d'abord, comme exigé — l'invariant 1

**PROUVÉ, le 28/07/2026.** Run CI n°1 (30223145565) VERTE avec la politique
en place ; run n°2 (30223419721) ROUGE sur branche jetable
`ci-sensibilite-rls` avec la politique `tenant_isolation` de `users`
volontairement retirée — échec exactement à l'étape des tests, fmt/clippy
verts, branche supprimée après lecture. Le test sait échouer, donc il
prouve. Réserve honnête : les journaux bruts de CI exigent un jeton
(403) — impossible de distinguer LEQUEL des deux tests RLS a produit le
rouge (les deux détectent la politique manquante). La partie « y compris en
recherche vectorielle » de l'invariant reste sans objet : aucune table
vectorielle n'existe (M5).

## 1. Ce que la session précédente laissait en suspens — et ce qui en a été fait

- **M0 non prouvé** (la dette n°1 des deux rapports précédents) →
  **PROUVÉ** (ci-dessus). Le README, la matrice et les en-têtes
  « NON VÉRIFIÉ » de `ci.yml` et `rls_structural.rs` sont à jour.
- **`canonical.py` jamais exécuté** → **différentiel VERT en CI (run n°4)** :
  12 014 encodages + 2 000 empreintes complètes comparés octet à octet à
  l'implémentation Rust, zéro divergence. La spécification est confirmée
  par deux lecteurs indépendants. Seul défaut de premier passage : le bloc
  `__main__` précédait `_from_json` (NameError garanti) — corrigé AVANT
  exécution, ce n'était pas une divergence de spécification.
- **`.down.sql` jamais joués** → job CI `reversibilite` écrit et poussé
  (cluster vierge dédié — `DROP ROLE` est global au cluster) : up → down →
  diff avec l'état vierge → re-up → diff (schéma, rôles, extensions, ACL).
  Verdict de la première exécution : voir section 5.
- **Contrat d'URL du test d'isolation** (question n°8 du 27) : inchangé —
  la CI verte valide le contrat actuel (le test dérive l'URL applicative) ;
  l'arbitrage reste ouvert mais n'est plus bloquant.
- **Retouche utilisateur de CLAUDE.md** : toujours non commitée, laissée
  telle quelle (consigne).

## 2. Blocs — sans arrondir

| Bloc | État | Tours |
|---|---|---|
| 0 Reprise | terminé | — |
| 1 CI, priorité absolue | **terminé** (inv. 1 prouvé + sensibilité ; différentiel Python vert) | 0 |
| 2 Confinement inv. 7, neutralisation retirée | terminé | 1 |
| 3 Argon2 : plafond 64 Mio + sémaphore | terminé | 1 |
| 4 Bornes à deux étages | terminé | 1 |
| 5 Enveloppe versionnée d'état | terminé | 1 |
| 6 Cadrage injectivité (doc) | terminé | 0 (document) |
| 7 Méthode de travail, six corrections | terminé | 0 (document) |
| 8 Coutures | terminé **sans code** — décisions d'architecture, options recommandées consignées | 0 |
| 9 Réversibilité en CI | **terminé — PROUVÉ run n°15** (5 rouges instructifs, section 5) | 2 |
| 10 Matrice à jour | terminé (invariants 1, 2, 7, 12, 13 + lecture d'ensemble) | 0 |
| 11 Approfondissement libre | partiel : 2 corrections ciblées (expect(), numérotation) | — |

Hors blocs, sur consigne reçue en cours de session : README créé puis tenu
à jour (consigne permanente du propriétaire), `docs/taches-delegables-analyse.md`
versé au dépôt, ses quatre questions intégrées à la méthode de travail, la
recommandation M4 (relance client) notée au backlog SANS être appliquée.

## 3. Ce que chaque tour d'approfondissement a trouvé

- **Bloc 2** : le corpus n'avait AUCUN `\r` — le proptest verbatim ajouté
  couvre les chaînes arbitraires ; sabotage (`\r`→`\n` réintroduit) attrapé
  par le test unitaire ET le proptest (contre-exemple minimal `"\r"`).
  Bornes multi-octets et borne exacte testées.
- **Bloc 3 — la vraie trouvaille de la session** : le premier test de
  sémaphore PASSAIT SOUS SABOTAGE. Sur cette machine, argon2 à 19 Mio prend
  plus que les 300 ms de la fenêtre d'observation : la lenteur du hachage
  masquait une porte cassée. Corrigé (profil rapide 8 Mio t=1, fenêtre
  500 ms) ; le test ré-échoue sous sabotage, re-vert après. Leçon : un test
  de concurrence dont le sujet est LENT doit isoler la cause de l'attente.
- **Bloc 4** : sabotage (limite dure → validation) attrapé par 6 tests,
  dont le proptest « au-delà de la limite dure = refus, quel que soit
  `requires_approval` ».
- **Bloc 5** : sabotage (contrôle de version débranché) → l'enveloppe
  future est interprétée avec le schéma courant et échoue en
  « missing field `status` » — l'erreur TROMPEUSE exacte que l'enveloppe
  ferme ; le test le détecte.

## 4. Prouvé par un test EXÉCUTÉ (nouveautés de la session)

- Invariant 1 : isolation RLS + sensibilité (CI, runs 1 et 2).
- Spécification d'encodage non ambiguë : différentiel Rust↔Python, 14 014
  vecteurs (CI, run 4 et suivantes).
- Invariant 7 en mode confinement : contenu externe VERBATIM (corpus 34 cas
  + proptest), troncature à préfixe verbatim, bornes multi-octets.
- Argon2 : plafond 64 Mio aux deux bords ; sémaphore — au-delà de la borne
  les vérifications attendent puis aboutissent, jamais d'échec ; 3× la
  borne en parallèle sans erreur.
- Bornes à deux étages : trois zones par borne scalaire, limite dure
  inviolable (proptest), fail-open du préfixe vide fermé.
- Enveloppe d'état : version inconnue refusée en nommant trouvée/supportée,
  enveloppe sans version refusée, reprise via l'enveloppe.
- Suspension incohérente (WaitingApproval sans pending) : échec propre
  tracé, plus de panique possible du worker.

## 5. Écrit mais non (encore) vérifié

- **Réversibilité des migrations (invariant 13) : PROUVÉE, run n°15** —
  après CINQ rouges, chacun utile. Chronique complète : run 10 rouge →
  hypothèse ACL brute (un `nspacl` matérialisé après GRANT+REVOKE n'est
  pas le texte du NULL initial), correctif `aclexplode`+`acldefault` ;
  run 12 encore rouge et les journaux exigent un jeton → construction d'un
  canal de diagnostic lisible en anonyme (résumé de job d'abord — pas
  rendu dans le HTML public — puis branche `ci-diagnostic` via
  raw.githubusercontent.com) ; run 14 : diagnostic limpide — rôles,
  extensions, ACL effective et schéma réel IDENTIQUES au vierge, seule
  divergence : le jeton ALÉATOIRE `\restrict` que pg_dump ≥ 18 tire à
  chaque invocation (bruit d'outillage, pas défaut de migration) ;
  filtré → run 15 VERTE. Les `.down.sql` étaient corrects depuis le
  début ; ce qui ne l'était pas, c'était la mesure. Réserve maintenue :
  prouvé via psql — l'outillage applicatif (`sqlx::migrate!`) n'a
  toujours aucun chemin de descente.
- Les coutures machine→chaîne d'audit et machine→policy réel : options
  recommandées écrites (questions-nuit), rien de câblé — le graphe de
  dépendances gelé t'appartient.
- La CI ne vérifie toujours pas : `--locked` sur cargo (désynchronisation
  du lock invisible jusqu'au build d'image), l'épinglage par SHA des
  actions. Constats de la revue de complétude, non traités cette session.

## 6. Défauts trouvés dans le code des sessions PRÉCÉDENTES

- **Le test de sensibilité du sémaphore qui ne prouvait rien** est de CETTE
  session (attrapé le jour même) — mais il illustre l'angle mort des tests
  temporels écrits par le même modèle.
- **`expect()` en chemin de bibliothèque** (machine.rs, session du 28 nuit) :
  état incohérent représentable → panique du worker possible. Corrigé en
  échec propre tracé, testé.
- **Numérotation des invariants fausse** dans machine.rs (1/2 au lieu de
  2/3 CLAUDE.md) : corrigée — la traçabilité invariant→test était faussée.
- **`canonical.py` : NameError structurel** (session du 28 nuit) — le mode
  script n'avait jamais pu fonctionner. Corrigé avant première exécution.
- **Le corpus adversarial comptait 34 cas, pas 35** — le chiffre faux
  s'était propagé dans trois documents. Corrigé partout.
- Où j'ai regardé sans rien trouver de neuf : budget.rs (re-lu au moment du
  bloc 5 — le problème `org_balance` sérialisé reste ENTIER, consigné par
  l'analyse du matin, à traiter au M3), la chaîne d'audit (les proptests et
  le différentiel n'ont rien révélé de plus).

## 7. Décisions prises seul — à réexaminer

Tout est dans `docs/questions-nuit.md` (section « reprise de jour »). Les
plus lourdes : le confinement poussé deux crans plus loin que le brief
(CRLF et marqueur de troncature supprimés aussi — « intact veut dire
intact ») ; le préfixe vide devenu violation de protocole (l'ex-« couvre
tout ») ; le « souple sans plafond » retiré du type ; le sémaphore fixé à
4 permis ; l'enveloppe en version 1 avec refus strict. Toutes réversibles,
rien en production.

## 8. Ce que je ferais en premier à ta place

1. **Trancher les trois décisions qui n'attendent que toi** : M4
   comptes-rendus→relance client (backlog), canal expert-comptable,
   les deux arêtes du graphe (runtime→audit, runtime→policy).
2. **Sauvegarder `doc/`** (audit stratégique, dossier banque, classeur) :
   il est HORS git — le push ne le protège pas. Un incident disque
   l'efface encore.
3. Les trois actions à coût nul de l'analyse stratégique : la lettre R7,
   le tableur d'économie unitaire, les quinze entretiens (les seize
   questions sont prêtes dans la méthode de travail).
4. Optionnel : supprimer ou garder la branche `ci-diagnostic` (canal de
   diagnostic anonyme du job de réversibilité — elle se réécrit à chaque
   run et ne contient que des instantanés de schéma).

## 9. Inquiétudes

- **La machine à états reste un modèle réduit** — le rapport précédent le
  disait, c'est toujours vrai, et les défauts consignés (crédit vérifié
  APRÈS l'appel de modèle, org_balance sérialisé dans TaskState,
  idempotence après crash absente de credits-concurrence.md) sont
  documentés mais PAS corrigés : ils exigent les coutures, donc tes
  arbitrages.
- **L'affichage sûr du contenu externe est désormais une dette de M6** : en
  retirant la neutralisation, la protection du dirigeant-lecteur (bidi,
  invisibles) repose sur un rendu qui n'existe pas encore. C'est écrit
  dans le modèle de menace v2 ET dans la section M6 — ne pas la laisser
  s'évaporer.
- **La CI est devenue le socle de preuve du projet** — et elle tourne sur
  des références flottantes (actions non épinglées par SHA, images sans
  digest, pas de --locked). Tant que c'est vrai, « la CI est verte » a une
  astérisque.
- **Fiabilité** : session longue mais régulière ; les trios sont restés
  verts à chaque commit et chaque sabotage a été remis en état
  immédiatement. Pas de baisse ressentie, mais je clos ici : les blocs
  restants (coutures, run 10 éventuellement rouge) exigent soit tes
  décisions, soit un verdict que je n'ai pas encore.
