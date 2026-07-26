# Rapport de session autonome — nuit du 26 au 27/07/2026

Pas de dépôt distant (`git remote -v` vide au démarrage) : commits locaux
uniquement, 9 commits cette nuit, historique lisible par `git log --oneline`.
`CLAUDE.md` non modifié (une retouche de fin de fichier faite par toi avant la
session est restée non commitée, volontairement).

## 1. Blocs — sans arrondir

| Bloc | État |
|---|---|
| 1 — Types M1 (core) | **Terminé et prouvé** |
| 2 — Chaînage d'audit pur | **Terminé et prouvé** (le plus solide de la nuit) |
| 3 — Politiques pures | **Terminé et prouvé** |
| 4 — argon2id | **Terminé et prouvé** |
| 5 — Artefacts non vérifiables | **Écrit** : garde textuelle PROUVÉE ; filet structurel RLS et ci.yml NON VÉRIFIÉS |
| 6 — ADR authentification | **Terminé** (en 0005, pas 0003 — numéro pris ; voir questions-nuit) |

En plus du brief : une revue adversariale de 11 agents sur les blocs 1-4 a
produit 7 constats, tous confirmés par contre-expertise, tous corrigés et
re-testés (commit `fix(securite)` final). Détail en section 6 — c'est
important : plusieurs étaient de vrais trous.

## 2. Ce qui est PROUVÉ par `cargo test --workspace` (vert, avec clippy -D warnings et fmt)

**kollega-core (28 tests + 1 doctest)**
- Les six types validés : formes sérialisées figées (statuts, classification,
  décisions), rejet des flottants pour l'argent, plafonds stricts.
- Identifiants typés : la confusion OrgId/UserId **ne compile pas** (doctest
  `compile_fail`), sérialisation en UUID nu.
- Email : ASCII strict, normalisation minuscules, re-validation à la
  désérialisation, bornes 64/254 octets exactes, refus des homoglyphes
  Unicode (cyrillique, chiffres arabes, emoji), erreurs sans écho de
  l'adresse.

**kollega-audit (18 tests unitaires + vecteurs)**
- Encodeur canonique manuel : échappement figé, clés triées, entiers extrêmes.
- Chaîne : détection d'altération (position exacte), de réordonnancement, de
  suppression au milieu, de troncature de tête ; chaîne de A invalide vue
  comme B (l'organisation est dans les octets hachés — mélange impossible
  par le type).
- `verify_with_tail` : troncature de queue et réécriture complète d'un
  suffixe détectées contre une ancre — et deux tests documentent honnêtement
  que `verify` seul NE les détecte PAS.
- **5 vecteurs de référence figés**, V1 recoupé par un SHA-256 calculé hors
  Rust (PowerShell/.NET) sur les octets spécifiés : l'implémentation et la
  spécification coïncident, et toute dérive future de l'encodage cassera
  bruyamment.

**kollega-policy (18 tests)**
- Refus par défaut (sans règle, sans règles du tout, outil interdit).
- Tables sous/au/au-delà du seuil pour montant et destinataires ; non-déclaré
  sous borne = refus ; chemins par segments (`/data` ne couvre pas
  `/data-autre`), traversée `..` et antislash refusés, chemins non déclarés
  sous restriction refusés ; raison jamais vide, identique dans la variante
  du domaine.

**kollega-api::auth (9 tests)**
- Aller-retour argon2id, rejet du mauvais mot de passe, format PHC exact,
  sels distincts, corruption = erreur (y compris PHC sans digest),
  **rétrogradation refusée** (chaînes argon2i et m=8 réelles rejetées),
  erreurs sans écho du mot de passe.

**Gardes du dépôt**
- Graphe de dépendances : liste blanche de kollega-core appliquée à TOUTES
  les sections, y compris `[target.*]` (contournement par cfg() fermé).
- Garde textuelle : la seule pose de contexte admise est
  `set_config('app.current_org', $1, true)` ; tout `SET` sans `LOCAL`
  fera échouer le test.

## 3. Écrit mais NON VÉRIFIÉ, et ce qu'il faut pour vérifier

- `crates/kollega-store/tests/rls_structural.rs` — balaie les catalogues et
  exige RLS+FORCE+politique sur toute table à `org_id` ; liste blanche vide.
  NON VÉRIFIÉ : exige PostgreSQL. → premier passage CI.
- `crates/kollega-store/tests/rls_isolation.rs` (écrit à la session
  précédente) — **n'a jamais tourné**. Le jalon M0 n'est PAS prouvé.
- `.github/workflows/ci.yml` — jamais exécuté (pas de remote).
- `docs/adr/0005` — la migration `login_identities` n'est volontairement PAS
  écrite (invérifiable sans base).
→ Vérification de tout ce qui précède : pousser sur GitHub (la CI provisionne
PostgreSQL+pgvector), ou installer podman/WSL2 localement (droits admin).

## 4. Questions rencontrées

Toutes consignées dans `docs/questions-nuit.md` : 9 choix réversibles pris en
mode conservateur (numéro d'ADR 0005, règles Email ASCII, sémantique des
seuils, emplacement de auth, horodatage en microsecondes, genèse à 32 octets
zéro, périmètre haché, contrat CI inchangé, découpage en modules), et 2
questions non tranchées qui t'appartiennent (ancrage de la chaîne d'audit ;
deux imprécisions de CLAUDE.md que je n'ai pas le droit de corriger).
Aucun bloc abandonné.

## 5. Ce que je ferais en premier à ta place au réveil

1. **Créer le dépôt GitHub et pousser.** Tout le non-vérifié de la section 3
   se prouve (ou casse) au premier passage de CI — c'est la plus grosse
   incertitude restante, et c'est dix minutes.
2. Lire `docs/questions-nuit.md` et invalider ce qui te déplaît pendant que
   c'est encore à cinq minutes de correction.
3. Trancher l'ancrage de la chaîne d'audit (où vit l'empreinte de queue de
   confiance) avant d'écrire le sink PostgreSQL.
4. Corriger les deux formulations de CLAUDE.md (invariant 4 « toute
   altération » → « toute altération, ancre de queue à l'appui » ; forme
   exacte set_config) — je n'y touche pas, consigne respectée.

## 6. Inquiétudes, même vagues

- **La revue a trouvé 7 vrais défauts dans mon propre travail de la nuit**
  (dont un bloquant : l'évasion de chemins par antislash, et deux promesses
  non tenues : troncature de queue indétectable, vérification argon2 non
  épinglée). Tous corrigés et testés — mais la leçon vaut d'être écrite : le
  premier jet de code de sécurité, même discipliné, ne survit pas seul à une
  relecture adversariale. Budgète la même revue pour le sink d'audit et la
  couche d'inscription.
- Les vecteurs d'audit figent un format que PERSONNE ne consomme encore : si
  tu veux changer quoi que ce soit au format (champs hachés, horodatage,
  genèse), c'est MAINTENANT, avant le premier `INSERT` en production.
  Ensuite, c'est une migration de chaîne complète.
- La garde textuelle `SET` est heuristique (balayage de sources) : elle
  attrape la dérive honnête, pas un contournement déterminé. La vraie
  frontière reste le point de passage unique de kollega-store.
- `verify_password` refuse désormais tout profil hors liste blanche : le
  jour où tu durcis les paramètres argon2, il FAUT ajouter le nouveau profil
  à `ACCEPTED_PROFILES` sans retirer l'ancien, sinon tous les comptes
  existants seront verrouillés. C'est documenté dans le code, mais c'est le
  genre de piège qu'on retrouve en production deux ans plus tard.
- Le compile_fail du doctest des identifiants passerait aussi pour une autre
  erreur de compilation dans le snippet ; je l'ai gardé minimal pour limiter
  ce risque, mais `trybuild` ferait mieux si tu l'autorises un jour.
