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

2. **Cohérence entre le consommé de la tâche et le solde débité.** En pur,
   `charge` fait les deux d'un coup. En base, la ligne `tasks.cost_cents` et
   la ligne du solde d'organisation sont deux écritures : elles doivent être
   dans la **même transaction** que le débit, sinon un crash entre les deux
   laisse un écart.

3. **La reprise après interruption.** Une tâche suspendue puis reprise (jalon
   suivant) doit reconstruire son `Budget` depuis l'état persistant
   (`tasks.cost_cents`, solde courant de l'organisation) — jamais depuis un
   état en mémoire. Le noyau pur est déjà entièrement reconstructible
   (`Budget::new` + relecture), c'est la couture nécessaire.

4. **La file d'attente.** `SELECT … FOR UPDATE SKIP LOCKED` sur `tasks`
   distribue le travail entre workers ; chaque worker évalue le budget dans
   sa propre transaction, sous le verrou de solde ci-dessus.

## Résumé pour le développeur du jalon M3

Le noyau pur est la RÈGLE ; la base est l'EXÉCUTION CONCURRENTE de cette
règle. Ne jamais dupliquer la décision comptable en SQL : lire l'état, la
transaction verrouille, appeler `Budget::charge`, persister le résultat dans
la même transaction. Le seul comportement que la base AJOUTE est la
sérialisation des accès concurrents au solde partagé.
