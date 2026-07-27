# Rapport de session autonome — nuit du 28 au 29/07/2026

Session en boucle auto-cadencée, sur consigne de Micka (« continue toute la
nuit, mets-toi des rappels, tiens le README à jour »). ~20 commits, tous
poussés, tous vérifiés en CI. Environnement inchangé : pas de PostgreSQL
local (l'intégration se prouve en CI), **pas de clé d'API**.

## 1. L'essentiel en un tableau

**Neuf invariants sur treize sont prouvés par un test exécuté** (contre
sept en début de nuit) :

| Invariant | Avant la nuit | Après |
|---|---|---|
| 2 — aucun appel hors moteur | partiel (nom d'outil seul) | **prouvé** — requête complète, limite dure effective |
| 3 — deux entrées par appel | pur seulement | **prouvé** — validé sur la chaîne persistée |
| 4 — journal ajout seul | pur seulement | **prouvé** — porté par le rôle, double attestation impossible |
| 5 — crédit | pur seulement | **partiel** — atomicité prouvée sous concurrence réelle ; « avant l'appel » toujours faux |
| 6 — plafond de coût | pur seulement | **prouvé** — arrêt propre dans la boucle réelle, statut distinct persisté |
| 12 — aucune suppression physique | aucun test | **prouvé** — porté par le rôle sur les six tables |

Inchangés : 1, 7, 11, 13 (déjà prouvés) ; 8, 9, 10 (jalons M2, M5, M6 non
commencés).

## 2. Ce que l'intégration a révélé — le livrable réel

Sept trouvailles, dont **aucune n'était visible depuis un test pur** :

1. **Le trait `ToolRunner` rendait l'idempotence INEXPRIMABLE.** Il ne
   recevait que le nom de l'outil : un exécuteur qui ignore *quel* appel il
   exécute ne peut pas reconnaître un effet déjà réalisé.
2. **Une identité aléatoire aurait tout cassé.** L'idempotence exige une
   clé DÉRIVABLE — `SHA-256(task_id ‖ iteration)`. Un `uuid v4` par
   tentative n'aurait jamais reconnu l'effet précédent.
3. **L'idempotence des EFFETS ne donne pas la cohérence des
   ATTESTATIONS.** Trouvé par la CI sur mon propre commit : après une
   restauration d'état, le journal enregistrait une seconde clôture — il
   prétendait que l'outil s'était exécuté deux fois. Rendu **impossible**
   (migration 0005), pas seulement détectable.
4. **En PostgreSQL, une violation de contrainte AVORTE la transaction
   entière** (`25P02`). Attraper le `23505` « au passage » ne marche pas :
   tout ce qui suit échoue. Il faut `ON CONFLICT … DO NOTHING` **ciblé** —
   sans ciblage on avalerait aussi le conflit de hauteur, qui doit rester
   une erreur. Présent à **deux** endroits ; le premier m'a fait voir le
   second.
5. **`ALTER ROLE` n'est pas sûr en concurrence** (`XX000 — tuple
   concurrently updated`). Défaut de **production**, pas de test : deux
   instances lançant `kollega migrate` lors d'un redéploiement se
   seraient marché dessus. Corrigé par verrou consultatif transactionnel.
6. **Les bornes à deux étages étaient INERTES.** Testées, mais jamais
   atteintes par un appel réel : la boucle ne transmettait que le nom de
   l'outil. Un agent visant 500 destinataires serait passé.
7. **Le `GRANT DELETE` de la migration 0002** subsistait sur
   `organizations` et `users`, en contradiction avec la constitution.

Plus deux conventions attrapées par l'outillage : `items after a test
module` (clippy) et le module de tests devant clore le fichier.

## 3. Les six sabotages de vérification

Aucun garde-fou n'a été cru sur parole.

| Mécanisme | Sabotage | Résultat |
|---|---|---|
| « Forger une entrée ne compile pas » | champs rendus publics | doctest ROUGE — « compiled successfully, but it's marked compile_fail » |
| « Aucun SQL ne retire une preuve » | `DELETE FROM audit_chain` introduit | garde ROUGE, message exact |
| Fuite d'effets entre tâches | filtre de tâche retiré | **CI ROUGE** |
| Différentiel Rust ↔ Python | hex majuscule **côté Python** | **CI ROUGE à l'étape nommée**, tous les tests Rust verts — isolation parfaite |
| Noyau comptable | `consumed` non mis à jour | 4 tests ROUGES |
| Validateur de séquence | clôture orpheline rendue légale | 2 tests ROUGES |

## 4. Un fait gênant mais vrai : la CI a des faux rouges

Run n°39 verte, n°41 **rouge** sur la réversibilité, n°42 verte — sans
qu'aucune migration n'ait changé. L'échec était transitoire. Deux
conséquences : un rouge n'est pas toujours un défaut du code (le réflexe
inverse fait chercher au mauvais endroit), et le diagnostic publié ce
jour-là était **vide**. Corrigé : les jobs publient désormais
l'intégralité de leur sortie sur des branches lisibles sans jeton
(`ci-diagnostic`, `ci-diagnostic-tests`) — mécanisme qui a ensuite donné
la cause du `25P02` et du `XX000` en une lecture chacun.

## 5. Décisions prises seul — à réexaminer

- Ordre d'implémentation inversé (identité d'appel avant idempotence) :
  dépendance mécanique.
- Durcissement CI avancé avant le bloc 3f : il renforce l'instrument de
  preuve dont dépendait le reste.
- **Bases épinglées par digest** — contrepartie écrite : un digest jamais
  remonté est PIRE qu'un tag mobile. Procédure dans
  `deploy/README-bases.md`. **C'est un engagement de maintenance.**
- Suppression du trait `PolicyEngine` (API publique) — sous l'autorisation
  explicite « tu peux refondre, mais vérifie ».
- Deux types `ChainedEntry` / `StoredEntry` — la consigne littérale du
  bloc 3c aurait rendu la corruption indétectable.
- Format d'enveloppe porté à **v3** (deux changements de forme réels).

## 6. Ce qui reste ouvert et t'appartient

- **Le coût réel n'est pas mesuré** (pas de clé d'API). Un seul appel
  suffirait ; `docs/economie-unitaire.md` attend ce chiffre, et c'est lui
  qui sert à fixer un prix.
- **Invariant 5, « vérifié AVANT l'appel de modèle »** : trois options
  écrites dans `questions-nuit.md`, ma recommandation étant un seuil
  plancher puis une correction de CLAUDE.md par ADR. Non tranché : cela
  engage le produit et la constitution.
- Décisions produit antérieures : modèle M4 « relance client », canal
  expert-comptable.

## 7. Inquiétudes

- **`ToolRunner` reste un trait public** : rien n'empêche techniquement un
  appel hors de la boucle. Un type témoin délivré par le moteur fermerait
  ce dernier chemin de l'invariant 2.
- **L'effacement logique n'est écrit nulle part** : `deleted_at` existe
  dans le schéma, aucun code ne le pose. L'invariant 12 est prouvé du côté
  « on ne supprime pas », pas du côté « on efface proprement ».
- **`verify_org_chain` et `verify_org_sequence` chargent tout en mémoire.**
  À repenser en flux avant des volumes réels.
- Le README affirme beaucoup ; chaque affirmation est adossée à un test
  exécuté en CI. **Aucun mécanisme ne relie les deux** : si un test était
  désactivé, le README deviendrait faux en silence.
