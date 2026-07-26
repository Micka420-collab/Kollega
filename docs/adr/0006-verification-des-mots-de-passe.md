# ADR-0006 — Vérification des mots de passe : paramètres stockés, bornés, avec re-hachage

**Statut :** Acceptée (session autonome du 28/07/2026)
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
   (256 Mio), `t > 64`, `p > 16`. C'est le **seul** argument valable pour
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
