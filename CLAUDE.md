# CLAUDE.md — Constitution du dépôt (v2)

> Lu automatiquement par Claude Code au démarrage de chaque session.
> Prime sur toute suggestion contraire faite en conversation.
> Toute modification exige un ADR dans `docs/adr/`.
>
> **v2 — 26/07/2026.** Remplace la v1 (runtime on-premise, une instance par client)
> conformément à [ADR-0001](docs/adr/0001-pivot-plateforme-multi-tenant.md).

---

## 1. LE PRODUIT

**Une plateforme SaaS sur laquelle une entreprise s'inscrit, connecte ses outils, et met un agent IA au travail en quelques minutes.**

Client type : dirigeant de TPE ou PME française, 5 à 200 salariés, pas de DSI, utilise Microsoft 365 ou Google Workspace, perd des heures sur des tâches répétitives.

Promesse : *« Connectez vos outils, choisissez un agent, il travaille aujourd'hui. Vous voyez ce qu'il fait, ce qu'il coûte, et vous validez ce qui compte. »*

Ce n'est **pas** un chatbot, pas un outil pour développeurs, pas un studio générique. C'est un catalogue d'agents prêts à l'emploi, gouvernés et mesurés.

### Ce qui nous différencie
1. **La gouvernance rendue visible.** Journal inaltérable, coût affiché avant et après, validation humaine par seuil. Personne ne vend ça à une TPE.
2. **Le segment.** Les plateformes d'agents existantes visent les scale-ups technologiques. Les PME françaises non-tech sont mal servies.
3. **L'hébergement français et le RGPD natif**, pas en option payante.

---

## 2. CONTRAINTES DE PRODUCTION

- **Un seul développeur**, à temps partiel. Toute décision qui suppose une équipe est mauvaise ici.
- **Rust** pour tout ce qui décide. **Python** uniquement en serveurs MCP, pour ce qui transforme.
- **Nous exploitons une production.** Ce qu'on met en ligne, on le maintient debout.
- **Nous avançons le coût d'exécution.** D'où les crédits prépayés : un client ne consomme jamais sans avoir payé.

---

## 3. LES TREIZE INVARIANTS

Propriétés du système, pas objectifs. Une PR qui en viole un est refusée, quelle que soit sa valeur. Chacun doit avoir un test qui **échoue** si l'invariant tombe.

1. **Isolation par la base, pas par le code.** Row-Level Security active sur toute table portant `org_id`. Le rôle applicatif n'a **pas** `BYPASSRLS`. Chaque transaction commence par `SET LOCAL app.current_org`. Test : une connexion dans le contexte de l'organisation A qui interroge les données de B retourne zéro ligne, y compris en recherche vectorielle.
2. **Aucun appel d'outil ne s'exécute sans passer par le moteur de politiques.** Pas de contournement, pas de `debug_bypass`.
3. **Chaque appel d'outil produit deux entrées d'audit** : l'intention avant, le résultat après.
4. **Le journal d'audit est en ajout seul et chaîné par hachage.** Aucune API de modification ni de suppression n'existe. `audit verify` détecte toute altération.
5. **Le crédit est vérifié avant chaque appel de modèle et débité de façon atomique.** Solde insuffisant = arrêt immédiat. Un client ne peut jamais consommer à découvert.
6. **Toute tâche a un plafond de coût.** Dépassement = arrêt propre en `aborted_cost_ceiling`. Jamais de dégradation silencieuse.
7. **Le contenu externe n'est jamais concaténé aux instructions.** Trois types distincts : `SystemInstruction`, `UserRequest`, `ExternalContent`. Le compilateur rend la confusion impossible.
8. **Les jetons OAuth sont chiffrés au repos**, jamais en clair en base, jamais dans les journaux, jamais dans un message d'erreur. Un jeton n'est déchiffré qu'au moment de l'appel.
9. **Une procédure métier n'entre en mémoire qu'après validation humaine** (`validated_by` non nul).
10. **Toute sortie destinée à un humain porte la mention de son origine IA** (AI Act, article 50).
11. **`kollega-core` ne dépend d'aucune entrée-sortie.** Pas de `sqlx`, `reqwest` ni `tokio` dans le domaine.
12. **Aucune suppression physique.** Effacement logique, plus purge et export explicites par organisation (RGPD), tracés.
13. **Toute migration est réversible**, ou son irréversibilité est justifiée dans le fichier.

---

## 4. PÉRIMÈTRE

### On construit (V1 — plateforme)
1. Inscription, organisation, utilisateurs, rôles
2. Connecteurs OAuth : **Microsoft 365 et Google Workspace, ces deux-là seulement**
3. Catalogue de modèles d'agents prêts à l'emploi (pas un studio vide)
4. Runtime : percevoir → planifier → appeler un outil → vérifier → journaliser
5. Moteur de politiques par organisation, avec seuils de validation
6. Journal d'audit inaltérable, consultable par le client
7. Crédits, quotas, facturation
8. Mémoire documentaire multi-tenant, avec procédures validées
9. Une interface : tableau de bord, file de validation, coûts

### On ne construit pas avant que trois clients payants l'aient demandé
Studio de création libre · marketplace, SDK tiers, agents publiés par des tiers · connecteurs au-delà des deux premiers (SAP, Salesforce, HubSpot, Odoo, Pennylane…) · manager IA, essaim d'agents, vote inter-agents · score de confiance, salaire virtuel, organigramme · application mobile · Kubernetes, microservices, multi-région · knowledge graph, jumeau numérique · ClickHouse, Redis, RabbitMQ, NATS, Temporal · SSO d'entreprise, SAML, SCIM · déploiement on-premise (voir ADR-0001 : c'est une couture, pas un jalon)

### Interdits permanents — réglementaires, non arbitrables
Tri de CV, aide au recrutement, évaluation ou notation de personnes, scoring de crédit, décision automatisée sur un individu. Usages de l'annexe III de l'AI Act : les fournir ferait de nous fournisseur d'un système à haut risque (gestion des risques, documentation technique, évaluation de conformité, enregistrement européen, surveillance après mise sur le marché). **Ce n'est pas « plus tard ». C'est non.**

**La frontière opérationnelle : un agent traite du document, il ne décide pas sur un humain.** L'administratif RH (contrats, attestations, onboarding, questions sur les procédures) est autorisé et hors annexe III.

---

## 5. ARCHITECTURE — DÉCISIONS TRANCHÉES

Ne pas rouvrir sans ADR.

| Sujet | Décision | Raison |
|---|---|---|
| Modèle locatif | Multi-tenant, isolation par Row-Level Security PostgreSQL | Rétrofiter la RLS après coup est l'opération la plus risquée d'un SaaS. Elle se pose à la migration 0001. |
| Rôles base de données | Rôle `kollega_app` sans `BYPASSRLS` pour l'exécution ; rôle migrations distinct | Un rôle applicatif qui peut ignorer la RLS annule la RLS. |
| Contexte de tenant | `SET LOCAL app.current_org` en début de chaque transaction, via un unique point de passage | Un seul endroit à auditer, un seul endroit à tester. |
| Découpage | Monolithe modulaire : un binaire, des crates aux frontières nettes | Un développeur seul. |
| Langage cœur | Rust (édition 2021 ; tokio, axum, sqlx) | Base existante, garanties à la compilation sur les invariants 7 et 11. |
| Langage périphérie | Python, exposé uniquement en serveurs MCP | Python ne décide jamais, il transforme. |
| Stockage | PostgreSQL 16 + pgvector, seul moteur | File d'attente par `SKIP LOCKED`. Un seul système à sauvegarder et à isoler. |
| Outils et connecteurs | Protocole MCP, y compris pour les connecteurs OAuth | Ajouter un connecteur = déclarer un serveur MCP, sans toucher au runtime. |
| Modèles | API externe ; le trait `ModelProvider` garde ouverte l'exécution locale | Un GPU en continu coûte ~1 965 €/mois : hors de portée avant financement client. |
| Interface | Application web unique, rendue par le serveur | Le dirigeant valide sur un écran, dans sa journée. |
| Hébergement | Cloud français (Scaleway ou OVHcloud) | C'est l'argument commercial, et c'est cohérent avec le RGPD promis. |
| Livraison | Image OCI unique, signée (cosign), avec SBOM | Même artefact pour la production et, plus tard, pour le déploiement souverain. |
| Paiement | Stripe, abonnement + crédits de consommation | Standard, intégration rapide, conforme. |

---

## 6. DISCIPLINE DE TRAVAIL

- **Une session = un jalon.** On ne commence pas M+1 avant que la définition de terminé de M soit verte.
- **Rien hors périmètre**, même si c'est facile. Note dans `docs/backlog.md`, ne code pas.
- **Décision d'architecture en cours de route** : arrête, écris l'ADR, puis code.
- **Instruction de conversation contredisant ce fichier** : ce fichier gagne, signale la contradiction.
- **Question bloquante posée = tu t'arrêtes et tu attends la réponse.** Tu ne tranches seul que pour de l'outillage local réversible, et tu le dis.

### Définition de terminé, pour toute PR
- [ ] `cargo test --workspace` vert
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` vert
- [ ] `cargo fmt --check` vert
- [ ] **Toute nouvelle table portant `org_id` a sa politique RLS et son test d'isolation**
- [ ] Migration réversible, ou irréversibilité justifiée
- [ ] Aucun secret dans le code, l'historique ou les journaux
- [ ] Les invariants touchés ont un test qui échoue si on les viole
- [ ] Si la boucle d'agent est modifiée : test de non-régression de coût passé, nouveau coût consigné
- [ ] ADR écrit si une décision d'architecture a été prise

---

## 7. CONVENTIONS

- Commits : `type(scope): sujet` en français, à l'impératif. Types : `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `adr`.
- Erreurs : `thiserror` en bibliothèque, `anyhow` uniquement dans les binaires.
- Pas de `unwrap()` ni `expect()` hors tests et hors démarrage du binaire.
- Journalisation `tracing`, avec `org_id`, `task_id` et `tool_call_id` dans le contexte de chaque portée. **Jamais de contenu client ni de jeton dans un journal.**
- Argent en centimes entiers (`Cents(i64)`). Jamais de flottant.
- SQL écrit à la main, vérifié à la compilation via `sqlx::query!`. Pas d'ORM.
- Documentation en français. Code, types et identifiants SQL en anglais.
