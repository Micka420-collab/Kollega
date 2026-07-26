# Questions de la session autonome du 27/07/2026

Règle appliquée : détail réversible → option la plus conservatrice, notée
ici ; question architecturale → non tranchée, notée, bloc abandonné ou
contourné. Aucun bloc n'a été abandonné.

## Choix réversibles pris (à confirmer ou renverser au réveil)

1. **Numérotation de l'ADR d'authentification.** Le brief demandait
   `0003-authentification-hors-contexte.md`, mais 0003 est pris
   (`0003-postgres-seul-moteur.md`, renuméroté lors du pivot). Écrit en
   **0005**, prochain numéro libre.
2. **Email — règles de validation.** ASCII uniquement (atext RFC 5321 + point
   pour la partie locale ; alphanumérique + tiret pour les étiquettes de
   domaine), adresse entière minusculisée, blancs extérieurs tolérés puis
   retirés. Conséquences : pas d'IDN ni d'adresse Unicode (ce serait une
   décision punycode explicite), partie locale insensible à la casse (la RFC
   permet le contraire, personne ne le pratique). La restriction ASCII a été
   ajoutée après la revue adversariale : la version initiale acceptait les
   homoglyphes, c'était un vrai trou.
3. **Politiques — sémantique des seuils.** Au seuil exact = autorisé
   (cohérent avec le plafond de coût) ; dépassement = REFUS, jamais une
   demande de validation (la validation vient du seul drapeau
   `requires_approval`) ; valeur non déclarée sous une borne (montant,
   destinataires, chemins) = refus ; antislash et `..` dans un chemin =
   refus d'office. Question ouverte pour plus tard : veut-on un jour un
   seuil à deux étages (auto en dessous de X, validation entre X et le
   plafond) ? Non implémenté — trois clients payants d'abord.
4. **Emplacement du hachage de mots de passe.** Module `kollega-api::auth`.
   Créer une crate dédiée serait une décision de frontière (architecturale),
   non prise cette nuit. Déplaçable en cinq minutes si tu préfères.
5. **Audit — encodage de l'horodatage.** Microsecondes Unix (i64) en décimal
   ASCII, plutôt que RFC3339 : suit la précision de `timestamptz`, aucune
   dépendance de formatage. Fait partie du format figé par les vecteurs.
6. **Audit — genèse hachée avec 32 octets à zéro.** La version initiale
   omettait le préfixe pour la première entrée ; la revue a montré que
   l'argument « longueur fixe » était alors faux. Changé AVANT toute
   écriture en production (rien ne consomme encore ce format), vecteurs
   régénérés, V1 recoupé à nouveau hors Rust. C'était la dernière fenêtre où
   ce changement était gratuit.
7. **Audit — périmètre de l'enregistrement haché.** « payload canonique »
   inclut action, actor, org_id ET payload (ordre figé) : ne hacher que la
   charge utile aurait laissé l'acteur et l'action falsifiables.
8. **CI — contrat du test d'isolation.** Le brief mentionnait « deux URLs »
   fournies par la CI ; l'existant fait dériver l'URL kollega_app par le
   test lui-même (qui pose le mot de passe du rôle). Changer ce contrat sans
   pouvoir l'exécuter aurait risqué de casser la CI au premier push : NON
   MODIFIÉ cette nuit, à arbitrer quand la CI tournera.
9. **Organisation de kollega-core.** Les ajouts M1 vivent dans deux modules
   (`ids`, `identity`) ré-exportés à la racine ; le fichier des six types
   validés reste `lib.rs`, modifié uniquement pour les `mod`/`pub use` et
   les numéros d'invariants v1→v2 dans les commentaires.

## Questions architecturales rencontrées, non tranchées

- **Ancrage de la chaîne d'audit.** La revue a confirmé que `verify` seul ne
  détecte ni la troncature de queue ni la réécriture d'un suffixe par un
  attaquant en écriture. J'ai ajouté l'API pure `verify_with_tail` (ancre de
  confiance) et documenté le modèle de menace — mais OÙ vivra l'ancre
  (copie hors site, journal d'exploitation, remise au client ?) est une
  décision d'exploitation qui t'appartient, à prendre au jalon de
  persistance de l'audit.
- **CLAUDE.md.** Non touché, conformément à la consigne. Deux imprécisions y
  demeurent, à corriger par tes soins si tu en es d'accord : l'invariant 4
  dit « détecte toute altération » (vrai seulement avec une ancre de queue,
  cf. ci-dessus) ; l'invariant 1 et la table d'architecture disent
  « SET LOCAL app.current_org » alors que la forme réelle imposée par la
  garde textuelle est `set_config('app.current_org', $1, true)` —
  équivalente, mais autant que la constitution dise la vérité exacte.
