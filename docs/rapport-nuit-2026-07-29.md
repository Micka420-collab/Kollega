# Rapport de session autonome — nuit du 28 au 29/07/2026

Session en boucle auto-cadencée, sur consigne de Micka (« continue toute la
nuit, mets-toi des rappels, tiens le README à jour »). Onze commits, tous
poussés, tous vérifiés en CI. Environnement inchangé : pas de PostgreSQL
local (l'intégration se prouve en CI), pas de clé d'API.

## 1. Ce qui a été livré

| Priorité du brief | État |
|---|---|
| (1) Idempotence du rejeu | **Faite** — dette n°1 fermée |
| (2) Identité d'appel + pont vers `AuditRecord` | **Faite** — le validateur de séquence n'est plus du code mort |
| (3) Bloc 3c — entrée produite / entrée relue | **Faite** (autorisation explicite de Micka) |
| (4) Bloc 3f — conformance aux dépôts | **Faite** |
| (5) Durcissement CI | **Fait** — actions par SHA, `--locked`, bases par digest |
| (6) Approfondissement par sabotage | **Fait** — six sabotages, tous concluants |

## 2. Les trouvailles — le livrable réel

- **Le trait `ToolRunner` rendait l'idempotence INEXPRIMABLE.** Il ne
  recevait que le nom de l'outil : un exécuteur qui ignore *quel* appel il
  exécute ne peut pas reconnaître un effet déjà réalisé. L'itération est
  entrée dans la signature — ce n'est pas cosmétique, c'est ce qui rend le
  mécanisme possible.
- **Une identité aléatoire aurait tout cassé.** L'idempotence exige une clé
  DÉRIVABLE : `SHA-256(task_id ‖ iteration)`. Un `uuid v4` tiré à chaque
  tentative n'aurait jamais reconnu l'effet précédent. C'est la raison pour
  laquelle la machine porte désormais l'itération dans ses événements.
- **L'idempotence des EFFETS ne donne pas la cohérence des ATTESTATIONS.**
  Trouvée par la CI, sur mon propre commit (run n°32, rouge) : remettre un
  état en arrière après un pas committé fait enregistrer une seconde
  clôture pour le même appel — le journal prétend alors que l'outil s'est
  exécuté deux fois. Ce sont deux garanties distinctes. Assertée, pas
  esquivée : le cas arrivera lors d'une restauration partielle.
- **Appliquer le bloc 3c à la lettre aurait rendu la corruption
  INDÉTECTABLE.** Une entrée relue d'une base corrompue doit pouvoir porter
  un hachage qui ment, sinon `verify` n'a plus rien à dénoncer. D'où deux
  types : l'entrée *produite* ne peut pas mentir (forger ne compile pas),
  l'entrée *relue* le peut.
- **En PostgreSQL, une violation de contrainte AVORTE la transaction
  entière** (`25P02`). Attraper le `23505` « au passage » pour continuer ne
  marche pas : tout ce qui suit échoue. Il faut déclarer À L'AVANCE quel
  conflit est acceptable (`ON CONFLICT … DO NOTHING`, **ciblé** sur la
  contrainte tolérable — sans ciblage on avalerait aussi le conflit de
  hauteur, qui doit rester une erreur). Trouvé par la CI (run n°36), et
  présent à DEUX endroits : l'attestation et l'enregistrement d'effet.
  Aucun test pur ne pouvait le montrer — c'est une propriété du moteur.
- **`ALTER ROLE` n'est PAS sûr en concurrence** (`XX000 — tuple
  concurrently updated`). Deux appels simultanés à `set_app_role_password`
  et l'un échoue. Découvert en CI (run n°43) entre deux tests exécutés en
  parallèle — mais **c'est un défaut de PRODUCTION** : deux instances
  lançant `kollega migrate` lors d'un redéploiement se seraient marché
  dessus. Corrigé par un verrou consultatif transactionnel
  (`pg_advisory_xact_lock`), libéré au commit comme au rollback.
- **Une convention attrapée par clippy** : `items after a test module`. Le
  module de tests doit clore le fichier.
- **La feature `storage-boundary` était du code mort** derrière un drapeau
  que personne n'activait. Elle a maintenant son unique consommateur
  légitime (relire une empreinte persistée sans la recalculer).
- **La colonne `tool_call_id`**, posée à la migration 0003, n'avait jamais
  été remplie. C'est elle qui permet de reconstituer les `AuditRecord`.

## 3. Les six sabotages de vérification

Aucun garde-fou n'a été cru sur parole. Chaque mécanisme a été cassé
volontairement pour vérifier que le test le remarque, puis remis en état.

| Mécanisme | Sabotage | Résultat |
|---|---|---|
| « Forger une entrée ne compile pas » | champs rendus publics | doctest ROUGE — « compiled successfully, but it's marked compile_fail » |
| « Aucun SQL ne retire une preuve » | `DELETE FROM audit_chain` introduit | garde ROUGE, message exact |
| Fuite d'effets entre tâches | filtre de tâche retiré | **CI ROUGE** (run 30230062721) |
| Différentiel Rust ↔ Python | hex majuscule **côté Python** | **CI ROUGE à l'étape nommée**, tous les tests Rust verts — isolation parfaite |
| Noyau comptable | `consumed` non mis à jour | 4 tests ROUGES dont le proptest de conservation |
| Validateur de séquence | clôture orpheline rendue légale | 2 tests ROUGES |

Le quatrième est le plus concluant : en sabotant *côté Python*, aucun test
Rust ne pouvait bouger — seul le différentiel pouvait rougir, et c'est
exactement ce qui s'est passé. L'affirmation « spécification confirmée par
deux implémentations indépendantes » est donc fondée, pas décorative.

## 4. Décisions prises seul — à réexaminer

- **Ordre d'implémentation inversé (2 avant 1)** : une clé d'idempotence
  *par appel* exige que les appels aient une identité. Dépendance
  mécanique, pas changement de périmètre.
- **Durcissement CI avancé avant le bloc 3f** : il renforce l'instrument de
  preuve dont dépendait tout le reste de la nuit.
- **Bases épinglées par digest** — avec la contrepartie écrite : un digest
  jamais remonté est PIRE qu'un tag mobile, il fige une base vulnérable en
  donnant l'apparence de la rigueur. Procédure de bump dans
  `deploy/README-bases.md`. **C'est un engagement de maintenance, à
  assumer ou à annuler.**
- **Version d'enveloppe portée à v2** : le format d'état a réellement
  changé. Une tâche v1 suspendue est refusée proprement.
- **Graine proptest née d'un sabotage supprimée** : elle ferait croire à un
  cas de régression réel.

## 5. Ce qui reste ouvert

- **Le coût réel n'est toujours pas mesuré** (pas de clé d'API). Un seul
  appel suffirait ; `docs/economie-unitaire.md` attend ce chiffre, et c'est
  lui qui sert à fixer un prix.
- ~~Le trait `PolicyEngine` ne transporte que le nom de l'outil.~~
  **CORRIGÉ en fin de nuit** : le trait est SUPPRIMÉ, `drive` appelle
  `kollega_policy::decide` directement avec la requête complète. Les bornes
  à deux étages étaient jusque-là **inertes en production** — testées,
  mais jamais atteintes par un appel réel. Invariant 2 promu dans la
  matrice. Réserve restante, honnête : `ToolRunner` est un trait public,
  rien n'empêche techniquement un appel hors de la boucle (un type témoin
  délivré par le moteur fermerait ce dernier chemin).
- Les décisions produit en attente (modèle M4 « relance client », canal
  expert-comptable) — inchangées, elles t'appartiennent.

## 5 bis. La CI a un taux de faux rouges non nul — observé, pas supposé

Run n°39 verte, run n°41 ROUGE sur le job de réversibilité, run n°42 verte
— **sans qu'aucune migration n'ait changé entre les trois**. L'échec était
donc transitoire (infrastructure). Deux conséquences à retenir :

- un rouge n'est pas toujours un défaut du code, et le réflexe « la CI est
  rouge donc j'ai cassé quelque chose » peut faire chercher au mauvais
  endroit ;
- le diagnostic publié était VIDE ce jour-là (il ne capturait que les
  échecs ciblés). Corrigé : l'étape publie désormais l'intégralité de sa
  sortie. Si cela se reproduit, ce sera lisible.

## 6. Inquiétudes

- ~~La double attestation lors d'une restauration est détectée mais pas
  empêchée.~~ **CORRIGÉ dans la foulée** (migration 0005) : unicité sur
  `(org, tool_call_id, action)`, et l'écriture d'attestation est devenue
  idempotente elle aussi. Le pilote distingue désormais DEUX violations
  d'unicité aux sens opposés — hauteur prise (course d'écrivains → rejeu du
  pas) et attestation déjà présente (pas rejoué → rien à ajouter). Les
  confondre aurait fait rejouer un pas qui n'avait rien à rejouer.
- **`verify_org_sequence` charge toute la chaîne** — comme `verify`. À
  repenser en flux avant des volumes réels.
- Le README affirme beaucoup de choses ; chacune est adossée à un test
  exécuté en CI. Si un jour un de ces tests est désactivé, le README
  deviendra faux en silence. Il n'existe aucun mécanisme qui relie les
  deux.
