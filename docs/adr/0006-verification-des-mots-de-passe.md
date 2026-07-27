# ADR-0006 — Vérification des mots de passe : paramètres stockés, bornés, avec re-hachage

**Statut :** Acceptée (session autonome du 28/07/2026), **amendée le 28/07/2026** (voir Amendement)
**Date :** 28 juillet 2026
**Corrige :** l'épinglage par liste blanche introduit la nuit du 27/07

---

## Contexte

La vérification argon2 relit l'algorithme et les coûts depuis la chaîne PHC
stockée. La nuit du 27/07, une revue avait conclu à un risque de
rétrogradation et l'implémentation avait épinglé une **liste blanche stricte
de profils** : tout profil hors liste = refus.

C'était un défaut, pas une protection :

- **La liste blanche ne ferme aucun chemin réel.** L'attaquant qu'elle vise
  doit pouvoir écrire dans la colonne `password_hash`. Or cet attaquant n'a
  aucun besoin d'un profil affaibli : il écrit un argon2id **parfaitement
  conforme** d'un mot de passe qu'il connaît, et il entre. Le profil du
  hachage n'est pas la barrière ; l'accès en écriture est déjà la défaite.
- **Son coût, lui, est certain.** Le jour où l'on durcit les paramètres, un
  oubli d'ajout à la liste verrouille tous les comptes existants. Un piège
  qui se déclenche des années plus tard, en production.

## Décision

1. **Vérifier avec les paramètres stockés.** Le format PHC est
   auto-descriptif ; c'est son intérêt et c'est le fonctionnement naturel de
   la crate.
2. **Un plancher** — refuser le réellement cassé : `m < 8 192 KiB` (8 Mio),
   `t = 0`, `p = 0`.
3. **Un plafond** — refuser l'absurdement coûteux : `m > 262 144 KiB`
   (256 Mio), `t > 64`, `p > 16`.
   > ⚠️ **Chiffre PÉRIMÉ, conservé pour l'historique.** L'amendement en fin
   > de document abaisse ce plafond à **64 Mio** (`MAX_MEMORY_KIB =
   > 65 536`), et c'est ce que le code applique. Le pointeur est posé ici
   > parce qu'un lecteur qui ne lirait que ce point pourrait « réparer » le
   > code en le remontant à 256 Mio, c'est-à-dire annuler la correction.

   C'est le **seul** argument valable pour
   contraindre les paramètres d'une chaîne stockée : une chaîne forgée
   `m=4 Gio` ferait allouer 4 Gio à chaque tentative de connexion — un déni
   de service. Le contrôle précède la vérification : la mémoire demandée
   n'est jamais allouée.
4. **Re-hachage à la connexion.** Si le mot de passe est correct mais que le
   profil stocké diffère du profil courant, l'issue est
   `ValidNeedsRehash` : l'appelant re-hache immédiatement (le clair est
   disponible à cet instant précis) et remplace la colonne. Le parc migre
   tout seul ; personne n'est jamais verrouillé.
5. **Cohérence de format conservée** : seule une chaîne `argon2id` v19 est
   acceptée — nous n'avons jamais produit autre chose ; `argon2i`/`argon2d`
   dans cette colonne est une anomalie de données, signalée comme telle.

## Conséquences

- Durcir la politique = changer les constantes du profil courant. Rien
  d'autre. Les anciens comptes continuent de se connecter et migrent à leur
  première connexion.
- La couche d'inscription (jalon M1) doit traiter `ValidNeedsRehash` : même
  chemin que `Valid`, plus une écriture de la nouvelle empreinte.
- Les bornes sont des constantes documentées dans `kollega-api::auth` ; les
  élargir est une décision, pas un réflexe.

## Alternatives écartées

| Alternative | Pourquoi écartée |
|---|---|
| Liste blanche stricte de profils (état du 27/07) | Ne ferme aucun chemin réel ; verrouille le parc au premier durcissement oublié |
| Aucune borne (paramètres stockés bruts) | Laisse la primitive de déni de service mémoire/CPU par chaîne forgée |
| Re-hachage par tâche de fond | Exige le mot de passe en clair, qu'on n'a qu'à la connexion — impossible par construction |

---

## Amendement du 28/07/2026 — plafond abaissé ET sémaphore (revue externe, bloc 3)

Le plafond initial (256 Mio) était trop haut, et surtout il ne défendait
pas contre la vraie attaque : une tentative à 256 Mio passait, mais **deux
cents tentatives simultanées à 19 Mio parfaitement conformes font
~3,8 Gio** et le serveur tombe. Le plafond par opération ne dit rien du
volume. Deux mesures, cumulatives — l'une sans l'autre ne suffit pas :

1. **Plafond abaissé à 64 Mio** (`MAX_MEMORY_KIB = 65 536`). Défend contre
   une empreinte stockée **empoisonnée** : ~3,4× le profil courant
   (19 Mio), assez pour durcir la politique plusieurs fois sans re-toucher
   la borne, plus rien pour faire coûter cher une chaîne forgée. Testé aux
   deux bords : 64 Mio exact accepté (et signalé pour re-hachage),
   128 Mio — légal sous l'ancien plafond — refusé.
2. **Sémaphore sur les opérations argon2 concurrentes**
   (`MAX_CONCURRENT_ARGON2 = 4`, hachage et vérification). Défend contre le
   **volume**. Dimensionnement documenté dans le code : pire cas légitime
   4 × 64 Mio = 256 Mio, cas courant 4 × 19 Mio ≈ 76 Mio — compatible avec
   le serveur mutualisé modeste du profil. Contrat : au-delà de la borne,
   les appels **attendent, ils n'échouent pas** — testé (porte saturée à la
   main : la vérification patiente puis aboutit ; 3× la borne en parallèle :
   aucune erreur, aucune fausse réponse). Le permis n'est pris qu'après le
   contrôle des bornes : une chaîne forgée absurde est refusée sans
   consommer de place dans la file.

Conséquence pour M1 : le gestionnaire HTTP appellera ces fonctions dans un
`spawn_blocking` (elles bloquent, par contrat) ; la borne vivra alors côté
travail bloquant, pas dans l'exécuteur async.
