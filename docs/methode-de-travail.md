# Méthode de travail — la délégation par paliers de confiance

> **AVERTISSEMENT.** Ce document est un ensemble d'HYPOTHÈSES écrites sans
> avoir parlé à un seul dirigeant. Il n'a aucune valeur de constat. Sa seule
> fonction est de rendre ces hypothèses assez précises pour être RÉFUTÉES en
> entretien. Tant qu'un dirigeant réel ne les a pas confrontées, chaque
> affirmation ci-dessous est à lire au conditionnel.
>
> Révision du 28/07/2026 (bloc 7) : six corrections issues de la revue
> externe — palier porté par le couple (mandat, catégorie), palier 1 sur
> l'historique, abandon silencieux nommé, cliquet tranché, critère de
> promotion par déclaration, questions d'entretien complétées (dont les
> quatre de `docs/taches-delegables-analyse.md`).

## La thèse

On n'installe pas un agent, on lui **délègue progressivement**, par paliers
de confiance **qui ne redescendent jamais sans décision du dirigeant**. Un
dirigeant n'accorde pas de l'autonomie parce qu'un logiciel la réclame ; il
l'accorde parce qu'il a vu l'agent faire, plusieurs fois, la chose qu'il
aurait faite lui-même. La confiance se gagne par la preuve accumulée, et une
preuve acquise ne se reperd pas sans raison.

C'est le contraire du modèle « configure puis lâche » : ici, le produit
accompagne une montée en autonomie que le dirigeant contrôle.

## À quoi s'applique un palier — décision de schéma

**Le palier appartient au couple (mandat, catégorie d'action), pas à
l'agent.** Les critères de ce document le disaient déjà sans le dire : la
promotion depuis le palier 2 se fait « pour une CATÉGORIE précise », la
rétrogradation depuis le palier 4 « sur la catégorie concernée ». Avec un
palier accroché à l'agent, on ne peut pas exprimer la situation la plus
banale : « les accusés de réception partent seuls, les devis sont toujours
validés » — un même agent, deux niveaux de confiance.

C'est une décision de SCHÉMA, gratuite aujourd'hui (rien n'est en base),
coûteuse dès que des mandats existeront : la table des mandats porte un
niveau de palier PAR CATÉGORIE D'ACTION, pas un niveau global. L'impact sur
le schéma de M4 est noté dans `docs/backlog.md`.

## Les quatre paliers

### Palier 1 — Simulation sur l'HISTORIQUE (l'agent rejoue le passé)

La simulation ne se fait pas en temps réel : elle se fait sur **la semaine
écoulée**. L'agent traite des mails et des documents que le dirigeant a DÉJÀ
traités, et l'on compare ce que l'agent AURAIT fait à ce que le dirigeant a
RÉELLEMENT fait — pas à ce qu'il croit qu'il ferait.

- **Ce que le dirigeant voit** : élément par élément, côte à côte — la
  proposition de l'agent (classement, brouillon de réponse, ligne extraite),
  ce qui s'est réellement passé, et le coût estimé.
- **Ce qu'il fait** : il constate les accords et les désaccords, par
  catégorie d'action. Rien ne part : le passé est déjà parti.
- **Temps** : une séance, pas trois semaines. La simulation temps réel
  consommait des appels et 10-15 min/jour pour zéro action exécutée —
  vingt éléments d'affilée, c'était le délai exact pour perdre le client.
- **Passage au palier suivant** : par DÉCLARATION, catégorie par catégorie
  (voir « le critère de promotion » ci-dessous), séance tenante si la
  comparaison sur l'historique le convainc.
- **Rétrogradation** : sans objet — c'est le palier plancher.

Bonus produit : c'est ce qui rend la promesse de M4 démontrable — « voici ce
que l'agent aurait fait de votre boîte la semaine dernière » est une
démonstration sur données réelles, pas une projection.

### Palier 2 — Validation systématique (l'agent agit, chaque action validée)

- **Ce que le dirigeant voit** : une file d'attente d'actions prêtes à
  partir, chacune avec sa source, son coût, son risque.
- **Ce qu'il fait** : il approuve ou refuse chaque action. L'agent exécute
  les approuvées.
- **Temps par jour** : 5-10 min, en une ou deux passes.
- **Passage au suivant** : quand il se surprend à approuver sans lire, pour
  une CATÉGORIE d'actions précise (« les accusés de réception, je les laisse
  passer ») — et qu'il le déclare.
- **Signal de rétrogradation** (proposé, jamais imposé — voir « le
  cliquet ») : une action refusée qui aurait dû l'être automatiquement —
  signe que le périmètre était trop large.

### Palier 3 — Validation par seuil (routine automatique, exception validée)

**C'est le palier où le produit tient sa promesse.** L'agent exécute seul ce
qui est routinier ; il ne demande une validation que pour l'exception —
au-delà d'un montant, hors d'un dossier, vers un destinataire inhabituel.

- **Ce que le dirigeant voit** : le matin, ce qui a été fait seul (résumé,
  coût) ; en cours de journée, uniquement les exceptions à trancher.
- **Ce qu'il fait** : il tranche les exceptions — quelques-unes par jour.
- **Temps par jour** : 2-5 min.
- **Passage au suivant** : quand les exceptions elles-mêmes deviennent rares
  et répétitives, et qu'il est prêt à les borner plutôt qu'à les voir.
- **Signal de rétrogradation** : une exception mal jugée par l'agent (il a
  agi seul là où il aurait dû demander) — le système propose de resserrer le
  seuil ou de repasser la catégorie au palier 2 ; le dirigeant tranche.

### Palier 4 — Autonomie bornée (plafonds, revue a posteriori)

- **Ce que le dirigeant voit** : rien en temps réel ; une revue périodique
  (hebdomadaire ?) de ce qui a été fait, avec le coût et les actions notables.
- **Ce qu'il fait** : il révise les bornes (plafond de coût, périmètre) et
  contrôle par échantillon.
- **Temps par jour** : 0 en semaine ; ~15 min à la revue.
- **Passage au suivant** : il n'y en a pas — c'est le palier plafond, et il
  reste borné par construction.
- **Signal de rétrogradation** : une action hors borne (bloquée par le
  plafond, donc jamais exécutée) ou une revue qui révèle une dérive — le
  système propose le retour au palier 3 sur la catégorie concernée ; le
  dirigeant tranche.

## Le cliquet — tranché

La v1 disait « des paliers qui ne redescendent pas d'eux-mêmes » puis
donnait à chaque palier sa rubrique « ce qui fait redescendre ». Les deux ne
tenaient pas ensemble. Tranché ainsi :

- **La rétrogradation n'est JAMAIS automatique.** Une rétrogradation
  silencieuse ferait passer le temps quotidien du dirigeant de 2-5 min à
  5-10 min sans prévenir — c'est exactement le genre de surprise qui tue la
  confiance dans l'outil. Le système DÉTECTE les signaux (listés par
  palier), les PRÉSENTE avec leur raison, PROPOSE la rétrogradation ; le
  dirigeant décide d'un clic.
- **Une seule chose est automatique, et ce n'est pas une rétrogradation :
  la suspension de sécurité.** Limite dure franchie, crédit épuisé, plafond
  atteint → l'action est BLOQUÉE (elle ne part pas), le mandat continue à
  son palier. Bloquer un acte n'est pas retirer une confiance ; c'est le
  moteur de politiques qui fait son travail.

## Le critère de promotion — la déclaration, pas le compteur

« 20 d'affilée » mesurait le volume, pas la difficulté : vingt mails de
routine ne disent rien du vingt-et-unième, inhabituel — le seul qui compte.
Le critère est donc la **déclaration explicite de catégorie**, celle que le
palier 2 utilisait déjà : « les accusés de réception, je les laisse
passer » est précis, réfutable, et c'est le dirigeant qui le dit — pas un
compteur qui le déduit. Le produit peut suggérer une déclaration quand les
chiffres la soutiennent (« 40 accords sur 40 dans cette catégorie ») ; il ne
promeut jamais de lui-même.

## Le mode de défaillance à surveiller : l'abandon silencieux

Les paliers 3 et 4 supposent que le dirigeant lit la page du matin et
tranche les exceptions. S'il arrête — et il arrêtera, une semaine de
salon professionnel ou de coup de feu suffit — il est DE FAIT au palier 4
sans l'avoir décidé : les exceptions s'entassent, rien ne part, ou pire, il
tamponne en bloc pour vider la file. C'est l'état dangereux, et il était
invisible dans le design v1.

Traitement — comme un problème de PALIER, pas comme une notification :

- **Mesure** : validations non lues depuis N jours ; page du matin non
  ouverte depuis N jours ; taux d'approbation en bloc (tout tamponné en
  moins de X secondes).
- **Réaction** : le mandat concerné passe en « confiance non entretenue » —
  les actions en attente au-delà de l'échéance déclarée EXPIRENT (refus
  propre, tracé, jamais d'envoi tardif surprise), et la reconduction du
  mandat à son échéance exige une revue réelle, pas un clic.
- **Ce qu'on ne fait pas** : continuer comme si de rien n'était (palier 4 de
  fait), ni spammer de rappels (c'est traiter le symptôme).

## L'unité de travail déléguée : le mandat

Ce qu'on délègue n'est pas « une tâche », c'est un **mandat** borné :

- un **périmètre** (quelle boîte, quels dossiers, quel type d'action) ;
- un **budget** (plafond de coût par tâche et par période) ;
- un **seuil de validation** ET une **limite dure** au-dessus (les deux
  étages de `kollega-policy` : entre les deux, l'humain arbitre ; au-delà,
  refus que nulle validation ne lève) ;
- une **échéance** (le mandat n'est pas éternel ; il se reconduit
  explicitement) ;
- et, par catégorie d'action, un **palier de confiance** (cf. décision de
  schéma ci-dessus).

Le mandat est l'objet que le dirigeant comprend et ajuste. Les « agents »,
« configs » et « politiques » du système en sont la traduction technique — il
ne les voit pas sous ces noms.

## Le rythme quotidien

Ce que le dirigeant voit **le matin** (hypothèse centrale du produit) : une
page unique — ce qui a été fait pendant la nuit et ce qui attend sa décision,
avec pour chaque ligne la source, le coût et le risque. Pas un tableau de
bord d'analyste ; une liste de choses faites et de choses à trancher, lisible
en trois minutes avec un café.

## Ce qui ne se délègue jamais

- **Un agent traite du document, il ne décide pas sur un humain** (frontière
  opérationnelle de CLAUDE.md ; annexe III de l'AI Act — interdit permanent,
  pas un palier). L'administratif RH documentaire reste autorisé ; répondre
  à un client est traiter du document, pas décider sur une personne — la
  formulation v1 (« la décision sur une personne ») était si large qu'elle
  semblait interdire la relance ou la réponse client.
- L'engagement juridique ou financier au-delà du plafond, sans validation —
  et au-delà de la limite dure, même avec validation.
- Le changement du mandat lui-même : un agent n'élargit jamais son propre
  périmètre ni son propre budget.
- La relation quand elle bascule dans l'exceptionnel (un client mécontent, un
  litige) — l'agent prépare, l'humain porte.

## Seize questions à poser en entretien

Chaque question est formulée pour qu'une réponse puisse INVALIDER une
hypothèse de ce document. Les questions 11 et 12 sont les plus décisives —
elles déterminent le discours ET le prix. Les questions 13 à 16 viennent de
`docs/taches-delegables-analyse.md`.

1. Quand vous confiez une tâche à quelqu'un de nouveau dans votre équipe,
   combien de temps le regardez-vous faire avant de le laisser seul — et
   qu'est-ce qui vous fait décider que c'est bon ? *(teste : la confiance se
   gagne-t-elle par accumulation de preuves, ou autrement ?)*
2. Préféreriez-vous un outil qui fait tout seul et vous rend compte, ou un
   qui vous demande avant chaque action ? Pourquoi ? *(teste : les paliers
   2-3 correspondent-ils à un besoin, ou le dirigeant veut-il d'emblée le
   palier 4 — ou jamais le quitter ?)*
3. Racontez-moi la dernière fois qu'un logiciel a fait quelque chose en votre
   nom sans vous demander. Qu'avez-vous ressenti ? *(teste : l'autonomie est-elle
   désirée ou redoutée ?)*
4. Sur quelles tâches accepteriez-vous de NE PAS relire chaque action ? Sur
   lesquelles voudriez-vous toujours valider ? *(teste : le seuil du palier 3
   existe-t-il, et où ?)*
5. Le matin, qu'est-ce que vous regardez en premier pour savoir où en est
   votre entreprise ? *(teste : l'hypothèse de la « page du matin ».)*
6. Combien de temps par jour seriez-vous prêt à passer à superviser un tel
   assistant — et à partir de quand est-ce que ça ne vaut plus le coup ?
   *(teste : les budgets-temps annoncés par palier.)*
7. Racontez-moi la dernière erreur qu'un outil ou un prestataire a commise
   pour vous. Qu'est-ce qu'elle a coûté, et qu'avez-vous changé ensuite ?
   *(teste : la tolérance RÉELLE à l'erreur et son prix — la v1 demandait
   « une fois sur dix ? sur cent ? », ce qui ancrait sur NOS chiffres au
   lieu de faire parler les siens.)*
8. Qu'est-ce que vous ne confieriez à personne, humain ou machine ? *(teste :
   la liste « ce qui ne se délègue jamais ».)*
9. Comment décidez-vous aujourd'hui qu'une dépense mérite votre validation
   personnelle ? Un montant ? Un type ? *(teste : le mandat comme
   périmètre + budget + seuil.)*
10. Si vous pouviez rendre compte à vous-même de ce qu'a fait cet assistant,
    à qui d'autre auriez-vous besoin de le montrer — comptable, banque,
    client, personne ? *(teste : à qui sert vraiment le journal d'audit — et
    donc si la gouvernance « rendue visible » est un argument de vente ou une
    fonctionnalité que personne ne regarde.)*
11. Qui fait ce travail aujourd'hui ? *(décisive : si c'est l'assistante ou
    le comptable, on concurrence une personne et la vente porte sur son
    temps ; si c'est le dirigeant à 21 h, on vend ses soirées. Discours et
    prix opposés.)*
12. Que payez-vous aujourd'hui pour que ça se fasse ? *(décisive : quinze
    conversations sans jamais poser la question du prix gâchent
    l'échantillon.)*
13. Combien avez-vous d'impayés en ce moment, et depuis combien de temps ?
    *(teste : si la réponse est immédiate et précise, la douleur est vive ;
    si le dirigeant ne sait pas, elle est diffuse et la vente sera dure.)*
14. Le 1er septembre, vous devez pouvoir recevoir des factures
    électroniques. Vous avez choisi votre plateforme ? *(teste : la
    connaissance de la réforme et la fenêtre commerciale ; « quelle
    réforme ? » = cinq semaines et une conversation facile.)*
15. Qu'est-ce que votre expert-comptable pense des outils que vous
    utilisez ? *(teste : le canal est-il un allié ou un obstacle — sans
    jamais critiquer l'expert-comptable.)*
16. Vous avez déjà essayé ChatGPT pour votre travail. Qu'est-ce qui a fait
    que vous n'avez pas continué ? *(presque tout le monde a essayé ; la
    réponse donne l'objection exacte à traiter — plus utile que toute
    question sur ce qu'il « voudrait ».)*

Une question dont toutes les réponses imaginables confirment la thèse aurait
été supprimée ; chacune ci-dessus a au moins une réponse qui casserait une
hypothèse.
