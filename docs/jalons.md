# JALONS M0 – M7 — PLATEFORME MULTI-TENANT

Une session Claude Code = un jalon. `/clear` entre deux. `CLAUDE.md` (v2) étant à la racine, il est rechargé automatiquement : ne le recopie pas dans les prompts.

**Ordre non négociable :** la Row-Level Security se pose à M0. Elle ne se rétrofite pas.

---

## M0 — SOCLE MULTI-TENANT

```
Jalon M0 : squelette du projet ET fondation multi-tenant. Aucune logique métier.

Ce jalon est le plus important de tous : l'isolation se décide ici, ou jamais.

À produire :
1. Workspace Cargo, crates vides qui compilent, graphe de dépendances imposé,
   test d'intégration continue qui échoue si sqlx/reqwest/tokio apparaît dans
   kollega-core.
2. Migration 0001 : extensions (pgvector), rôles PostgreSQL.
   - rôle kollega_migrate : propriétaire du schéma, applique les migrations
   - rôle kollega_app : rôle d'exécution, SANS BYPASSRLS, sans droit de
     modifier les politiques
   L'application se connecte UNIQUEMENT avec kollega_app.
3. Migration 0002 : organizations, users, et pour chaque table portant org_id :
   ALTER TABLE ... ENABLE ROW LEVEL SECURITY;
   ALTER TABLE ... FORCE ROW LEVEL SECURITY;
   CREATE POLICY tenant_isolation ON ... USING (org_id = current_setting('app.current_org')::uuid);
4. kollega-store : un point de passage UNIQUE pour ouvrir une transaction, qui
   exécute SET LOCAL app.current_org avant toute requête. Aucun autre chemin
   d'accès à la base ne doit exister dans le code — c'est ce point unique qu'on
   auditera et qu'on testera.
5. Le test d'isolation (invariant 1), écrit pour échouer si la RLS tombe :
   insérer des données pour org A et org B ; dans le contexte de A, un SELECT
   sans clause WHERE ne retourne QUE les lignes de A. Même test avec la clause
   de politique retirée => le test doit échouer.
6. kollega-cli : serve, migrate, version. Endpoint GET /health vérifiant la base.
7. CI : fmt, clippy -D warnings, test, vérification du graphe, build image,
   SBOM, signature cosign.
8. deploy/Containerfile + compose.yaml (app + PostgreSQL 16 + pgvector).
9. docs/adr/0001 (le pivot, fourni), 0002-rls-des-la-premiere-migration.md,
   0003-postgres-seul-moteur.md.

Terminé quand : le test d'isolation passe, ET échoue bien quand on désactive
manuellement la politique RLS. C'est la seule preuve qui compte.

Rien d'autre. Pas de types métier, pas d'agent, pas d'interface.
```

---

## M1 — IDENTITÉ, AUDIT, POLITIQUES

```
Jalon M1 : qui es-tu, qu'as-tu le droit de faire, et qu'as-tu fait.

À produire :
1. kollega-core : les six types du domaine validés (Segment, Cents, CostCeiling,
   Decision, Risk, TaskStatus), avec leurs tests unitaires purs. Le fichier
   validé le 26/07/2026 est repris tel quel.
2. Inscription et authentification : création d'une organisation, compte
   administrateur, invitation d'utilisateurs, rôles (proprietaire, membre).
   Session par cookie, mots de passe en argon2. Pas de SSO, pas de SAML.
3. kollega-audit : AuditSink sur PostgreSQL.
   - hash = SHA-256(prev_hash || payload canonique || horodatage)
   - sérialisation canonique et stable dans le temps, sinon la vérification
     cassera au premier changement de version de serde
   - AUCUNE fonction d'update ni de delete sur audit_log
   - verify_chain() retourne la première rupture
   - chaîne PAR ORGANISATION : une org ne doit pas pouvoir invalider la chaîne
     d'une autre
4. kollega-policy : PolicyEngine lisant les politiques par organisation.
   Par défaut : refus. Un outil sans politique explicite est refusé.
   Seuils minimaux : montant maximal, nombre de destinataires, chemins autorisés.
5. Tests d'invariants 2, 3, 4 (et rappel du 1 sur les nouvelles tables).

Terminé quand : altérer une ligne d'audit_log en base fait échouer verify_chain,
et chaque nouvelle table a sa politique RLS et son test.
```

---

## M2 — CONNECTEURS OAUTH

```
Jalon M2 : l'entreprise branche ses outils. C'est la première moitié des
« quelques minutes ».

À produire :
1. kollega-tools : client MCP (stdio et socket), registre d'outils par
   organisation, implémentation du trait Tool par pont MCP.
2. Connecteur Microsoft 365 (OAuth 2.0, Entra ID) : mail lecture, fichiers
   lecture/écriture, calendrier lecture.
3. Connecteur Google Workspace (OAuth 2.0) : Gmail lecture, Drive
   lecture/écriture, Agenda lecture.
   AUCUN AUTRE CONNECTEUR. Deux, c'est le périmètre.
4. Invariant 8 — le point critique de ce jalon :
   - jetons chiffrés au repos (chiffrement authentifié, clé hors base)
   - jamais en clair en base, en journal, en trace ni en message d'erreur
   - déchiffrés au dernier moment, en mémoire, pour la durée de l'appel
   - rafraîchissement et révocation gérés ; révocation côté client = connecteur
     désactivé proprement, pas une erreur en boucle
   - un test vérifie qu'aucun jeton n'apparaît dans les journaux
5. tools/ingest : serveur MCP Python, extract(source) -> texte + métadonnées
   (PDF, docx, xlsx, images avec OCR). Il ne décide de rien.
6. Parcours de connexion dans l'interface : autoriser, voir l'état, révoquer.

Terminé quand : un dirigeant connecte son Microsoft 365 en moins de 2 minutes,
un outil s'exécute sous politique, journalisé, et aucun jeton n'est lisible en
base ni en journal.
```

---

## M3 — RUNTIME ET CRÉDITS

```
Jalon M3 : l'agent travaille, et ne peut pas ruiner personne.

À produire :
1. kollega-model : trait ModelProvider + implémentation API externe
   (is_external() == true, sortie de données journalisée) + MockModelProvider
   déterministe rejouant des réponses enregistrées.
2. Crédits (invariant 5) — à traiter avant la boucle, pas après :
   - solde prépayé par organisation
   - vérification AVANT chaque appel de modèle, débit atomique APRÈS
   - solde insuffisant = arrêt immédiat, statut explicite, notification
   - un test de concurrence : deux tâches en parallèle ne peuvent pas faire
     passer le solde sous zéro
3. kollega-runtime : la boucle percevoir → planifier → appeler → vérifier →
   journaliser.
   - plafond de coût vérifié APRÈS CHAQUE appel, jamais à la fin
   - suspension en WaitingApproval reprise-compatible : le processus redémarre,
     la tâche repart où elle s'est arrêtée
   - sortie propre en AbortedCostCeiling, distincte d'un échec
4. File d'attente sur tasks via SELECT ... FOR UPDATE SKIP LOCKED, respectant
   le contexte d'organisation.
5. Six tests de référence : nominal / refusé par politique / validation requise /
   plafond atteint / crédit épuisé / redémarrage en cours de tâche.
6. Test de non-régression de coût : mesure des tokens sur un scénario type,
   échec si +15 %.
7. docs/economie-unitaire.md rempli avec les chiffres réellement mesurés.

Terminé quand : les six scénarios passent, et le coût réel par tâche est écrit
dans la documentation.

C'est le jalon le plus important après M0. Ne le bâcle pas.
```

---

## M4 — MODÈLES D'AGENTS ET ONBOARDING

```
Jalon M4 : la promesse. « Un agent au travail en quelques minutes. »

Un studio vide ne produit pas un agent utile en cinq minutes. Des modèles, si.

À produire :
1. Format de modèle d'agent, versionné, en base :
   - prompt système, outils requis, politiques par défaut, seuils de validation,
     plafonds de coût par défaut, connecteurs nécessaires
2. Trois modèles seulement, choisis parce qu'ils sont hors annexe III :
   - Tri et réponse de premier niveau sur une boîte mail générique
   - Extraction de données depuis des documents entrants vers un tableau
   - Rédaction de comptes rendus à partir de notes et de pièces jointes
3. Parcours d'activation, chronométré, cible < 5 minutes :
   connecter un outil → choisir un modèle → l'agent s'exécute sur 3 éléments
   réels en mode simulation → le dirigeant voit le résultat et le coût estimé →
   il active
4. Le mode simulation est obligatoire avant première activation : aucune action
   d'écriture, résultat affiché, coût projeté.
5. Personnalisation limitée et bornée : ton, seuils, périmètre des dossiers.
   PAS d'édition libre du prompt système à ce stade.
6. Un test chronométré en CI mesure le parcours complet et échoue au-delà de
   la cible.

Terminé quand : un utilisateur test, sans aide, active un agent en moins de
5 minutes et voit un résultat sur ses vraies données.
```

---

## M5 — MÉMOIRE MULTI-TENANT

```
Jalon M5 : l'agent connaît l'entreprise — la sienne, et rien qu'elle.

À produire :
1. Chaîne d'ingestion : source connectée → extraction (MCP Python) → découpage
   respectant la structure du document → embeddings → indexation.
2. Recherche hybride : vectorielle (pgvector, HNSW) + plein texte (tsvector),
   fusion des classements.
3. Invariant 1 appliqué au vectoriel — le piège de ce jalon : un index HNSW
   ignore les tenants. La RLS filtre, mais il faut le PROUVER.
   Test obligatoire : org A insère des documents, org B lance une recherche
   sémantique sur un contenu identique, et n'obtient AUCUN fragment de A.
4. Filtrage par classification et permissions, en plus de l'isolation.
5. Procédures : extraction de règles candidates, mises en file de validation.
   Invariant 9 : une procédure sans validated_by n'entre JAMAIS dans un contexte
   d'agent. Test dédié.
6. Jeu d'évaluation : 50 questions réelles, rappel@5 mesuré en CI.
7. Purge et export par organisation (invariant 12), tracés.

Terminé quand : le test d'isolation vectorielle passe, le rappel@5 est consigné,
et l'export RGPD d'une organisation fonctionne.

Pas de knowledge graph. Hors périmètre.
```

---

## M6 — INTERFACE, VALIDATION, FACTURATION

```
Jalon M6 : le dirigeant garde la main, et il paie.

À produire :
1. kollega-api : rendu serveur (askama ou maud), SSE pour le temps réel.
   Pas de framework JavaScript.
2. Tableau de bord dirigeant, en trois zones :
   - file de validation : quoi, pourquoi, quelle source, quel coût, quel risque,
     avec Approuver / Refuser / Détail
   - agents actifs, tâches en cours, coût accumulé en direct
   - crédit restant, avec alerte de seuil bas
3. Historique consultable, chaque tâche rejouable dans son journal, export du
   journal d'audit par le client lui-même (c'est un argument de vente : il n'a
   pas à nous le demander).
4. Invariant 10 : mention d'origine IA sur chaque vue et chaque document produit.
   Test sur chaque gabarit.
5. Facturation Stripe : abonnement mensuel + achat de crédits, webhooks,
   factures, gestion de l'échec de paiement. Aucun accès à découvert.
6. Emails transactionnels : validation en attente, crédit bas, échec d'agent.

Terminé quand : un dirigeant non technique s'inscrit, paie, active un agent,
valide une action et consulte son journal — sans notre intervention.
```

---

## M7 — MISE EN PRODUCTION

```
Jalon M7 : on exploite pour de vrai. Le jalon qu'on saute et qu'on paie cher.

À produire :
1. Déploiement sur cloud français (Scaleway ou OVHcloud), infrastructure décrite
   en code, image signée dont la signature est VÉRIFIÉE au déploiement.
2. Sauvegardes automatiques, chiffrées, hors site. RESTAURATION TESTÉE POUR DE
   VRAI sur un environnement séparé, avec le temps de restauration mesuré et
   écrit dans le runbook.
3. Supervision : disponibilité, latence, taux d'erreur, coût par organisation,
   alertes. Page d'état publique.
4. Journaux sans donnée client ni jeton, rétention définie.
5. Conformité, exigible dès le premier client payant :
   - contrat de sous-traitance RGPD (article 28)
   - registre des sous-traitants ultérieurs (hébergeur, fournisseur de modèle,
     Stripe, envoi d'emails) — publié
   - procédure de notification de violation, écrite et datée
   - CGU/CGV, politique de confidentialité
6. docs/runbook.md : démarrage, arrêt, diagnostic, incidents fréquents,
   restauration, procédure de fuite de données.
7. Un test de déploiement complet en CI, depuis une infrastructure vierge.

Terminé quand : la restauration d'une sauvegarde est prouvée sur un
environnement séparé, et le registre des sous-traitants est publié.
```

---

## APRÈS M7

Ne planifie rien. La suite est décidée par ce que trois clients payants ont demandé deux fois. Relis `docs/backlog.md` et compte les occurrences.

Le déploiement souverain (VibeOS) s'active quand un client le finance explicitement — voir ADR-0001, section « Position de VibeOS ».

---

## RAPPELS DE SESSION

```
Rappel : vérifie CLAUDE.md. Ce code est-il demandé par le jalon en cours ?
Sinon, note dans docs/backlog.md et reviens au jalon.
```

```
Cette table porte-t-elle org_id ? Alors : politique RLS + test d'isolation.
Montre-les-moi.
```

```
Avant de conclure ce jalon : passe la définition de terminé de CLAUDE.md ligne
par ligne et dis-moi lesquelles ne sont pas satisfaites.
```
