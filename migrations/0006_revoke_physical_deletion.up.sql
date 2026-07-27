-- Invariant 12 porté par le RÔLE, pas par la discipline.
--
-- ÉCART CORRIGÉ : la migration 0002 accordait `DELETE` à `kollega_app` sur
-- `organizations` et `users`, alors que la constitution interdit toute
-- suppression physique (effacement logique par `deleted_at`, plus purge et
-- export explicites, tracés). Les tables ajoutées depuis — `tasks`,
-- `audit_chain`, `tool_call_effects` — ne l'ont jamais reçu ; ces deux-là
-- étaient les dernières à contredire la règle.
--
-- Ce que le rôle applicatif peut encore supprimer : `audit_content`, et
-- UNIQUEMENT lui — c'est la purge RGPD (invariant 12 également), qui passe
-- par un acte nommé (`purge_org`) et laisse une attestation dans la chaîne.
--
-- Après cette migration, « aucune suppression physique » cesse d'être une
-- convention relue en revue : une tentative échoue au niveau du rôle.

REVOKE DELETE ON organizations FROM kollega_app;
REVOKE DELETE ON users FROM kollega_app;
