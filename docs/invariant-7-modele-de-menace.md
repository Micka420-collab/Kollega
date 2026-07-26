# Modèle de menace — invariant 7 (instruction vs contenu externe)

Version 1 — 28/07/2026. Ce que l'assemblage protège, et surtout ce qu'il ne
protège PAS. L'honnêteté de ce document vaut plus que sa longueur.

Code : `crates/kollega-core/src/prompt.rs`. Corpus adversarial (35 cas) :
`crates/kollega-core/tests/segment_assembly.rs`.

## La menace

Un contenu externe — mail, document, sortie d'outil — arrive dans le
contexte d'un agent. Il peut contenir du texte qui *ressemble* à une
instruction (« ignore les instructions précédentes », fausses balises de
rôle, marques Unicode qui inversent l'affichage). Le risque : que ce texte
soit traité comme une consigne à exécuter, et non comme une donnée à traiter.

## Ce que l'assemblage garantit

1. **Séparation structurelle jusqu'au bout.** `compile` produit un
   `CompiledPrompt { system, user_request, documents[] }`. Par construction
   (un `match` sur `Segment`, sans branche traversante), un
   `ExternalContent` ne peut alimenter que `documents[]` — jamais `system`
   ni `user_request`. Le corpus le vérifie sur 35 contenus hostiles : dans
   tous les cas, les deux champs d'instruction ressortent identiques à
   l'entrée.
2. **Neutralisation de la manipulation invisible.** Marques bidi
   (U+202A..U+202E, U+2066..U+2069, U+061C), caractères à largeur nulle
   (U+200B..U+200F, U+FEFF, U+2060) et contrôles hors `\n`/`\t` sont
   remplacés par U+FFFD. Un opérateur qui relit le journal voit une
   substitution, pas un texte trafiqué qui s'affiche à l'envers.
3. **Bornage explicite.** Un contenu qui dépasse la limite est tronqué avec
   une marque visible et `truncated = true` — jamais de disparition
   silencieuse d'une partie du contexte.
4. **Origine préservée après sérialisation.** La forme JSON garde le contenu
   dans `documents[].content`, étiqueté par sa provenance et sa
   classification.

## Ce que l'assemblage NE garantit PAS — sans détour

- **La concaténation en aval.** Si un `ModelProvider` prend `system`,
  `user_request` et `documents` et les colle en un seul bloc de texte, toute
  la séparation est perdue. Transporter la structure jusqu'à l'API (rôles
  distincts, contenu externe présenté comme donnée) est le contrat du
  `ModelProvider`, pas de ce module — et ce sera un point de revue au jalon
  M3. Le type `CompiledPrompt` rend ce contrat explicite ; il ne peut pas le
  faire respecter à la couche suivante.
- **L'obéissance du modèle.** Aucune neutralisation n'empêche un modèle de
  langue de *suivre* une consigne qu'il sait pourtant être une donnée. La
  défense réelle contre « le document a demandé de virer 50 000 € » n'est
  pas typographique : c'est le moteur de politiques (aucun outil sans règle)
  et la validation humaine par seuil. L'assemblage réduit la surface, il ne
  la supprime pas.
- **Les homoglyphes dans le contenu.** On ne réécrit pas le contenu externe
  (un `о` cyrillique reste tel quel) : le neutraliser fausserait la donnée
  que l'agent doit traiter. Les homoglyphes sont refusés là où ils créent
  une usurpation d'IDENTITÉ (adresses email, `kollega-core::identity`), pas
  dans le corps d'un document.
- **La sémantique du contenu tronqué.** Tronquer peut couper une phrase en
  deux ; on marque la troncature, on ne garantit pas que le sens survit. Un
  agent qui a besoin de l'intégralité d'un très long document est un cas à
  traiter au niveau produit (découpage, résumé), pas ici.

## Conséquence pour les jalons suivants

- M3 (`ModelProvider`) : test dédié prouvant que les trois champs partent en
  rôles distincts vers l'API, jamais concaténés.
- Tout nouveau canal d'entrée de contenu externe passe par `Segment` puis
  `compile` — il n'existe pas d'autre porte, et il ne doit pas en exister.
