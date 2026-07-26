# Spécification — encodage canonique et empreinte de la chaîne d'audit

Version 3 — 28/07/2026. Toute modification de ce document invalide les
vecteurs de référence (`crates/kollega-audit/tests/reference_vectors.rs`) et,
dès qu'une chaîne existera en production, exigerait une migration de chaîne
complète. Historique : v1 (27/07, genèse sans préfixe), v2 (27/07, genèse à
32 octets zéro), v3 (28/07, hauteur dans l'enregistrement).

Cette spécification est LA source : l'implémentation Rust
(`crates/kollega-audit`) et la référence indépendante Python
(`tools/reference/canonical.py`) doivent toutes deux en découler — jamais
l'une de l'autre.

## 1. Valeurs canoniques

Types admis : `Null`, `Bool`, `Int` (entier signé 64 bits), `Text` (UTF-8),
`Array`, `Object`. **Pas de flottant** : un flottant n'a pas de forme
canonique sûre, et l'argent est un entier.

Encodage (syntaxe proche de JSON, définie ici indépendamment) :

| Valeur | Forme |
|---|---|
| `Null` | `null` |
| `Bool` | `true` / `false` |
| `Int` | décimal ASCII signé, sans zéros de tête (forme `i64` de Rust : `0`, `-42`, `9223372036854775807`) |
| `Text` | `"…"`, échappement §2 |
| `Array` | `[v1,v2]`, sans aucune espace |
| `Object` | `{"k1":v1,"k2":v2}`, sans aucune espace ; clés uniques, échappées comme `Text`, triées par ordre lexicographique d'octets UTF-8 |

## 2. Échappement de texte

Dans l'ordre de priorité :

| Caractère | Forme |
|---|---|
| `\` (U+005C) | `\\` |
| `"` (U+0022) | `\"` |
| U+000A | `\n` |
| U+000D | `\r` |
| U+0009 | `\t` |
| autre contrôle U+0000..U+001F | `\u00xx` (hexadécimal MINUSCULE, quatre chiffres) |
| tout le reste | verbatim UTF-8 (aucun échappement du non-ASCII) |

## 3. Enregistrement d'audit

Champs, dans cet ordre FIGE (qui coïncide avec l'ordre alphabétique, mais
c'est l'ordre écrit ici qui fait foi) :

```
{"action":<Text>,"actor":<Text>,"height":<Int>,"org_id":<Text>,"payload":<Valeur>}
```

- `action` : nature de l'événement (`task_started`, …).
- `actor` : qui agit.
- `height` : hauteur de l'entrée dans la chaîne de son organisation,
  0 pour la première entrée. Entier non signé émis en décimal. Sa présence
  dans les octets hachés rend invalide toute entrée déplacée ou rejouée à
  une autre position.
- `org_id` : UUID de l'organisation, forme textuelle canonique (minuscules,
  tirets). L'organisation vient de la chaîne, jamais de l'entrée.
- `payload` : valeur canonique (§1).

L'horodatage n'est PAS dans l'enregistrement : il entre séparément dans
l'empreinte (§4).

## 4. Empreinte

```
hash = SHA-256( prefixe_prev || enregistrement || horodatage )
```

- `prefixe_prev` : exactement 32 octets — l'empreinte de l'entrée
  précédente, ou 32 octets 0x00 pour la première entrée (hauteur 0). Le
  préfixe de longueur fixe rend la séparation des trois champs structurelle.
- `enregistrement` : les octets UTF-8 du §3.
- `horodatage` : microsecondes depuis l'époque Unix (entier signé 64 bits),
  décimal ASCII. Suit la précision de `timestamptz` PostgreSQL.

Non-ambiguïté : le préfixe est de longueur fixe ; l'enregistrement se
termine par `}` ; l'horodatage ne contient que des chiffres et `-`.

## 5. Vérification

- `verify` : pour chaque position i — `height == i`, `prev_hash` égal à
  l'empreinte précédente (`None` en tête), empreinte recalculée égale à
  l'empreinte stockée. Garantit la **cohérence interne** seulement.
- `verify_with_tail` : `verify`, puis l'empreinte de queue comparée à une
  ancre de confiance externe — seule défense contre la troncature de queue
  et la réécriture d'un suffixe (modèle de menace :
  `docs/audit-modele-de-menace.md`).

## 6. Vecteurs de référence

Cinq vecteurs figés dans `reference_vectors.rs` ; le premier est recoupé par
un SHA-256 calculé hors Rust à chaque changement de version de ce format.
Ne JAMAIS mettre à jour une empreinte attendue sans comprendre pourquoi elle
a changé.

## 7. Ce qui prouve quoi — cadrage (bloc 6, 28/07/2026)

Deux dispositifs, deux propriétés distinctes — ne pas les confondre :

- **L'injectivité est prouvée par le round-trip Rust**
  (`crates/kollega-audit/tests/canonical_injectivity.rs`) : un décodeur
  écrit depuis cette spécification, jamais depuis l'encodeur, rend
  l'original pour toute valeur générée (proptest, biais vers les
  séparateurs). Un encodage qui admet un inverse à gauche est **injectif
  par construction** — deux valeurs distinctes ne peuvent pas produire les
  mêmes octets, sinon le décodeur ne saurait pas rendre les deux. C'est
  acquis, et c'est plus fort que « aucune collision observée ».
- **La non-ambiguïté de LA SPÉCIFICATION est prouvée par le différentiel
  Rust ↔ Python** (`tools/reference/canonical.py`, exécuté en CI depuis le
  28/07/2026) : deux lecteurs indépendants de ce document produisent les
  mêmes octets et les mêmes empreintes sur ≥ 14 000 vecteurs. Cela ne
  prouve PAS l'injectivité — deux implémentations pourraient converger sur
  un encodage ambigu — mais c'est exactement ce dont un **auditeur tiers**
  a besoin : la garantie qu'il peut vérifier une chaîne depuis ce document
  seul, sans notre code.

Les deux ensemble : le format est injectif (personne ne peut forger deux
enregistrements distincts à empreinte de contenu identique) ET
indépendamment implémentable (personne n'est obligé de nous croire sur
parole).
