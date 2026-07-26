# Référence indépendante de l'encodage canonique

## État : VÉRIFIÉ EN CI depuis le 28/07/2026 (run n°4)

`canonical.py` a été écrit le 28/07/2026 **depuis la spécification**
(`docs/encodage-canonique.md`), sans interpréteur Python sur la machine de
développement. Le harnais différentiel tourne désormais à chaque CI
(`.github/workflows/ci.yml`, étape « Différentiel encodage canonique ») :

- le test Rust `crates/kollega-audit/tests/diff_vectors.rs` génère
  12 014 vecteurs d'encodage (préambule de pièges figés + génération
  déterministe SplitMix64, graine figée, rejouable) et 2 000 empreintes
  complètes (`entry_hash` : prev × contenu × hauteur × horodatage) ;
- `canonical.py` les rejoue (modes stdin et `--hashes`) ;
- la CI compare octet à octet. Première exécution : run n°4, 28/07/2026,
  **aucune divergence** sur les 14 014 vecteurs.

Ce que cela prouve : la spécification est non ambiguë — deux lecteurs
indépendants en tirent les mêmes octets et les mêmes empreintes. C'est ce
qu'un auditeur tiers exigera pour vérifier une chaîne sans notre code.
Ce que cela ne prouve PAS : l'injectivité (prouvée côté Rust par le
round-trip de `canonical_injectivity.rs`, voir le cadrage dans
`docs/encodage-canonique.md`).

Correction du 28/07/2026, AVANT toute exécution : le bloc `__main__`
précédait la définition de `_from_json` (NameError garanti en mode script) ;
bloc déplacé en fin de fichier, fonctions d'encodage et de hachage
inchangées. Défaut d'ordre de définition, pas de spécification.

## Règle

Toute divergence Rust/Python est d'abord un **défaut de spécification** :
on met à jour `docs/encodage-canonique.md` pour lever l'ambiguïté, PUIS on
aligne les deux implémentations. On ne « corrige » jamais silencieusement
l'une pour coller à l'autre — ce serait perdre l'intérêt d'avoir deux
implémentations écrites indépendamment.
