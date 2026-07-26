# Rapport de session autonome — nuit du 27 au 28/07/2026

Environnement : **pas de base PostgreSQL** (port 5432 fermé, pas de psql),
**pas de Python**, **pas de dépôt distant**. Surface pure uniquement, commits
locaux. 13 commits cette nuit. `cargo test --workspace`,
`clippy -D warnings`, `fmt --check` verts avant chaque commit.

## 1. Ce que la session précédente avait laissé en suspens — et ce que j'en ai fait

- **M0 non prouvé** (test d'isolation RLS jamais exécuté) → **toujours non
  prouvé** : aucune base cette nuit. Rien à faire sans PostgreSQL ; consigné,
  non bricolé.
- **Épinglage argon2 par liste blanche** (défaut identifié dans le brief) →
  **corrigé** (BLOC 2, ADR-0006).
- **Dépassement de seuil = refus** (défaut produit) → **corrigé** (BLOC 3).
- **Ancrage de la chaîne d'audit** (question ouverte) → **tranché en pur**
  (BLOC 5, défaut révocable : double témoin, dont remise au client).
- **Deux corrections de CLAUDE.md** → **proposées** sans toucher au fichier
  (BLOC 13, `docs/claude-md-corrections-proposees.md`, 4 corrections).

## 2. Blocs — sans arrondir

| Bloc | État | Tours d'approfondissement |
|---|---|---|
| 0 Reprise | terminé | — |
| 1 Prouver M0 | **abandonné** (pas de base) | 0 |
| 2 Épinglage argon2 → bornes + re-hachage | terminé | 1 |
| 3 Limite dure / seuil souple | terminé | 1 |
| 4 Hauteur dans la charge hachée | terminé | 1 |
| 5 Ancre de chaîne (pur) | terminé | 1 |
| 6 Assemblage des segments (inv. 7) | terminé | 1 |
| 7 Injectivité de l'encodeur | terminé | 1 |
| 8 Propriétés proptest | terminé | 1 |
| 9 Plafond/crédit (pur) | terminé | 1 |
| 10 Machine à états d'agent | terminé | 0 |
| 11 Matrice invariant→test | terminé | 0 |
| 12 Méthode de travail | terminé | 0 |
| 13 Corrections CLAUDE.md | terminé | 0 |

Un seul bloc abandonné : le 1, faute de base. Les blocs 11-13 sont des
documents (pas de boucle d'approfondissement applicable).

## 3. Approfondissement — ce que chaque tour a trouvé

Chaque bloc de code a subi un tour (trois façons d'être faux + entrée
hostile). Trouvailles réelles, toutes corrigées AVANT commit :

- **BLOC 2** : un mauvais mot de passe sur un profil ancien devait rester
  `Invalid`, pas `ValidNeedsRehash` — vérifié par test dédié. Refus d'une
  chaîne `m=4 Gio` chronométré pour prouver l'absence d'allocation.
- **BLOC 3** : une limite dure doit l'emporter sur un seuil souple simultané ;
  les violations de protocole (`\`, `..`, non-déclaré) restent des refus quel
  que soit le mode. Testé.
- **BLOC 4** : déplacement d'entrée détecté que la hauteur soit conservée
  (rupture de lien) OU réécrite (empreinte fausse) — deux tests.
- **BLOC 5** : la **fenêtre d'ancrage** (ce qui suit la dernière ancre n'est
  pas protégé) est démontrée par un test, pas cachée.
- **BLOC 9** : **un coût qui déborderait i64 doit compter comme dépassement
  de plafond, pas comme erreur** — piège attrapé par mon propre test, corrigé.
- **BLOC 8** : le garde-fou du graphe a **attrapé mon ajout de `proptest`**
  en dev-dependency de `kollega-core` — le tripwire renforcé la nuit
  précédente a fonctionné sur moi. `proptest` (pur) ajouté à la liste blanche
  de test, décision consignée.
- Deux propriétés du BLOC 8 étaient **mal posées** (supprimer/insérer en fin
  de chaîne est légitime, pas une corruption) — restreintes aux mutations
  internes, la troncature de queue relevant de l'ancre.

## 4. Ce qui est PROUVÉ par `cargo test` (sans base)

Total ~140 tests + proptests. Par crate :

- **kollega-core** : types du domaine, identifiants non interchangeables
  (doctest compile_fail), Email ASCII, **assemblage des segments** (corpus
  adversarial de 35 injections : le contenu externe n'atteint jamais les
  champs d'instruction ; aucun caractère de manipulation ne survit),
  propriétés (Cents = arithmétique i128, normalisation idempotente).
- **kollega-audit** : chaînage par organisation, **injectivité de l'encodeur**
  (round-trip proptest 4000 cas + chasse au séparateur, aucune collision),
  détection à la bonne position sous mutation générée, **ancre** (troncature
  de 1/N/tout, suffixe réécrit), 5 vecteurs de référence dont V1 recoupé hors
  Rust.
- **kollega-policy** : refus par défaut, limite dure vs seuil souple sur
  chaque borne, déterminisme, monotonie (ajout de règles ne renverse pas un
  match).
- **kollega-api::auth** : argon2id, **vérification bornée** (plancher/plafond,
  re-hachage), erreurs sans écho du mot de passe.
- **kollega-runtime** : **budget** (solde ≥ 0, conservation des débits, refus
  avant facturation) et **machine à états** (6 scénarios, dont **reprise après
  sérialisation JSON identique au parcours direct** — preuve du « rien en
  mémoire »).
- **Gardes** : graphe de dépendances (toutes sections, `[target.*]` compris),
  garde textuelle `set_config`.

## 5. Écrit mais NON VÉRIFIÉ

- `rls_isolation.rs`, `rls_structural.rs` — exigent PostgreSQL. **M0 reste
  non prouvé.**
- `.github/workflows/ci.yml` — jamais exécuté (pas de remote).
- `tools/reference/canonical.py` — **Python absent**, jamais exécuté ; écrit
  depuis la spec pour un futur test différentiel ≥ 10 000 vecteurs.
- Migrations `.down.sql` — réversibilité écrite, jamais testée.
→ Tout se prouve au premier push GitHub (la CI provisionne PostgreSQL) ou
avec podman/WSL2 local (droits admin, hors autonomie).

## 6. Défauts trouvés dans le code des sessions PRÉCÉDENTES

Regardé avec un œil hostile — la relecture par le même modèle attrape
l'inattention, pas l'angle mort partagé, donc je cite où j'ai regardé :

- **argon2 (nuit du 27)** : la liste blanche de profils était un faux
  dispositif de sécurité (elle ne ferme aucun chemin réel et verrouille le
  parc au premier durcissement). Corrigé ce soir (ADR-0006). C'est le défaut
  le plus net d'une session précédente.
- **Politiques (nuit du 27)** : le « tout dépassement = refus » était une
  décision produit par défaut, contraire à la promesse. Corrigé (BLOC 3).
- **Chaîne d'audit (nuit du 27)** : la genèse et l'absence de hauteur
  laissaient deux angles (séparation genèse/chaîné computationnelle ;
  rejeu d'entrée à une autre position). Durcis (32 octets zéro déjà fait le
  27 ; hauteur ce soir).
- **Où je n'ai RIEN trouvé** : le point de passage unique `kollega-store`
  (relu, cohérent) ; l'encodeur canonique (prouvé injectif, pas seulement
  relu) ; les types de `kollega-core` (formes sérialisées verrouillées). Je
  ne garantis pas qu'ils sont sans défaut — je dis que ni la relecture ni les
  propriétés n'en ont révélé.

## 7. Décisions prises seul — à réexaminer

Toutes dans `docs/questions-nuit.md` (mis à jour). Les plus engageantes :
- **ADR-0006** (vérification des mots de passe) : c'est une décision
  d'architecture que j'ai prise seul parce que le brief la demandait
  explicitement. À valider.
- **Ancre au client** (BLOC 5) : défaut « révocable », mais c'est un
  engagement produit (le client devient témoin). À confirmer.
- **Format de hachage v3** (hauteur ajoutée) : les vecteurs sont caducs.
  **Dernière fenêtre gratuite** avant qu'une chaîne existe en production.
- **Bornes argon2** (8 Mio / 256 Mio) : chiffres à valider.

## 8. Ce que je ferais en premier à ta place

1. **Créer le dépôt GitHub et pousser.** Tout le NON VÉRIFIÉ (section 5), et
   surtout l'invariant 1, se prouve ou casse au premier passage CI. C'est la
   plus grosse incertitude du projet et c'est dix minutes.
2. Lire `docs/matrice-invariants.md` : elle dit l'état réel, sans
   complaisance.
3. Trancher les décisions de la section 7 pendant qu'elles sont encore
   réversibles — surtout le format de hachage.
4. Appliquer (ou rejeter) les 4 corrections de
   `docs/claude-md-corrections-proposees.md`.

## 9. Inquiétudes, même vagues

- **L'invariant 1 n'a jamais tourné.** C'est le cœur du produit (isolation
  multi-tenant) et il repose aujourd'hui sur une relecture. Tant que la CI ne
  l'a pas exécuté, « les données ne fuient pas entre clients » est une
  intention, pas une preuve. C'est la dette qui m'inquiète le plus.
- **La machine à états (BLOC 10) est un modèle réduit.** Elle prouve la
  reprise et les six scénarios en PUR, mais la vraie boucle devra brancher le
  journal d'audit chaîné (elle a son propre `AuditEvent`, à réconcilier avec
  `kollega-audit`), le vrai moteur de politiques (`kollega-policy`, pas le
  trait local), et la persistance. Ces coutures existent mais ne sont pas
  cousues — ne pas prendre le BLOC 10 pour la boucle de production.
- **Le corpus d'injection (BLOC 6) est fini.** Il prouve que l'assemblage
  tient sur 35 techniques connues ; il ne prouve rien sur la 36ᵉ. Et
  l'assemblage ne protège pas de l'obéissance du modèle — seulement de la
  confusion structurelle. C'est écrit dans le modèle de menace, mais ça vaut
  d'être répété : la vraie barrière reste la politique + la validation.
- **La référence Python n'a jamais tourné.** Si elle diverge du Rust une fois
  exécutée, c'est un défaut de spécification à traiter — je n'ai pas pu le
  vérifier, donc l'injectivité est prouvée côté Rust seulement (round-trip),
  pas encore de façon différentielle et indépendante.
- **Fiabilité** : je me suis arrêté après le BLOC 10 avec les 13 blocs faits
  et verts ; je ne ressens pas de baisse, mais la prudence commande de clore
  ici plutôt que de rouvrir des blocs déjà solides pour un gain marginal.
