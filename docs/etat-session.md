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
