# Crédits et concurrence — cahier des charges du jalon M3

Le noyau comptable (`kollega-runtime::budget`) est **pur** : il décide, sur
un état donné, si un appel se facture, franchit le plafond de tâche ou épuise
le crédit. Ce qu'il ne peut PAS garantir seul, et que la base devra garantir.

## Ce que le noyau pur garantit (prouvé par `cargo test`)

- Un appel qui franchirait le plafond de tâche ou le crédit est refusé
  **avant** d'être facturé (aucun débit en cas de refus).
- Sur toute séquence de coûts, le solde reste ≥ 0 et la somme des débits
  égale exactement la variation du solde (propriété proptest, 2000 cas).
- Atteindre exactement une borne est permis ; la dépasser ne l'est jamais.
- Un coût négatif est une erreur d'usage, jamais un crédit déguisé.

## Ce qui ne peut PAS être garanti en pur — à la charge de la base

> **Relu contre le code le 29/07/2026.** Trois des quatre exigences
> ci-dessous sont tenues ; la quatrième n'est pas commencée. Le document
> décrivait par ailleurs une colonne « tasks.cost_cents » **qui n'a jamais
> existé** (sans accents graves : ils annoncent un identifiant réel, et une
> garde le vérifie désormais — `schema_claims.rs`) :
> le consommé de la tâche vit dans l'enveloppe `tasks.state` (JSONB), avec
> le reste de l'état. L'exigence de fond — consommé et solde écrits
> ensemble — est tenue, mais par ce mécanisme-là. Corrigé ci-dessous.

1. **Atomicité du débit du solde d'organisation.** Le solde est partagé
   entre TOUTES les tâches d'une organisation qui tournent en parallèle.
   Deux tâches qui lisent `org_balance = 100`, décident chacune de facturer
   80, et débitent, laisseraient le solde à −60 : invariant 5 violé. Le
   noyau pur ne voit qu'un état à la fois.
   → **Exigence M3** : le débit du solde est une transaction PostgreSQL avec
   verrou de ligne (`SELECT … FOR UPDATE` sur la ligne du solde de
   l'organisation, ou `UPDATE … SET balance = balance − :cost WHERE balance
   >= :cost` avec vérification du nombre de lignes affectées). Un débit qui
   ne « prend » pas la ligne à 1 doit être traité comme
   [`SpendDecision::AbortedCredit`].
   → **Test de concurrence obligatoire (M3)** : deux tâches concurrentes ne
   peuvent pas faire passer le solde sous zéro. C'est un test d'intégration
   sur base réelle, hors périmètre de la surface pure.
   → **TENU (29/07).** La première option a été retenue : `SELECT
   balance_cents … FOR UPDATE` dans la transaction du pas, puis
   `Budget::refreshed`. L'écriture est ensuite un `UPDATE credits SET
   balance_cents = $2` — une valeur ABSOLUE calculée en Rust, ce qui n'est
   correct QUE parce que le verrou a été pris avant la lecture. Quiconque
   retirerait le `FOR UPDATE` en jugeant l'écriture inoffensive rouvrirait
   la course. Prouvé par `two_concurrent_tasks_never_overdraw_the_credit`,
   et sa sensibilité l'est aussi : retirer `refreshed` fait aboutir les
   deux tâches (CI n°68 rouge).

2. **Cohérence entre le consommé de la tâche et le solde débité.** En pur,
   `charge` fait les deux d'un coup. En base, ce sont deux écritures : elles
   doivent être dans la **même transaction** que le débit, sinon un crash
   entre les deux laisse un écart.
   → **TENU (29/07).** `UPDATE tasks SET state = …` et `UPDATE credits SET
   balance_cents = …` sont émis dans la même transaction, validée une seule
   fois. Le consommé n'est PAS une colonne (« tasks.cost_cents » n'a jamais
   existé) : il vit dans `Budget`, sérialisé au sein de l'enveloppe
   `tasks.state`. Conséquence à connaître : toute lecture du consommé passe
   par la désérialisation de l'état, jamais par du SQL — et un futur
   tableau de bord qui voudrait agréger les coûts devra soit lire du JSONB,
   soit se voir ajouter une colonne, décision non prise.

3. **La reprise après interruption.** Une tâche suspendue puis reprise doit
   reconstruire son `Budget` depuis l'état persistant — jamais depuis un
   état en mémoire.
   → **TENU (29/07).** L'état est relu à chaque pas depuis `tasks.state`, et
   le solde d'organisation N'EST PAS repris de l'instantané sérialisé : il
   est relu et verrouillé en base, puis réinjecté par `Budget::refreshed`.
   C'est précisément ce que le point 1 exige, et ce que le sabotage de la
   CI n°68 a confirmé nécessaire.

4. **La file d'attente.** `SELECT … FOR UPDATE SKIP LOCKED` sur `tasks`
   distribue le travail entre workers ; chaque worker évalue le budget dans
   sa propre transaction, sous le verrou de solde ci-dessus.
   → **NON COMMENCÉ.** `SKIP LOCKED` n'apparaît nulle part dans le dépôt.
   Rien ne distribue de travail aujourd'hui : les pas sont déclenchés
   explicitement par l'appelant. La décision figure dans CLAUDE.md §5 ;
   elle reste à construire.

## Résumé pour le développeur du jalon M3

Le noyau pur est la RÈGLE ; la base est l'EXÉCUTION CONCURRENTE de cette
règle. Ne jamais dupliquer la décision comptable en SQL : lire l'état, la
transaction verrouille, appeler `Budget::charge`, persister le résultat dans
la même transaction. Le seul comportement que la base AJOUTE est la
sérialisation des accès concurrents au solde partagé.
