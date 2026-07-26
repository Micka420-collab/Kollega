# Référence indépendante de l'encodage canonique

## État : NON VÉRIFIÉ

`canonical.py` a été écrit le 28/07/2026 **depuis la spécification**
(`docs/encodage-canonique.md`), sans interpréteur Python sur la machine de
développement : il n'a **jamais été exécuté**. Il ne prouve donc rien
aujourd'hui — c'est un artefact à activer, pas une garantie.

## Ce qu'il faut pour le vérifier

1. Un interpréteur Python 3.10+ (`char | None` dans les annotations).
2. Un harnais différentiel (à écrire) :
   - le test Rust `canonical_injectivity.rs` génère N ≥ 10 000 vecteurs et
     émet, pour chacun, la valeur (forme JSON taguée) et l'encodage Rust ;
   - `canonical.py` relit les valeurs sur stdin et émet l'encodage Python ;
   - comparaison octet à octet ; toute divergence est signalée.
3. Recouper au moins un vecteur d'`entry_hash` avec le SHA-256 indépendant
   déjà utilisé (le V1 des vecteurs de référence Rust).

## Règle

Toute divergence Rust/Python est d'abord un **défaut de spécification** :
on met à jour `docs/encodage-canonique.md` pour lever l'ambiguïté, PUIS on
aligne les deux implémentations. On ne « corrige » jamais silencieusement
l'une pour coller à l'autre — ce serait perdre l'intérêt d'avoir deux
implémentations écrites indépendamment.
