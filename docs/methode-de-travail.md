# Méthode de travail — la délégation par paliers de confiance

> **AVERTISSEMENT.** Ce document est un ensemble d'HYPOTHÈSES écrites sans
> avoir parlé à un seul dirigeant. Il n'a aucune valeur de constat. Sa seule
> fonction est de rendre ces hypothèses assez précises pour être RÉFUTÉES en
> entretien. Tant qu'un dirigeant réel ne les a pas confrontées, chaque
> affirmation ci-dessous est à lire au conditionnel.

## La thèse

On n'installe pas un agent, on lui **délègue progressivement**, par paliers
de confiance **qui ne redescendent pas d'eux-mêmes**. Un dirigeant n'accorde
pas de l'autonomie parce qu'un logiciel la réclame ; il l'accorde parce qu'il
a vu l'agent faire, plusieurs fois, la chose qu'il aurait faite lui-même. La
confiance se gagne par la preuve accumulée, et une preuve acquise ne se
reperd pas sans raison.

C'est le contraire du modèle « configure puis lâche » : ici, le produit
accompagne une montée en autonomie que le dirigeant contrôle.

## Les quatre paliers

### Palier 1 — Simulation (l'agent propose, n'agit pas)

- **Ce que le dirigeant voit** : pour chaque élément (un mail, un document),
  ce que l'agent FERAIT — le classement proposé, le brouillon de réponse, la
  ligne de tableau extraite — et le coût estimé.
- **Ce qu'il fait** : il compare la proposition à ce qu'il aurait fait. Il ne
  valide rien qui parte : rien ne part.
- **Temps par jour** : 10-15 min au début, pour se faire une opinion.
- **Passage au palier suivant** : quand, sur N éléments d'affilée (hypothèse :
  ~20), il constate qu'il aurait pris la même décision.
- **Ce qui fait redescendre** : rien d'automatique — c'est le palier
  plancher. Mais un taux de désaccord élevé y maintient.

### Palier 2 — Validation systématique (l'agent agit, chaque action validée)

- **Ce que le dirigeant voit** : une file d'attente d'actions prêtes à
  partir, chacune avec sa source, son coût, son risque.
- **Ce qu'il fait** : il approuve ou refuse chaque action. L'agent exécute
  les approuvées.
- **Temps par jour** : 5-10 min, en une ou deux passes.
- **Passage au suivant** : quand il se surprend à approuver sans lire, pour
  une CATÉGORIE d'actions précise (« les accusés de réception, je les laisse
  passer »).
- **Ce qui fait redescendre** : une action refusée qui aurait dû l'être
  automatiquement — signal que le périmètre était trop large.

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
- **Ce qui fait redescendre** : une exception mal jugée par l'agent (il a agi
  seul là où il aurait dû demander) — le seuil se resserre.

### Palier 4 — Autonomie bornée (plafonds, revue a posteriori)

- **Ce que le dirigeant voit** : rien en temps réel ; une revue périodique
  (hebdomadaire ?) de ce qui a été fait, avec le coût et les actions notables.
- **Ce qu'il fait** : il révise les bornes (plafond de coût, périmètre) et
  contrôle par échantillon.
- **Temps par jour** : 0 en semaine ; ~15 min à la revue.
- **Passage au suivant** : il n'y en a pas — c'est le palier plafond, et il
  reste borné par construction.
- **Ce qui fait redescendre** : une action hors borne (bloquée par le plafond,
  donc jamais exécutée) ou une revue qui révèle une dérive → retour au
  palier 3 sur la catégorie concernée.

## L'unité de travail déléguée : le mandat

Ce qu'on délègue n'est pas « une tâche », c'est un **mandat** borné :

- un **périmètre** (quelle boîte, quels dossiers, quel type d'action) ;
- un **budget** (plafond de coût par tâche et par période) ;
- un **seuil de validation** (au-delà de quoi l'humain est appelé) ;
- une **échéance** (le mandat n'est pas éternel ; il se reconduit
  explicitement).

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

- La décision sur une personne (annexe III de l'AI Act — interdit permanent,
  pas un palier).
- L'engagement juridique ou financier au-delà du plafond, sans validation.
- Le changement du mandat lui-même : un agent n'élargit jamais son propre
  périmètre ni son propre budget.
- La relation quand elle bascule dans l'exceptionnel (un client mécontent, un
  litige) — l'agent prépare, l'humain porte.

## Dix questions à poser en entretien

Chaque question est formulée pour qu'une réponse puisse INVALIDER une
hypothèse de ce document.

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
7. Si l'assistant se trompe une fois sur dix, est-ce utilisable ? Une fois sur
   cent ? *(teste : le seuil de fiabilité qui permet de quitter le palier 1 —
   mon « 20 d'affilée » est peut-être absurde.)*
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

Une question dont toutes les réponses imaginables confirment la thèse aurait
été supprimée ; chacune ci-dessus a au moins une réponse qui casserait une
hypothèse.
