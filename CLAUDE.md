# CLAUDE.md — Constitution du dépôt

> Ce fichier est lu automatiquement par Claude Code au démarrage de chaque session.
> Il prime sur toute suggestion contraire faite en cours de conversation.
> Toute modification de ce fichier est une décision d'architecture : elle exige un ADR.

---

## 1. LE PRODUIT

**Un runtime d'agents IA gouvernés, qui s'exécute sur l'infrastructure du client.**

Client type : PME industrielle ou bureau d'études techniques, 50 à 300 salariés, pas de DSI dédiée, données qui ne sortent pas du site.

Promesse vendue : *« Un agent traite ce processus sur votre serveur. Chaque action est tracée, rejouable et attribuable. Le coût d'exécution est plafonné avant le lancement. »*

Ce n'est **pas** une plateforme, pas un studio sans code, pas une marketplace, pas un chatbot.

---

## 2. CONTRAINTES DE PRODUCTION

- **Un seul développeur**, à temps partiel. Toute décision qui suppose une équipe est mauvaise ici.
- **Rust** pour tout ce qui décide. **Python** pour tout ce qui transforme (extraction, OCR, parsing).
- Rythme de référence : le démon `vibed` de VibeOS, 77 tests, en Rust/tokio. C'est le calibre réel.
- Budget d'exécution réel : voir `docs/economie-unitaire.md`. Un agent mal plafonné coûte plus cher qu'il ne rapporte.

---

## 3. LES DOUZE INVARIANTS

Ce sont des propriétés du système, pas des objectifs. Une PR qui en viole un est refusée, quelle que soit sa valeur fonctionnelle. Chacun doit être couvert par au moins un test.

1. **Aucun appel d'outil ne s'exécute sans être passé par le moteur de politiques.** Pas de contournement, pas de drapeau `debug_bypass`, pas d'exception « juste pour tester ».
2. **Chaque appel d'outil produit deux entrées d'audit** : l'intention avant exécution, le résultat après. Une tâche interrompue laisse donc une trace de son intention.
3. **Le journal d'audit est en ajout seul et chaîné par hachage.** Aucune API de modification ou de suppression n'existe dans le code. `audit verify` doit détecter toute altération.
4. **Toute tâche a un plafond de coût.** Le dépassement interrompt la tâche proprement et la marque `aborted_cost_ceiling`. Il ne la dégrade jamais silencieusement.
5. **Le contenu externe n'est jamais concaténé aux instructions.** Trois types distincts, non interchangeables : `SystemInstruction`, `UserRequest`, `ExternalContent`. Le compilateur doit rendre la confusion impossible. Un document ne peut jamais devenir une règle.
6. **Une procédure métier n'entre en mémoire qu'après validation humaine** (`validated_by` non nul). L'agent ne décide jamais seul de ce qui devient une règle de l'entreprise.
7. **Toute sortie destinée à un humain porte la mention de son origine IA** (AI Act, article 50, applicable depuis le 2 août 2026).
8. **Aucune donnée ne sort de l'instance** sans un `ModelProvider` explicitement déclaré comme externe dans la configuration, et chaque sortie est journalisée avec le nom du fournisseur.
9. **`kollega-core` ne dépend d'aucune entrée-sortie.** Pas de `sqlx`, pas de `reqwest`, pas de `tokio` dans le domaine. Le test : `cargo tree -p kollega-core` doit tenir sur un écran.
10. **Aucune suppression physique de donnée.** Effacement logique + purge explicite, tracée, déclenchée par une commande dédiée (droit à l'effacement RGPD).
11. **Une instance = un client.** `org_id` existe dans le schéma pour préparer l'avenir, mais aucune logique métier ne branche dessus. Pas de multi-tenant tant qu'il n'y a pas 20 clients.
12. **Toute migration est réversible**, ou explicitement marquée irréversible avec sa justification dans le fichier.

---

## 4. PÉRIMÈTRE

### On construit (V0 — six capacités)
1. Un runtime qui exécute une mission : percevoir → planifier → appeler un outil → vérifier → journaliser
2. Un moteur de politiques qui autorise, refuse ou exige une validation pour chaque appel d'outil
3. Un journal d'audit inaltérable, chaîné
4. Trois outils réels seulement : lecture d'une source documentaire, écriture d'un document, lecture d'une boîte mail
5. Une mémoire : documentaire (recherche) + procédurale (règles validées)
6. Un écran unique : file d'attente de validation humaine, avec le coût par tâche

### On ne construit JAMAIS sans une demande client payée deux fois
Studio de création d'agents sans code · marketplace, App Store, SDK tiers, certification · manager IA, essaim d'agents, vote inter-agents · score de confiance, salaire virtuel, organigramme, évaluation de performance · connecteurs SAP/Salesforce/HubSpot/Odoo · application mobile, Electron · Kubernetes, multi-régions, microservices · knowledge graph, jumeau numérique · ClickHouse, Redis, RabbitMQ, NATS, Temporal · tableau de bord ROI

### Interdits permanents (risque réglementaire, pas arbitrage produit)
Tri de CV, aide au recrutement, évaluation de personnes, scoring de crédit, notation d'individus. Ce sont des usages de l'annexe III de l'AI Act : les fournir ferait de nous un fournisseur de système à haut risque (documentation technique, évaluation de conformité, enregistrement européen). **Ce n'est pas « plus tard ». C'est non.**

---

## 5. ARCHITECTURE — DÉCISIONS DÉJÀ TRANCHÉES

Ne les rouvre pas sans ADR.

| Sujet | Décision | Raison |
|---|---|---|
| Découpage | Monolithe modulaire : un binaire, des crates aux frontières nettes | Un développeur seul. On extraira un service quand une contrainte réelle l'imposera. |
| Langage cœur | Rust (édition 2021, tokio, axum, sqlx) | Base existante réutilisable, garanties à la compilation sur les invariants 5 et 9 |
| Langage périphérie | Python, exposé **uniquement** comme serveurs MCP | Écosystème d'extraction. Frontière stricte : Python ne décide jamais. |
| Stockage | PostgreSQL 16 + pgvector, **seul moteur** | File d'attente via `SELECT … FOR UPDATE SKIP LOCKED`. Transactions, vecteurs et journal dans un seul système à sauvegarder. |
| Événements | Table en ajout seul dans Postgres | Audit et rejeu sans bus de messages. |
| Outils | Protocole MCP, y compris pour les outils internes | Standard, déjà pratiqué sur `vibed`, compatibilité gratuite avec l'écosystème. |
| Modèles | API externe en développement ; exécution locale seulement si un client la finance | Un GPU H100 en continu coûte ~1 965 €/mois. |
| Interface | Une seule application web, rendue par le serveur | La validation humaine se fait sur un écran, dans la journée de travail. |
| Livraison | Une image OCI unique, signée (cosign), avec SBOM | C'est le différenciant commercial, et la pratique déjà en place sur VibeOS. |

---

## 6. DISCIPLINE DE TRAVAIL

- **Une session Claude Code = un jalon.** On ne commence pas M+1 avant que la définition de terminé de M soit verte.
- **Aucune fonctionnalité hors périmètre**, même si elle est facile, même si elle prend dix minutes. Note-la dans `docs/backlog.md`, ne la code pas.
- **Si une décision d'architecture apparaît en cours de route** : arrête, écris l'ADR dans `docs/adr/`, puis code.
- **Si une instruction de conversation contredit ce fichier** : ce fichier gagne. Signale la contradiction plutôt que de l'ignorer.
- **Ne jamais générer de spécification anticipée.** Ce dépôt existe pour remplacer un document de 19 volumes écrit avant le premier client.

### Définition de terminé, pour toute PR
- [ ] `cargo test --workspace` vert
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` vert
- [ ] `cargo fmt --check` vert
- [ ] Migration réversible, ou irréversibilité justifiée en commentaire
- [ ] Aucun secret dans le code, l'historique ou les tests
- [ ] Les invariants touchés sont couverts par un test qui échoue si on les viole
- [ ] Si la boucle d'agent est modifiée : le test de non-régression de coût est passé et le nouveau coût est noté
- [ ] ADR écrit si une décision d'architecture a été prise

---

## 7. CONVENTIONS

- Commits : `type(scope): sujet` en français, à l'impératif. Types : `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `adr`.
- Erreurs : `thiserror` dans les bibliothèques, `anyhow` uniquement dans les binaires.
- Pas de `unwrap()` ni `expect()` hors tests et hors démarrage du binaire.
- Journalisation : `tracing`, avec `task_id` et `tool_call_id` dans le contexte de chaque portée.
- Tout coût est manipulé en **centimes entiers** (`Cents(i64)`). Jamais de flottant pour de l'argent.
- SQL écrit à la main, vérifié à la compilation via `sqlx::query!`. Pas d'ORM.
- Documentation en français. Noms de code, types et identifiants SQL en anglais.
