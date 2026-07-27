# Questions des sessions autonomes

## Nuit du 28 au 29/07 — objection de conception sur le bloc 3c

**Le bloc 3c est inapplicable TEL QUEL à `ChainedEntry`, et pour une bonne
raison.** La consigne — « l'attestation n'a pas de champ `hash` mais une
méthode ; aucun chemin ne permet de construire un objet dont l'empreinte ne
correspond pas à son contenu » — a été appliquée à `AuditContent` (fait :
l'empreinte y est une méthode calculée). Appliquée à `ChainedEntry`, elle
se retourne contre le produit : **une entrée relue d'une base corrompue
DOIT pouvoir porter un hachage qui ment**, sinon `verify` n'a plus rien à
dénoncer — la corruption devient inreprésentable au lieu d'être détectée.
C'est exactement ce que fait `verify_org_chain` : il relit des empreintes
brutes et les confronte au recalcul.

**APPLIQUÉE** dans la nuit, sur autorisation explicite du propriétaire
(« tu peux le refondre si besoin, mais vérifie ce que tu fais ») — et
vérifiée par sabotage : rendre les champs publics fait ÉCHOUER le doctest
`compile_fail` avec « Test compiled successfully, but it's marked
compile_fail », donc le test prouve bien l'impossibilité de forger et non
un accident de compilation.

**Deux types au lieu d'un.**

- `ChainedEntry` — PRODUITE par `OrgChain::append`, empreinte privée +
  méthode : cohérente par construction, personne ne peut en forger une qui
  mente. C'est ce que le domaine émet.
- `StoredEntry` — RELUE du stockage (frontière `from_storage`, déjà gardée
  textuellement) : empreinte brute, potentiellement menteuse. C'est ce que
  `verify` prend en entrée, et c'est la seule forme qui rende la
  vérification utile.

Coût : refonte de `chain.rs`, `anchor.rs` et de leurs tests de mutation
(~39 tests touchés). Gain : il devient impossible d'écrire dans la chaîne
une entrée forgée par le code du domaine ; seule la frontière de stockage
peut en représenter une, et uniquement pour la dénoncer. À trancher par
toi — je ne refonds pas une API publique du domaine seul, de nuit.

## Session du 28/07/2026 — reprise de jour (remote actif, CI opérationnelle)

Choix réversibles pris seul :

1. **Document « tâches délégables » versé au dépôt.** Le document fourni en
   cours de session est enregistré verbatim dans
   `docs/taches-delegables-analyse.md` (tableau remis en forme markdown —
   le collage l'avait aplati — et dernier mot tronqué complété :
   « commencé »). `docs/` étant versionné, il est désormais sauvegardé sur
   le remote, contrairement à `doc/` qui reste hors git.
2. **Recommandation M4 du document (« comptes rendus » → « relance
   client ») : NON appliquée.** C'est une décision produit qui modifie
   `docs/jalons.md` ; notée au backlog, à trancher par toi. Ses quatre
   questions d'entretien seront intégrées à `docs/methode-de-travail.md`
   au bloc 7 (le document dit explicitement « à ajouter »).
3. **Canal expert-comptable.** Le document signale que ce choix (passer par
   lui ou aller au dirigeant) « a plus d'impact que la plupart des
   décisions techniques » et n'est tranché nulle part. Aucun bloc de la
   nuit ne le couvre : décision commerciale qui t'appartient.
4. **`serde_json` ajouté aux dev-dependencies de `kollega-audit`** pour le
   harnais différentiel (émission de la forme JSON de transport). Même
   statut que le précédent consigné (pur, sans E/S, déjà dans le
   workspace). Réversible.
5. **Bug corrigé dans `canonical.py` avant première exécution** : le bloc
   `__main__` précédait la définition de `_from_json` (NameError garanti
   en mode script). Déplacé en fin de fichier, fonctions d'encodage et de
   hachage inchangées octet pour octet — ce n'est PAS une divergence de
   spécification, c'est un défaut d'ordre de définition. Un mode
   `--hashes` a été ajouté pour le différentiel d'empreintes complètes.
6. **Bloc 4 — deux décisions dans la marge du brief.** (a) Le constructeur
   « souple seul » (validation sans limite dure au-dessus) n'existe plus :
   champs privés, `hard(limite)` ou `two_tier(seuil, limite)` seulement —
   c'est l'esprit du brief (« une même borne porte DEUX niveaux ») poussé
   dans le type. (b) Les chemins restent à UN niveau (dedans/dehors, pas
   d'ordre → pas d'« entre les deux ») ; en revanche j'ai fermé le
   fail-open du préfixe vide : `""`/`"/"` dans une liste n'« autorise plus
   tout », c'est un refus de règle malformée — l'accès universel légitime
   se déclare en omettant la restriction de chemins. Si tu veux un jour un
   vrai deux-étages spatial (libre dans X, validation dans Y, refus
   ailleurs), c'est exprimable par deux listes de préfixes — non construit,
   rien ne le demande.
7. **Bloc 2 — j'ai appliqué le confinement jusqu'au bout, deux crans plus
   loin que la lettre du brief** : la normalisation `\r\n`→`\n` et le
   marqueur de troncature injecté dans le contenu étaient aussi des
   modifications — supprimés (le drapeau `truncated` porte l'information),
   et le champ `neutralized` retiré du type (rien ne le consomme encore,
   la forme sérialisée était libre). Pas de désaccord sur le fond, mais
   une conséquence à ne pas perdre : la neutralisation protégeait AUSSI ce
   que le dirigeant LIT à la validation (un bidi peut inverser
   l'affichage). Ce devoir est transféré à la couche de présentation (M6 :
   isolation bidi au rendu, invisibles rendus visibles à l'affichage) —
   c'est écrit dans le modèle de menace v2 et dans la section M6, à ne pas
   laisser tomber au moment de l'interface.

## Session du 28/07/2026 (choix réversibles pris seul)

1. **ADR-0006 — vérification des mots de passe.** Remplacement de la liste
   blanche argon2 par plancher (m ≥ 8 Mio) + plafond (m ≤ 256 Mio) + re-hachage
   à la connexion. Décision d'architecture, prise parce que le brief la
   demandait explicitement ; bornes chiffrées à valider.
2. **Modes de bornes des politiques.** Défauts recommandés : montant souple,
   destinataires souple, chemins dur. Surchargeables. À confirmer.
3. **Format de hachage v3** — ajout de `height` dans l'enregistrement. Vecteurs
   régénérés. C'est la DERNIÈRE fenêtre gratuite avant une chaîne en production.
4. **Ancre au client** (BLOC 5) : double témoin dont remise quotidienne au
   dirigeant. Engagement produit, « révocable » tant que rien n'est livré.
5. **`proptest` dans la liste blanche de test de `kollega-core`** : ajouté
   après que le garde-fou du graphe l'a (correctement) refusé. Pur, sans E/S,
   comme `serde_json`.
6. **Numéro d'ADR 0006** (le 0005 a été pris par l'authentification hors
   contexte la nuit du 27).
7. **`kollega-runtime` gagne `serde`** (dépendance, pas dev) pour la
   sérialisation de l'état de tâche — indispensable à la reprise. Périphérie,
   pas le domaine : hors invariant 11.

## Bloc 8 — les deux coutures : options recommandées, NON tranchées

Les deux exigent de modifier le graphe de dépendances gelé par
`dependency_graph.rs` — c'est une décision d'architecture qui t'appartient.
Rien n'a été câblé cette session.

- **Couture AuditEvent → chaîne `kollega-audit`.** Option recommandée :
  `kollega-runtime` dépend de `kollega-audit` ; la machine pure continue
  d'ÉMETTRE des `AuditEvent` sans horodatage (elle n'a pas d'horloge, et ne
  doit pas en avoir), et c'est la boucle d'exécution de la périphérie (M3)
  qui transforme chaque événement en `EntryContent` (horodatage injecté par
  l'horloge du monde réel) et l'ajoute à la chaîne de l'organisation
  (`OrgChain::append`) DANS LA MÊME transaction que l'état de tâche — le
  `Vec<AuditEvent>` local devient un simple tampon d'émission, jamais une
  source de vérité. À trancher par toi : (a) l'arête runtime→audit dans le
  graphe ; (b) qui détient la queue de chaîne (table `audit_log`, verrou
  par organisation — rejoint `docs/credits-concurrence.md`) ; (c) le refus
  explicite de toute horloge dans le pur.
- **Couture moteur de politiques réel.** Option recommandée :
  `PlannedAction::UseTool` porte un `ToolCallRequest` COMPLET (montant,
  destinataires, chemins — pas seulement le nom), le trait `PolicyEngine`
  du runtime prend ce `ToolCallRequest` et retourne l'`Evaluation` de
  `kollega-policy` ; l'adaptateur de production appelle
  `kollega_policy::decide(règles_de_l_organisation, requête)`. Sans cela,
  l'invariant 2 reste un contrôle par nom d'outil. À trancher par toi :
  (a) l'arête runtime→policy dans le graphe ; (b) la véracité des valeurs
  déclarées (montant, destinataires) à garantir par la couche MCP — point
  de revue M2 déjà consigné.

## Questions architecturales non tranchées (28/07)

- Réconcilier l'`AuditEvent` de la machine à états (BLOC 10) avec la chaîne
  `kollega-audit` : la boucle de production devra journaliser DANS la chaîne
  chaînée, pas dans un `Vec` local. Couture identifiée, non cousue.
- Brancher le vrai `kollega-policy` (avec `ToolRule`) dans la machine à états,
  à la place du trait `PolicyEngine` local. Décision de câblage, jalon M3.

---

# Questions de la session autonome du 27/07/2026

Règle appliquée : détail réversible → option la plus conservatrice, notée
ici ; question architecturale → non tranchée, notée, bloc abandonné ou
contourné. Aucun bloc n'a été abandonné.

## Choix réversibles pris (à confirmer ou renverser au réveil)

1. **Numérotation de l'ADR d'authentification.** Le brief demandait
   `0003-authentification-hors-contexte.md`, mais 0003 est pris
   (`0003-postgres-seul-moteur.md`, renuméroté lors du pivot). Écrit en
   **0005**, prochain numéro libre.
2. **Email — règles de validation.** ASCII uniquement (atext RFC 5321 + point
   pour la partie locale ; alphanumérique + tiret pour les étiquettes de
   domaine), adresse entière minusculisée, blancs extérieurs tolérés puis
   retirés. Conséquences : pas d'IDN ni d'adresse Unicode (ce serait une
   décision punycode explicite), partie locale insensible à la casse (la RFC
   permet le contraire, personne ne le pratique). La restriction ASCII a été
   ajoutée après la revue adversariale : la version initiale acceptait les
   homoglyphes, c'était un vrai trou.
3. **Politiques — sémantique des seuils.** Au seuil exact = autorisé
   (cohérent avec le plafond de coût) ; dépassement = REFUS, jamais une
   demande de validation (la validation vient du seul drapeau
   `requires_approval`) ; valeur non déclarée sous une borne (montant,
   destinataires, chemins) = refus ; antislash et `..` dans un chemin =
   refus d'office. Question ouverte pour plus tard : veut-on un jour un
   seuil à deux étages (auto en dessous de X, validation entre X et le
   plafond) ? Non implémenté — trois clients payants d'abord.
4. **Emplacement du hachage de mots de passe.** Module `kollega-api::auth`.
   Créer une crate dédiée serait une décision de frontière (architecturale),
   non prise cette nuit. Déplaçable en cinq minutes si tu préfères.
5. **Audit — encodage de l'horodatage.** Microsecondes Unix (i64) en décimal
   ASCII, plutôt que RFC3339 : suit la précision de `timestamptz`, aucune
   dépendance de formatage. Fait partie du format figé par les vecteurs.
6. **Audit — genèse hachée avec 32 octets à zéro.** La version initiale
   omettait le préfixe pour la première entrée ; la revue a montré que
   l'argument « longueur fixe » était alors faux. Changé AVANT toute
   écriture en production (rien ne consomme encore ce format), vecteurs
   régénérés, V1 recoupé à nouveau hors Rust. C'était la dernière fenêtre où
   ce changement était gratuit.
7. **Audit — périmètre de l'enregistrement haché.** « payload canonique »
   inclut action, actor, org_id ET payload (ordre figé) : ne hacher que la
   charge utile aurait laissé l'acteur et l'action falsifiables.
8. **CI — contrat du test d'isolation.** Le brief mentionnait « deux URLs »
   fournies par la CI ; l'existant fait dériver l'URL kollega_app par le
   test lui-même (qui pose le mot de passe du rôle). Changer ce contrat sans
   pouvoir l'exécuter aurait risqué de casser la CI au premier push : NON
   MODIFIÉ cette nuit, à arbitrer quand la CI tournera.
9. **Organisation de kollega-core.** Les ajouts M1 vivent dans deux modules
   (`ids`, `identity`) ré-exportés à la racine ; le fichier des six types
   validés reste `lib.rs`, modifié uniquement pour les `mod`/`pub use` et
   les numéros d'invariants v1→v2 dans les commentaires.

## Questions architecturales rencontrées, non tranchées

- **Ancrage de la chaîne d'audit.** La revue a confirmé que `verify` seul ne
  détecte ni la troncature de queue ni la réécriture d'un suffixe par un
  attaquant en écriture. J'ai ajouté l'API pure `verify_with_tail` (ancre de
  confiance) et documenté le modèle de menace — mais OÙ vivra l'ancre
  (copie hors site, journal d'exploitation, remise au client ?) est une
  décision d'exploitation qui t'appartient, à prendre au jalon de
  persistance de l'audit.
- **CLAUDE.md.** Non touché, conformément à la consigne. Deux imprécisions y
  demeurent, à corriger par tes soins si tu en es d'accord : l'invariant 4
  dit « détecte toute altération » (vrai seulement avec une ancre de queue,
  cf. ci-dessus) ; l'invariant 1 et la table d'architecture disent
  « SET LOCAL app.current_org » alors que la forme réelle imposée par la
  garde textuelle est `set_config('app.current_org', $1, true)` —
  équivalente, mais autant que la constitution dise la vérité exacte.
