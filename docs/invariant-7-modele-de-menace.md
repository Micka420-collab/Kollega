# Modèle de menace — invariant 7 (instruction vs contenu externe)

Version 2 — 28/07/2026. Remplace la v1 du même jour : **la neutralisation
sort, le confinement reste** (correction demandée par la revue externe,
bloc 2). Ce que l'assemblage protège, et surtout ce qu'il ne protège PAS.
L'honnêteté de ce document vaut plus que sa longueur.

Code : `crates/kollega-core/src/prompt.rs`. Corpus adversarial (34 cas) :
`crates/kollega-core/tests/segment_assembly.rs`.

## La menace

Un contenu externe — mail, document, sortie d'outil — arrive dans le
contexte d'un agent. Il peut contenir du texte qui *ressemble* à une
instruction (« ignore les instructions précédentes », fausses balises de
rôle, marques Unicode qui inversent l'affichage). Le risque : que ce texte
soit traité comme une consigne à exécuter, et non comme une donnée à traiter.

## La décision : confinement seulement — pourquoi la neutralisation est sortie

Il existe deux stratégies contre l'injection, et elles ne se mélangent pas :
le **confinement** (séparation structurelle, l'origine reste explicite, la
donnée est intacte) et la **neutralisation** (on modifie le contenu pour le
rendre inoffensif). La v1 revendiquait le confinement et pratiquait AUSSI la
neutralisation (bidi, invisibles, contrôles → U+FFFD). C'était une confusion
de stratégie, au coût réel : un agent d'extraction qui reçoit du contenu
épuré ne lit plus le document du client. Une PME avec des noms en arabe ou
en hébreu a des marques de direction Unicode **légitimes** ; les retirer
corrompt ses données. Idem pour un U+200B collé depuis un ERP.

Les trois questions que la v1 laissait sans réponse, tranchées :

1. **Le contenu neutralisé était-il celui envoyé au modèle, celui
   journalisé, ou les deux ?** Les deux — `CompiledDocument.content` était
   l'unique forme conservée en aval de `compile`. C'était le cœur du
   problème : il n'existait plus de forme intacte du document après
   l'assemblage.
2. **Que voit un agent d'extraction ?** Depuis la v2 : le document du client
   VERBATIM, octet pour octet (marques de direction, largeur nulle, CRLF et
   contrôles compris). En v1 il voyait une version mutilée, avec des U+FFFD
   à la place de caractères porteurs de sens.
3. **Une entrée d'audit qui hache du contenu MODIFIÉ atteste-t-elle encore
   de ce qui est arrivé ?** Non — elle atteste de ce que NOUS avons fabriqué
   à partir de ce qui est arrivé, ce qui affaiblit la valeur probante du
   journal. Depuis la v2, le contenu transporté est celui qui est arrivé :
   ce que le journal hachera (jalon de persistance de l'audit) est ce que
   la source a réellement produit.

Conséquences d'implémentation, toutes dans le même esprit — « intact veut
dire intact » :

- plus AUCUNE substitution de caractères (la fonction `neutralize` et le
  champ `neutralized` n'existent plus) ;
- plus de normalisation `\r\n`/`\r` → `\n` : c'était aussi une modification ;
- la troncature ne s'écrit plus DANS le contenu (l'ancien marqueur
  « [contenu tronqué…] » injectait notre texte dans la donnée) : le préfixe
  conservé est verbatim et le drapeau `truncated` porte l'information.

## Ce que l'assemblage garantit

1. **Séparation structurelle jusqu'au bout.** `compile` produit un
   `CompiledPrompt { system, user_request, documents[] }`. Par construction
   (un `match` sur `Segment`, sans branche traversante), un
   `ExternalContent` ne peut alimenter que `documents[]` — jamais `system`
   ni `user_request`. Le corpus le vérifie sur 34 contenus hostiles : dans
   tous les cas, les deux champs d'instruction ressortent identiques à
   l'entrée.
2. **Transport verbatim.** Le contenu externe ressort octet pour octet
   (testé sur le corpus entier et par proptest sur chaînes arbitraires) :
   la valeur d'usage (extraction) et la valeur probante (audit) sont
   préservées ensemble.
3. **Bornage explicite.** Un contenu qui dépasse la limite est coupé à la
   borne — préfixe verbatim, `truncated = true` — jamais de disparition
   silencieuse d'une partie du contexte, jamais de texte injecté.
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
- **L'obéissance du modèle.** Rien n'empêche un modèle de langue de *suivre*
  une consigne qu'il sait pourtant être une donnée — et depuis la v2, les
  caractères invisibles ou directionnels d'un contenu hostile lui parviennent
  tels quels. C'est assumé : la défense réelle contre « le document a demandé
  de virer 50 000 € » n'a jamais été typographique — c'est le moteur de
  politiques (aucun outil sans règle) et la validation humaine par seuil.
  Une liste noire de caractères ne fermait pas ce chemin (elle était de
  toute façon incomplète : tags U+E0000–E007F, sélecteurs de variante…) ;
  elle donnait une impression de sécurité en détruisant de la donnée.
- **L'affichage trompeur chez l'humain.** Un contenu intact peut contenir
  des marques bidi qui inversent visuellement un texte, ou des invisibles
  qui font différer ce que le dirigeant LIT de ce que le modèle A LU. C'est
  désormais explicitement un devoir de la couche de PRÉSENTATION (jalon
  M6) : isolation bidi au rendu (`<bdi>`, `unicode-bidi: isolate`) et mise
  en évidence des invisibles à l'AFFICHAGE — sans jamais modifier la donnée
  stockée. Ce transfert de responsabilité est le prix du confinement, et il
  doit se payer à M6, pas s'oublier.
- **Les homoglyphes dans le contenu.** On ne réécrit pas le contenu externe
  (un `о` cyrillique reste tel quel). Les homoglyphes sont refusés là où ils
  créent une usurpation d'IDENTITÉ (adresses email,
  `kollega-core::identity`), pas dans le corps d'un document.
- **La sémantique du contenu tronqué.** Couper à la borne peut couper une
  phrase en deux ; on signale la coupe, on ne garantit pas que le sens
  survit. Un agent qui a besoin de l'intégralité d'un très long document est
  un cas à traiter au niveau produit (découpage, résumé), pas ici. Le
  consommateur qui veut signaler la troncature au modèle le fait par un
  canal SÉPARÉ du contenu (métadonnée du document), jamais en écrivant dans
  la donnée.

## Conséquence pour les jalons suivants

- M3 (`ModelProvider`) : test dédié prouvant que les trois champs partent en
  rôles distincts vers l'API, jamais concaténés ; la métadonnée `truncated`
  est transmise hors du contenu.
- M6 (interface) : rendu bidi isolé et invisibles rendus visibles à
  l'affichage de tout contenu externe — test sur les gabarits, avec les cas
  du corpus adversarial comme jeu d'essai.
- Tout nouveau canal d'entrée de contenu externe passe par `Segment` puis
  `compile` — il n'existe pas d'autre porte, et il ne doit pas en exister.
