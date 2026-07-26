# Corrections proposées pour CLAUDE.md — je ne touche pas au fichier

Format : ligne actuelle citée exactement / texte de remplacement / raison en
une phrase. À appliquer par tes soins si tu es d'accord (toute modification
de CLAUDE.md exige un ADR selon son propre en-tête).

## 1. Invariant 4 — « toute altération » surpromet

**Actuel** (§3, invariant 4) :
> **Le journal d'audit est en ajout seul et chaîné par hachage.** Aucune API de modification ni de suppression n'existe. `audit verify` détecte toute altération.

**Proposé** :
> **Le journal d'audit est en ajout seul, chaîné par hachage et ancré.** Aucune API de modification ni de suppression n'existe. `audit verify`, confronté à la dernière ancre publiée, détecte toute altération antérieure à cette ancre ; la fenêtre depuis la dernière ancre est bornée par le rythme de publication.

**Raison** : un hachage chaîné sans ancre ne détecte ni la troncature de
queue ni un suffixe réécrit — la formulation actuelle promet ce que le
dispositif ne tient qu'avec l'ancrage (démontré par tests,
`docs/audit-modele-de-menace.md`).

## 2. Invariant 1 — la forme réelle de pose du contexte

**Actuel** (§3, invariant 1, extrait) :
> Chaque transaction commence par `SET LOCAL app.current_org`.

**Proposé** :
> Chaque transaction commence par `SELECT set_config('app.current_org', $1, true)` — l'équivalent paramétrable de `SET LOCAL`.

**Raison** : la garde textuelle du dépôt interdit tout `SET` littéral (non
paramétrable) ; la constitution doit décrire la forme réellement imposée.

## 3. Table d'architecture — même correction

**Actuel** (§5, ligne « Contexte de tenant ») :
> | Contexte de tenant | `SET LOCAL app.current_org` en début de chaque transaction, via un unique point de passage | Un seul endroit à auditer, un seul endroit à tester. |

**Proposé** :
> | Contexte de tenant | `set_config('app.current_org', $1, true)` en début de chaque transaction, via un unique point de passage | Un seul endroit à auditer, un seul endroit à tester. |

**Raison** : cohérence avec la correction n° 2.

## 4. Invariant 2 — le nom réel du drapeau interdit n'est qu'un exemple

**Actuel** (§3, invariant 2) :
> **Aucun appel d'outil ne s'exécute sans passer par le moteur de politiques.** Pas de contournement, pas de `debug_bypass`.

**Proposé** : inchangé sur le fond — ajouter simplement « (la validation
humaine se demande par `requires_approval` ou un seuil souple ; un
dépassement de limite dure est un refus, jamais un contournement) ».

**Raison** : depuis le BLOC 3 de la nuit du 28/07, la distinction limite
dure / seuil souple existe dans le moteur ; la constitution peut nommer le
comportement attendu pour éviter qu'une session future ne le réinvente
autrement. (Optionnelle — c'est une précision, pas une correction.)
