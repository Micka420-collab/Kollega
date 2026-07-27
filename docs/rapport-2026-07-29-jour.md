# Rapport — journée du 29/07/2026 (session autonome en boucle)

Ce document dit ce qui a été cherché, ce qui a été trouvé, ce que j'ai
cassé, et ce qui reste. Il ne remplace pas `docs/etat-session.md`, qui est
le journal courant ; il en donne la lecture d'ensemble.

## Ce que la journée a cherché — et pourquoi

**Aucune fonctionnalité n'a été ajoutée, délibérément.** Le brief initial
était fait, et la question qui restait n'était plus « que construire ? »
mais « ce qui est construit tient-il vraiment ? ». À ce stade, le risque
n'est plus d'écrire du code faux : c'est de **croire un tableau vert**.

Trois campagnes, dans cet ordre :

1. Falsifier chaque preuve — la voir ROUGE, ou ne pas la croire.
2. Attaquer les garde-fous eux-mêmes, en cherchant à les contourner.
3. Confronter la documentation, le schéma et les chemins d'erreur au code.

## Ce que ça a trouvé

### Toute preuve existante a été vue échouer

Chaque invariant marqué prouvé a été falsifié exprès. Le détail est dans
la rubrique « Sensibilité » de `docs/matrice-invariants.md`. Deux
enseignements plus durables que la liste elle-même :

- **Un test qui n'a pas tourné ressemble en tout point à un test qui n'a
  rien vu.** Un premier essai a conclu qu'une preuve était insensible ;
  en réalité la CI s'était arrêtée avant de l'exécuter. `--no-fail-fast`
  est désormais posé, non par confort de diagnostic, mais parce que sans
  lui **un sabotage peut mentir dans le sens rassurant** — le pire des
  deux.
- **Une sensibilité démontrée sur un cas ne se transporte pas aux
  autres.** Le sabotage de l'invariant 12 prouvait que le test savait
  échouer *pour `users`* ; il ne disait rien des cinq autres tables.

### Sept des dix garde-fous avaient un trou

Dix trous au total, alors que tous avaient été « vérifiés par sabotage ».
Il n'y a pas de contradiction : chacun avait été saboté **sous la forme
exacte qu'il était écrit pour reconnaître**. La vérification était
circulaire.

Le plus grave était ancien et touchait l'invariant 1 : trois écritures
équivalentes (`FALSE`, `'f'`, un argument paramétré) posaient le contexte
d'organisation en portée **session**, où il survit au retour de la
connexion dans le pool — la requête suivante, celle d'un autre client, se
serait exécutée sous le contexte du précédent, et la RLS aurait servi les
données du mauvais tenant en faisant exactement ce qu'on lui demandait.

La leçon vaut au-delà de ce dépôt : **la question utile n'est pas « est-ce
que ça rougit quand je casse la chose ? » mais « comment ferais-je pour
passer au travers ? »**. Et : une garde peut n'être complète qu'EN
COMBINAISON avec une autre, auquel cas ce couplage doit être écrit.

### Le vert pouvait être un vert de complaisance

Sept tests exigent une base réelle et se sautaient sans elle. Cette base
ne leur était fournie que par **une ligne** de `ci.yml` : la retirer aurait
rendu sept tests verts en ne prouvant plus rien, pendant que six lignes de
la matrice auraient continué d'afficher « OUI — CI ». Se sauter en
intégration continue est maintenant un échec.

Dans le même esprit, cinq `crate_compiles() {}` — corps vide — comptaient
comme des tests. Le nombre affiché au README a **baissé** en les
supprimant, et c'est un progrès : un chiffre qu'on gonfle est un chiffre
auquel on ne peut plus se fier.

### La documentation affirmait des choses fausses

- Une colonne nommée « tasks.cost_cents » qui **n'a jamais existé**, pièce
  centrale de deux exigences de `credits-concurrence.md`. (Sans accents
  graves ici : ils annoncent un identifiant réel, et une garde le vérifie —
  elle a d'ailleurs rougi sur ce paragraphe même.)
- Une table nommée « audit_log » qui n'a jamais existé non plus (c'est
  `audit_chain` + `audit_content`).
- Deux ADR portant dans leur titre le **numéro d'un autre ADR**.
- Trois références d'invariant périmées de la numérotation v1 — la
  réversibilité rattachée au 12 au lieu du 13, l'absence d'entrée-sortie
  au 9 au lieu du 11.

Ce n'est pas cosmétique : un document de conception est ce qu'on suit en
reprenant le travail six mois plus tard. Nommer une colonne absente invite
à l'ajouter en croyant réparer un oubli ; citer le mauvais invariant envoie
lire la mauvaise règle. Quatre gardes en sont nées (colonnes, noms de
tables, numéros d'ADR, couverture de la matrice par la constitution).

### Le schéma se défendait sans qu'on le sache

**Aucun test n'essayait jamais de violer une contrainte.** Les tests
prouvaient que l'application ne produit pas d'état interdit ; aucun ne
prouvait que la base le refuserait si l'application s'égarait. Or
`CHECK (balance_cents >= 0)` est la dernière ligne de défense de
l'invariant 5 : le retirer aurait laissé le test de concurrence vert,
puisque celui-ci éprouve le verrou et non la contrainte.

Deux cas méritent d'être retenus pour leur intention, invisible dans le
code : l'unicité d'email est **par organisation** et délibérément pas
globale — une unicité globale divulguerait à un client l'existence d'un
autre ; et l'unicité `(org_id, task_id, iteration)` est un filet
**indépendant de la dérivation** d'identité, qu'aucun test existant ne
pouvait atteindre puisqu'ils passent tous par cette dérivation.

### Des tests verts pour de mauvaises raisons

Plusieurs se contentaient d'un `is_err()`. La seule preuve de
l'invariant 12 en faisait partie : elle aurait été verte sur une faute de
frappe, une transaction déjà avortée, ou un refus de clé étrangère — ce
dernier cas n'ayant rien de théorique pour `organizations`, dont dépendent
quatre tables. Le code d'erreur PostgreSQL est désormais exigé nommément.

### Le code dégradait en silence

- `now_micros` écrivait `.unwrap_or(0)` : une horloge antérieure à l'époque
  Unix scellait dans la chaîne d'audit une date valant **1970**, cohérente
  et fausse. L'erreur de `duration_since` porte pourtant l'écart ; il est
  repris en négatif, et l'anomalie devient lisible.
- La hauteur de chaîne était écrêtée : le stocké aurait divergé du haché.
- **Après une purge RGPD, un rejeu doit refuser** — la migration 0004
  l'affirme, rien ne le vérifiait. L'alternative aurait été d'envoyer un
  second mail au client d'une organisation qui vient d'exercer son droit à
  l'effacement.

## Ce que j'ai cassé, et rattrapé

Trois runs rouges viennent de moi, et l'historique les garde :

1. Un fichier non formaté poussé, parce que ma chaîne de commandes
   n'interrompait pas sur échec. Corrigé, et la règle est maintenant
   explicite.
2. Un test qui échouait sur base réelle — mes sections précédentes font
   délibérément avorter leurs transactions, si bien qu'aucune ligne de
   crédit ne subsistait pour la suite.
3. **Mes propres gardes m'ont attrapé trois fois** : deux fois en écrivant
   de la documentation qui *parlait* d'un identifiant inexistant, une fois
   en écrivant un motif de recherche que la garde anti-saut traque. La
   troisième s'est produite **en rédigeant le point qui raconte la
   première**. Un commit « docs » n'est pas plus sûr qu'un autre dès lors
   que des gardes lisent la documentation.

## Ce qui reste — et qui t'appartient

Rien de ce qui suit ne peut avancer sans toi.

1. **Le coût réel.** Une clé d'API, un seul appel sur le modèle le moins
   cher. C'est ce chiffre qui fixe un prix, et rien d'autre ne le débloque.
2. **`kollega-model` n'est branché nulle part.** Le contrat réel existe et
   aucun chemin d'exécution ne l'atteint. Deux conséquences : l'invariant 7
   n'a, en aval de l'assemblage, aucun chemin réel à protéger ; et
   l'invariant 5 ne deviendra « vérifié AVANT » que quand la boucle
   recevra l'estimation que `ModelRequest` porte déjà. Le branchement
   engage la conception de la boucle d'agent (M3/M4).
3. **L'effacement logique.** `deleted_at` existe dans le schéma, aucun code
   ne le pose. Écrire un `soft_delete` sans savoir qui efface quoi et
   depuis où serait spéculatif.
4. **Décisions produit** : modèle M4 « relance client », canal
   expert-comptable, engagement de maintenance des digests de base.

Deux variantes d'erreur restent non produites par un test, et je les nomme
plutôt que de les taire : `ChainConflict` (il faudrait épuiser les trois
rejeux de hauteur) et `Accounting` (simple report d'une erreur de budget
déjà couverte à sa source).
