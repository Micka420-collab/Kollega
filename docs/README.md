# Index de `docs/`

Dix-sept documents, dont six rapports de session. Sans cette page, on ne
sait pas lequel **fait autorité** et lequel est un instantané daté — et
c'est le genre d'ambiguïté qui fait suivre une décision périmée.

Trois catégories, et une seule d'entre elles est à jour par construction.

## Fait autorité — à lire, à tenir à jour

| Document | Ce qu'il dit | Ce qui le tient |
|---|---|---|
| [`matrice-invariants.md`](matrice-invariants.md) | Pour chacun des treize invariants : quel test le soutient, a-t-il **tourné**, a-t-il été vu **rouge**. | Trois gardes : les tests cités existent, les colonnes et tables nommées existent, la matrice couvre exactement les invariants de `CLAUDE.md`. |
| [`adr/`](adr/) | Les décisions d'architecture, et pourquoi les alternatives ont été écartées. | Une garde vérifie que le titre d'un ADR porte le numéro de son fichier. |
| [`encodage-canonique.md`](encodage-canonique.md) | La spécification des octets hachés. **C'est LA source** : Rust et Python doivent tous deux en découler, jamais l'un de l'autre. | Différentiel Rust ↔ Python en CI, 14 014 vecteurs. Vérifiée intégralement contre le code le 29/07. |
| [`audit-modele-de-menace.md`](audit-modele-de-menace.md) | Ce que la chaîne d'audit garantit, contre qui — et ce qu'elle ne garantit **pas**. | Lecture destinée à un auditeur. |
| [`invariant-7-modele-de-menace.md`](invariant-7-modele-de-menace.md) | Confinement du contenu externe : ce que l'assemblage garantit, et ce qu'il transfère à l'aval. | Corpus de 34 cas, dont le nombre est asserté dans le test. |
| [`credits-concurrence.md`](credits-concurrence.md) | Ce que le noyau comptable pur ne peut pas garantir seul, et ce que la base doit faire. | Relu contre le code le 29/07 : trois exigences sur quatre tenues, la file d'attente non commencée. |
| [`jalons.md`](jalons.md) | Le découpage M0 → M7 et les définitions de terminé. | — |
| [`questions-nuit.md`](questions-nuit.md) | **Ce qui attend une décision du propriétaire**, et les choix réversibles pris seul. | À lire avant de reprendre le travail. |
| [`backlog.md`](backlog.md) | Ce qu'on refuse de coder tant que trois clients payants ne l'ont pas demandé. | — |
| [`etat-session.md`](etat-session.md) | Le journal courant : où en est le travail, ce qui vient de se faire. | Mis à jour à chaque bloc. |

## Instantanés datés — historiques, jamais mis à jour

Ils disent ce qui était vrai **ce jour-là**. Ne pas s'en servir pour
connaître l'état actuel : c'est le rôle de la matrice et du README.

- [`rapport-2026-07-29-jour.md`](rapport-2026-07-29-jour.md) — le plus
  récent et le plus utile : la campagne de vérification (falsification des
  preuves, attaque des gardes, documentation confrontée au code).
- [`rapport-nuit-2026-07-29.md`](rapport-nuit-2026-07-29.md),
  [`rapport-nuit-2026-07-28-jour-2.md`](rapport-nuit-2026-07-28-jour-2.md),
  [`rapport-nuit-2026-07-28-jour.md`](rapport-nuit-2026-07-28-jour.md),
  [`rapport-nuit-2026-07-28.md`](rapport-nuit-2026-07-28.md),
  [`rapport-nuit-2026-07-27.md`](rapport-nuit-2026-07-27.md) — sessions
  antérieures, par ordre décroissant.

## Hypothèses et propositions — à confronter, pas à appliquer

- [`methode-de-travail.md`](methode-de-travail.md) — la délégation par
  paliers de confiance. **Écrit sans avoir parlé à un seul dirigeant** ;
  le document le dit lui-même en tête. Sa fonction est d'être assez précis
  pour être **réfuté** en entretien.
- [`taches-delegables-analyse.md`](taches-delegables-analyse.md) — analyse
  produit versée au dépôt, recommandations non appliquées.
- [`claude-md-corrections-proposees.md`](claude-md-corrections-proposees.md)
  — corrections proposées à la constitution. `CLAUDE.md` ne se modifie que
  par décision du propriétaire ; ce fichier existe pour que les
  imprécisions relevées ne se perdent pas.
