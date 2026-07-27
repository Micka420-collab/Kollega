# État de session — mis à jour après chaque bloc

> **29/07/2026 — reprise de la boucle. Quatre gardes, toutes vérifiées par
> sabotage.** 197 tests. Le fil conducteur de cette reprise n'est pas
> d'ajouter des fonctions, c'est de **vérifier que les preuves prouvent** —
> parce qu'à ce stade le risque n'est plus d'écrire du code faux, c'est de
> croire un tableau vert.
>
> 1. **Invariant 11, par transitivité** (dette du backlog du 26/07). La
>    liste blanche ne lisait que les manifestes, donc le *déclaré* ; la
>    fermeture est désormais lue dans le graphe résolu. Sabotage : ajouter
>    `tokio` au domaine révèle **tokio, socket2 et mio**. Le domaine ne
>    tire par ailleurs ni entropie ni horloge — la reprise dépend de son
>    déterminisme, et l'unification des features de cargo empêche le
>    compilateur de défendre cette frontière.
> 2. **Se sauter en CI devient un échec.** Sept tests exigent une base
>    réelle et se sautaient sans elle ; cette base ne leur était fournie
>    que par UNE ligne de `ci.yml`. La retirer aurait rendu sept tests
>    verts en ne prouvant plus rien, pendant que six lignes de la matrice
>    auraient continué d'afficher « OUI — CI ».
> 3. **L'inventaire des crates branchées nulle part devient mécanique**, et
>    il échoue dans les deux sens : une nouvelle orpheline rougit, et une
>    orpheline qu'on branche rougit aussi — pour forcer la mise à jour du
>    README à l'instant où l'état change, plutôt qu'après.
> 4. **Invariant 13 : chaque montée a sa descente**, ou dit pourquoi non.
>    Le job prouvait que les descentes existantes ramènent au vierge, pas
>    qu'une migration en ait une — et une migration de DONNÉES serait
>    passée sans laisser de trace dans un `pg_dump --schema-only`.
>
> 5. **La signature de l'image est enfin vérifiée** (run n°58, verte du
>    premier coup). Depuis le début, la CI signait et attestait sans jamais
>    valider : tout ce qu'on prouvait, c'est que `cosign sign` rend 0. Une
>    signature qu'on n'a jamais su valider n'est pas une garantie, c'est
>    une dépense. L'identité est épinglée — accepter n'importe quel
>    signataire reviendrait à vérifier qu'une signature existe sans
>    vérifier de qui elle vient.
> 6. **Le serveur HTTP démarre pour de vrai, et on l'écoute.** `kollega
>    serve` est la commande par défaut de l'image publiée et signée, et
>    aucun test ne l'avait jamais démarrée : `kollega-api` ne contenait
>    qu'un `crate_compiles()`, un placebo qui comptait comme un vert.
>    Toute la chaîne de livraison portait donc sur un binaire dont
>    personne n'avait vérifié que sa commande par défaut savait se lever.
>
> 7. **Le job de réversibilité ne comparait les privilèges NULLE PART.**
>    `pg_dump --no-privileges` les retire du schéma, et la comparaison
>    d'ACL ne portait que sur le schéma `public`. Or les `GRANT`/`REVOKE`
>    par table sont le mécanisme *même* de l'invariant 12 : une migration
>    qui accordait un privilège sans le reprendre à la descente passait
>    verte. Corrigé, run n°63 verte.
> 8. **Invariant 12 : la sensibilité est enfin prouvée** (run n°65 rouge).
>    `no_physical_deletion` était sa seule preuve et n'avait jamais été vue
>    échouer. Branche jetable, `REVOKE DELETE` sur `users` retiré de la
>    migration 0006 : `verifications` rouge sur ce test **seul**, message
>    exact attendu, `reversibilite` restée verte. Branche supprimée des
>    deux côtés. **Toutes les gardes textuelles du dépôt ont désormais été
>    observées en échec** — aucune n'est un vert de principe.
> 9. **Marque d'ordre d'octets interdite.** J'avais moi-même introduit un
>    BOM dans `ci.yml` via `Set-Content -Encoding utf8` ; GitHub l'a
>    toléré. Dans un `.sql` joué par psql, la même marque fait échouer la
>    migration sur une erreur de syntaxe illisible — une production qui ne
>    démarre pas. La garde a rougi sur ce défaut réel dès sa première
>    exécution, sans sabotage artificiel.
>
> 10. **Toute preuve existante a été vue ROUGE.** La matrice porte
>     désormais une rubrique « Sensibilité » qui le dit invariant par
>     invariant, avec l'évidence. Les dernières falsifiées : refus par
>     défaut inversé (2), contenu externe concaténé à l'instruction (7),
>     rechargement du solde retiré (5), `audit verify` rendant 0 sur
>     chaîne rompue (4), appel en attente perdu à la relecture (tranche
>     verticale, rouge à deux niveaux).
> 11. **`--no-fail-fast` posé dans la CI, et ce n'est pas du confort.**
>     Sans lui, cargo s'arrête au premier binaire en échec : mon premier
>     essai sur la tranche verticale a conclu que le test sur base réelle
>     ne détectait rien, alors qu'il n'avait jamais tourné. **Un test qui
>     n'a pas tourné ressemble en tout point à un test qui n'a rien vu** —
>     un sabotage peut donc mentir dans le sens rassurant, qui est le
>     pire. Le même piège s'était produit le même jour en local.
> 12. **Faille dans ma propre garde** `migrations_shape` : le plancher de
>     justification comptait tout le fichier, SQL compris — dix lignes de
>     DDL suffisaient à franchir les 60 caractères. La règle paraissait
>     stricte et était vide. Corrigée : seul le bloc de commentaire qui
>     suit le marqueur compte.
>
> 13. **Relecture ADVERSARIALE de mes propres gardes : quatre des cinq
>     avaient un trou.** Toutes avaient pourtant été « vérifiées par
>     sabotage » — mais je les avais sabotées sous la forme exacte
>     qu'elles savaient reconnaître, ce qui rendait la vérification
>     circulaire. En cherchant à les *contourner* : le corps vide écrit
>     sur deux lignes passait ; un nom de variable donné par une
>     constante passait ; dix lignes de SQL valaient justification
>     d'irréversibilité ; un `.sql` mal nommé échappait à l'exigence de
>     descente ; une simple ligne de manifeste faisait passer une crate
>     pour branchée. Toutes corrigées, chaque contournement vérifié
>     rouge. Le détail et la leçon sont en tête de
>     `docs/matrice-invariants.md`.
>
> 14. **Deuxième vague : les gardes ANCIENNES aussi.** Bilan des deux
>     passes — **sept des dix garde-fous du dépôt avaient au moins un
>     trou, dix trous en tout**, tous corrigés, chaque contournement
>     vérifié rouge. Le plus grave était ancien et touchait l'invariant 1 :
>     `sql_context_guard` ne cherchait que `$1, false` en minuscules, si
>     bien que `FALSE`, `'f'` et un troisième argument paramétré posaient
>     le contexte d'organisation en portée SESSION — il survit alors au
>     retour de la connexion dans le pool, et la requête suivante, celle
>     d'un AUTRE client, se serait exécutée sous le contexte du précédent.
>     Corrigé en fermant la question plutôt qu'en énumérant les fautes :
>     seule la forme canonique littérale est admise.
>     Ont tenu : `no_byte_order_mark` et `storage_boundary`.
>
> 15. **Documentation confrontée au code, puis migrations relues.** Trois
>     écarts. (a) `credits-concurrence.md` faisait d'une colonne
>     « tasks.cost_cents » la pièce centrale de deux exigences — elle n'a
>     **jamais existé** ; le consommé vit dans l'enveloppe `tasks.state`.
>     Document relu en entier : trois exigences sur quatre TENUES, la file
>     d'attente par `SKIP LOCKED` **non commencée** (le motif n'apparaît
>     nulle part). (b) ADR-0006 énonce un plafond de 256 Mio à son point 3
>     alors que son amendement l'abaisse à 64 Mio, seul appliqué — un
>     lecteur du point 3 aurait pu « réparer » le code à l'envers. (c) Le
>     « corpus de 34 cas », répété en trois endroits, était exact mais que
>     rien ne tenait : le nombre est désormais asserté dans le test.
> 16. **Aucun test n'essayait jamais de VIOLER une contrainte du schéma.**
>     Les tests prouvaient que l'application ne produit pas d'état
>     interdit, jamais que la base le refuserait. Or `CHECK (balance_cents
>     >= 0)` est la dernière ligne de défense de l'invariant 5 : le
>     retirer aurait laissé le test de concurrence vert, puisqu'il éprouve
>     le verrou et non la contrainte. Éprouvées désormais, à l'insertion
>     comme à la mise à jour, en vérifiant le CODE d'erreur PostgreSQL —
>     et l'unicité d'email dans ses DEUX moitiés, doublon refusé dans une
>     organisation, même email accepté dans une autre.
>
> 17. **Suite de la relecture documentaire.** `encodage-canonique.md` se
>     vérifie **intégralement** : ordre des champs, préfixe de 32 octets
>     zéro, cinq vecteurs de référence, et le « 14 014 » du README
>     recalculé exactement (12 014 encodages + 2 000 empreintes).
>     `methode-de-travail.md` s'annonce lui-même comme des hypothèses non
>     confrontées — rien à vérifier contre le code, et son seul renvoi
>     (l'impact schéma au backlog) est exact. **ADR-0002 point 3** promet
>     qu'une requête hors contexte ÉCHOUE ; le test existait mais se
>     contentait d'un `is_err()`, vert sur une connexion coupée ou une
>     faute de frappe. Il nomme désormais la cause. Deux contraintes de
>     plus éprouvées : le « deuxième filet » de l'idempotence (unicité par
>     tâche et itération, que les tests existants ne pouvaient PAS
>     atteindre puisqu'ils passent tous par la dérivation) et la clé
>     étrangère des effets vers les tâches.
> 18. **Deux erreurs de ma part, rattrapées — trois runs rouges dans
>     l'historique (91, 92, 93).** (a) J'ai poussé un fichier non formaté :
>     ma chaîne PowerShell enchaînait par `;`, qui n'interrompt pas sur
>     échec, et le `git add` s'est exécuté malgré `fmt --check` rouge. La
>     règle du dépôt est explicite et je l'ai enfreinte ; les commandes
>     s'arrêtent désormais au premier rouge. (b) Le nouveau test de
>     contraintes échouait sur base réelle : `create_task` lit le solde, et
>     mes sections précédentes font délibérément avorter leurs transactions
>     — aucune ligne de crédit ne subsistait. Corrigé, CI n°94 verte. Les
>     rouges restent dans l'historique plutôt que réécrits.
>
> 19. **Assertions faibles : trois autres, dont la seule preuve de
>     l'invariant 12.** `no_physical_deletion` se contentait de
>     `attempt.is_err()` sur six tables — vert sur une faute de frappe
>     (42P01), une transaction déjà avortée, ou un refus de CLÉ ÉTRANGÈRE
>     (23503), ce dernier cas n'ayant rien de théorique pour
>     `organizations`, dont dépendent quatre tables. On aurait eu une
>     preuve verte de l'invariant 12 ne disant rien des **privilèges**,
>     c'est-à-dire du mécanisme qui le porte. Le code 42501 est désormais
>     exigé nommément, ici et sur les deux refus équivalents de la tranche
>     verticale. À retenir : le sabotage de la CI n°65 avait prouvé la
>     sensibilité **pour `users`** — elle ne se transportait pas aux cinq
>     autres tables.
> 20. **Les ADR 0003 et 0004 étaient les documents les plus abîmés.**
>     Titres portant le numéro d'un AUTRE ADR (renumérotation du pivot non
>     répercutée) ; une table nommée « audit_log » qui n'a jamais existé —
>     citée ici sans accents graves, puisqu'ils annoncent un identifiant
>     réel et qu'une garde le vérifie (c'est
>     `audit_chain` + `audit_content`) ; et **trois références d'invariant
>     périmées** de la numérotation v1 — la réversibilité rattachée au 12
>     au lieu du 13, l'absence d'E/S au 9 au lieu du 11. Les numéros
>     périmés sont une classe d'erreur récurrente, à surveiller. Deux
>     gardes ajoutées : les NOMS DE TABLES cités dans la documentation
>     (avec une liste explicite des tables PRÉVUES, jalon à l'appui), et
>     le titre d'un ADR adossé au numéro de son fichier.
>
> 21. **Balayage complet des « invariant N » : rien d'autre.** Après les
>     corrections des ADR 0003 et 0004, les quelque soixante citations de
>     la documentation sont toutes justes. La classe d'erreur était
>     confinée à ces deux fichiers. Une garde adosse désormais la matrice
>     aux **treize invariants déclarés par CLAUDE.md** — lue, jamais
>     écrite : un invariant ajouté à la constitution sans sa ligne serait
>     un engagement que rien ne surveille.
> 22. **`jalons.md` et `compose.yaml` : aucun écart.** La définition de
>     terminé de M0 exige un `compose.yaml` — il existe (52 lignes), et
>     ses quatre variables d'environnement correspondent exactement à
>     celles que lit le binaire. Mais rien ne le garantissait, et cette
>     composition n'est **exécutée nulle part** : la CI construit l'image,
>     elle ne lance pas la composition. Renommer une variable dans
>     `main.rs` l'aurait cassée en silence, la panne n'apparaissant qu'au
>     premier déploiement. Une garde tient maintenant la correspondance.
> 23. **Une garde m'a attrapé, puis une autre — trois fois en tout.** (a)
>     Ma note de session nommait la table audit_log **entre accents
>     graves** en expliquant qu'elle n'a jamais existé : la garde des noms
>     de tables, écrite au commit
>     précédent, a rougi sur son auteur au premier document suivant (CI
>     n°99). (b) Le fichier de la garde de déploiement contenait le texte
>     `std::env::var(` comme MOTIF, que la garde anti-saut traque — elle
>     s'était tendu le même piège à elle-même quelques heures plus tôt.
>     (c) Et en RÉDIGEANT ce point (a), j'ai recopié la faute avec ses
>     accents graves — la garde a rougi une troisième fois, mais cette
>     fois **avant** le commit, parce que je vérifie désormais les gardes
>     documentaires avant de committer de la documentation.
>     Les trois corrigés. **Leçon : un commit « docs » n'est pas plus sûr
>     qu'un autre dès lors que des gardes lisent la documentation** — je
>     n'avais pas surveillé la CI après un commit purement documentaire.
>
> 24. **Chemins d'erreur du code : deux dégradations silencieuses dans
>     l'audit.** (a) `now_micros` écrivait `.unwrap_or(0)` — une horloge
>     antérieure à l'époque Unix (machine virtuelle dont l'horloge
>     matérielle repart de zéro, conteneur sans synchronisation) scellait
>     dans la chaîne un horodatage valant **1970**, cohérent et faux.
>     L'erreur de `duration_since` porte pourtant l'écart : il est repris
>     en négatif, et `Timestamp` sait déjà représenter l'avant-époque. Un
>     horodatage négatif se remarque ; un 1970 se confond avec une valeur
>     par défaut. (b) La hauteur de chaîne était écrêtée à `i64::MAX` en
>     cas de dépassement : le stocké aurait divergé du haché, rendant la
>     chaîne invérifiable pour une cause introuvable. Inatteignable
>     (2⁶³ entrées), mais le mode de défaillance était le mauvais — on
>     refuse d'écrire plutôt que d'écrire un mensonge.
> 25. **`TaskNotFound` n'était produit par aucun test**, et il porte une
>     propriété de sécurité : sous RLS, la tâche d'une AUTRE organisation
>     est invisible, donc indiscernable d'une tâche inexistante. Qui
>     « améliorerait » ce diagnostic en distinguant les deux cas ouvrirait
>     un canal d'énumération sans toucher à la RLS ni à aucune politique.
>     Le test refuse cette amélioration-là, et vérifie en outre que la
>     tentative de B n'a pas altéré la tâche de A.
>
> 26. **Quatre variantes d'erreur n'étaient produites par aucun test.**
>     La plus lourde portait une propriété que la migration 0004 affirme :
>     « un rejeu dont le contenu a été purgé échoue explicitement — il ne
>     ré-exécute surtout pas ». L'idempotence repose sur DEUX tables :
>     `tool_call_effects` retient qu'un appel a eu lieu, `audit_content`
>     ce qu'il a rendu ; la purge RGPD efface le second, jamais le
>     premier. Un rejeu n'a donc que deux conduites — refaire l'appel,
>     donc renvoyer un mail au client d'une organisation qui vient
>     d'exercer son droit à l'effacement, ou refuser en nommant la cause.
>     Le code choisit la seconde ; rien ne l'y tenait. Le test vérifie
>     séparément que le refus est bien `CorruptState`, qu'il **nomme** la
>     purge, et que le compteur d'envois réels reste à un.
>     Ajoutée aussi `BudgetError::NegativeState` — défense de dernier
>     recours jamais vue se déclencher. **Restent non produites** :
>     `ChainConflict` (il faudrait épuiser les trois rejeux de hauteur) et
>     `Accounting` (simple report d'une erreur de budget).
>
> 27. **Angles neufs, rendement en baisse — dit franchement.** Trois
>     explorations, une seule trouvaille. (a) Les `.down.sql` défont bien
>     ce que leurs `.up.sql` font ; la seule asymétrie — `kollega_migrate`
>     non supprimé — **est justifiée dans le fichier**, comme l'invariant
>     13 l'exige. (b) `CompiledPrompt` a ses champs publics, si bien que
>     la garantie de l'invariant 7 vient de `compile` et non du type :
>     latent, aucun document ne prétend le contraire, consigné dans
>     `questions-nuit.md` sans être appliqué (modification d'API publique
>     du domaine). (c) **Trouvaille réelle** : l'ADR-0006 pose trois
>     plafonds argon2 contre une empreinte empoisonnée, et un seul était
>     éprouvé. Les itérations sont pourtant un multiplicateur de temps de
>     calcul aussi direct que la mémoire en est un d'allocation — `t=1000`
>     immobiliserait un cœur des dizaines de secondes à chaque connexion
>     sans rien allouer d'anormal, et le plafond mémoire ne l'arrête pas.
>
> 28. **Le test d'isolation ne détruisait pas seulement SES données.**
>     `cargo test` exécute les binaires en parallèle contre la même base.
>     Trois dangers dans le seul `rls_isolation`, c'est-à-dire dans la
>     preuve la plus importante du dépôt. (a) Un `TRUNCATE … CASCADE`
>     **global** effaçait tâches, attestations, crédits et effets de tous
>     les tests tournant au même instant. (b) Sa preuve de sensibilité
>     comptait les utilisateurs de TOUTE la base : l'assertion « exactement
>     2 » dépendait de ce que les autres binaires faisaient au même moment.
>     (c) Le pire, vu en dernier : `ALTER TABLE … DISABLE ROW LEVEL
>     SECURITY` agit **globalement**, et pendant cette fenêtre
>     `rls_structural` — qui affirme l'inverse — pouvait échantillonner.
>     Deux tests corrects, un rouge intermittent, et une cause qu'on
>     chercherait dans les migrations. Corrigé : suppressions ciblées,
>     comptage borné, verrou consultatif partagé.
>     **Hypothèse, pas certitude** : le rouge inexpliqué de la run n°41,
>     imputé à l'époque à l'infrastructure, pourrait venir de là.
>
> 29. **Audit de concurrence clos, et deux angles propres.** Tous les
>     autres tests de base bornent leurs opérations par `org_id`, y compris
>     l'altération volontaire de chaîne d'`audit_verify_command` ; les deux
>     seules requêtes sans portée sont les assertions de refus par
>     privilège, où l'absence de portée est **voulue** — elles prouvent que
>     le rôle ne peut rien supprimer, nulle part. Seul `rls_isolation`
>     était en cause. Les proptest sont tous configurés entre 1 000 et
>     4 000 cas, loin du défaut de 256. Le générateur du différentiel
>     couvre bien les pièges : alphabet hostile pour les textes **et les
>     clés d'objet**, un caractère sur huit tiré de tout l'espace Unicode,
>     entiers biaisés vers `i64::MIN`/`MAX`.
>     **Rien trouvé sur ces trois angles, et c'est dit tel quel.**
> 30. **`docs/README.md`** : index distinguant ce qui fait autorité, les
>     instantanés datés jamais mis à jour, et les hypothèses à confronter.
>     Chaque entrée nomme **ce qui la tient** — un document sans mécanisme
>     derrière lui se reconnaît alors d'un coup d'œil.
>
> **Deux corrections de README dans le sens de la MODESTIE**, à noter parce
> qu'elles surprennent : il annonçait `kollega-model` comme un squelette de
> 9 lignes (il en fait 273) et « pas de serveur HTTP servi, pas de
> connexion base branchée » (les deux existent). La règle est l'avancement
> réel ; un README qui sous-estime ici pendant qu'il survend ailleurs n'est
> pas modeste, il est simplement peu fiable.
>
> **La trouvaille de fond, à toi :** `kollega-model` — 273 lignes, contrat
> `ModelProvider` réel, `ApiKey` expurgée, quatre modes d'échec — n'est
> utilisé par **aucune crate**. La boucle appelle un port homonyme qui ne
> reçoit qu'un numéro d'itération. C'est le défaut du trait `PolicyEngine`
> supprimé la veille, à l'identique. Conséquences : l'invariant 7 n'a, en
> aval de l'assemblage, aucun chemin réel à protéger ; et l'invariant 5 ne
> deviendra « vérifié AVANT » que quand la boucle recevra l'estimation que
> `ModelRequest` porte déjà — l'option 1 n'est pas à concevoir, elle est à
> brancher. Le branchement engage la conception de la boucle d'agent
> (M3/M4) : consigné dans `questions-nuit.md`, pas tranché seul.

> **Nuit du 28 au 29/07/2026 — session en boucle auto-cadencée. TERMINÉE.**
>
> Les six priorités du brief ont été faites, puis cinq invariants ont été
> promus (2, 5 partiellement, 6, 12) et huit trouvailles d'intégration
> consignées. **Neuf invariants sur treize sont prouvés par un test
> exécuté** ; 190 tests, dernière CI verte **n°50**. Rapport complet :
> `docs/rapport-nuit-2026-07-29.md`.
>
> **Pourquoi la boucle s'est arrêtée là** : le travail identifié est fait,
> et ce qui reste ne peut pas avancer sans toi ou sort du jalon en cours.
> Continuer aurait produit du code spéculatif — ce que CLAUDE.md interdit
> (« rien hors périmètre, même si c'est facile »).
>
> **Ce qui t'attend, par ordre de valeur :**
> 1. **Le coût réel** — une clé d'API, un seul appel sur le modèle le moins
>    cher. C'est ce chiffre qui fixe un prix ; `docs/economie-unitaire.md`
>    l'attend, et rien d'autre ne le débloque.
> 2. **L'invariant 5, « vérifié AVANT l'appel de modèle »** — trois options
>    dans `questions-nuit.md`, ma recommandation posée (seuil plancher,
>    puis corriger CLAUDE.md par ADR). Non tranché : cela engage le produit
>    ET la constitution.
> 3. **L'effacement logique** — `deleted_at` existe dans le schéma, aucun
>    code ne le pose. Écrire un `soft_delete` sans cas d'usage serait
>    spéculatif ; il faut d'abord savoir QUI efface quoi et depuis où.
> 4. Décisions produit en attente : modèle M4 « relance client », canal
>    expert-comptable, et l'engagement de maintenance des digests de base
>    (`deploy/README-bases.md`).


Session en cours : 28/07/2026, deuxième session de jour (suite directe).
Brief : LA TRANCHE VERTICALE D'ABORD — rien d'autre ne commence avant
qu'une tâche traverse réellement (créée → politique → exécutée → débitée →
journalisée dans PostgreSQL, attestation et contenu séparés → interrompue →
reprise depuis la base → même résultat).

Environnement : remote + CI opérationnels (runs 1-15, invariants 1 et 13
prouvés) ; PostgreSQL local ABSENT → toute l'intégration se prouve en CI,
itération par push. Pas de clé d'API dans l'environnement (à vérifier au
bloc 2).

Suspens hérité de la session précédente : rien d'interrompu ; bloc 11
partiel (gisement documenté : org_balance sérialisé, crédit vérifié après
plan — traités par la tranche) ; décisions propriétaire pendantes sur les
arêtes du graphe — LE PRÉSENT BRIEF ARBITRE : la tranche exige le câblage.

| Bloc | Statut | Tours | Note |
|---|---|---|---|
| 0 — Reprise | terminé | — | CI 15 verte intégrale, 16 (docs) en cours ; graine proptest-regressions à versionner |
| 1 — La tranche traverse | **TERMINÉ — run n°17 verte du premier coup** (verifications + reversibilite + image) | 0 | Migration 0003 + pilote `driver.rs` + test `vertical_slice.rs` (8 étapes : suspension→interruption→reprise→même résultat, fourche 23505, ajout seul par GRANT testé, isolation témoin, purge RGPD avec chaîne intacte). Trouvailles d'intégration accumulées : TRUNCATE→CASCADE (FK 0003), garde SET v1 morte au premier UPDATE…SET (prédite — refondue en liste d'interdits), sqlx sans feature json → casts `::jsonb`, verrou credits FOR UPDATE sérialise les pas d'une même org (le réessai de fourche devient défense en profondeur), le rejeu de pas ré-appellerait un vrai modèle (dette idempotence rendue concrète), graphe déjà prêt (store rang UPPER) |
| 2 — Coût réel / ModelProvider d'échec | terminé (voie sans clé) | 0 | **BLOCAGE (une ligne) : aucune clé d'API dans l'environnement — aucun appel réel, aucun coût mesuré, pas d'economie-unitaire.md inventé.** Voie prescrite appliquée : contrat réel `kollega-model::ModelProvider` (faillible, facturé en jetons réels) + `ScriptedProvider` rejouant les 4 modes d'échec (limite de débit, délai à effet inconnu, réponse tronquée FACTURÉE, facture ≠ estimation) + `ApiKey` à Debug/Display expurgés, test de non-fuite (formatage, Debug dérivé d'une config, erreurs) |
| 3 — Types porteurs (a-f) | **a✓ b✓ c✓ d✓ e✓ f✓ — complet** (nuit du 28 au 29/07 : c et f achevés sur autorisation de Micka, chacun vérifié par sabotage) | 1 |
| 3 (détail v1, conservé pour l'historique) | — | 0 | a : ContentDigest::of seul constructeur + from_storage sous feature + garde textuelle d'atteignabilité. b : Timestamp tronque à la construction (euclidien, saturé), pilote câblé dessus. c : AuditContent avec digest-MÉTHODE fait ; la refonte de ChainedEntry (hash privé) NON faite. d : Intent/Outcome/Abandoned + branché sur le rejeu réel (attestation step_abandoned avant rejeu). e : validateur asymétrique à rapport, table du brief testée ligne à ligne. f : traits append/read et put/read/purge_org + garde anti-retrait ; conformance LITTÉRALE du pilote aux traits à câbler |
| 6 — Approfondissement par sabotage | en cours | 3 | Sabotages VÉRIFIÉS : (a) champs de `ChainedEntry` rendus publics → le doctest `compile_fail` échoue avec « Test compiled successfully, but it's marked compile_fail » ; (b) `DELETE FROM audit_chain` introduit → la garde SQL de la persistance rougit ; (c) filtre de tâche retiré du chargement des effets → CI ROUGE (run 30230062721, `verifications` en échec, branche jetable supprimée) — le test de fuite d'effets sait donc échouer. (d) noyau comptable — `consumed` non mis à jour → 4 tests rougissent dont le proptest de conservation ; (e) validateur de séquence — clôture orpheline rendue légale → 2 tests rougissent. (f) **différentiel Rust↔Python CONCLUANT** : saboté côté PYTHON (hex majuscule) pour isoler, run 30230220434 rouge à l'étape nommément « Différentiel encodage canonique Rust <-> Python », tous les tests Rust verts — l'affirmation « spécification confirmée par deux implémentations indépendantes » est donc fondée. Branche jetable supprimée. Tous les sabotages retirés, workspace revenu au vert à chaque fois ; graine proptest née d'un sabotage supprimée (elle ferait croire à un vrai cas de régression) |
| 4 — Documents | terminé | 0 | ADR-0007 (invariants→types→dépôts→SQL→migration, avec la réserve : le schéma porte en propre ce que les types ne peuvent pas exprimer) ; matrice : rubrique « où il vit » (13 lignes, 3 invariants en prose seulement rendus visibles) ; README : hébergement français et RGPD reformulés en décisions prises NON réalisées (ADR-0001, M7) |
