-- Retour arrière : privilèges de schéma, rôle applicatif, extension.
-- kollega_migrate n'est pas supprimé : c'est le rôle qui exécute ce retour.

REVOKE USAGE ON SCHEMA public FROM kollega_app;
REVOKE CREATE, USAGE ON SCHEMA public FROM kollega_migrate;

DROP ROLE IF EXISTS kollega_app;

DROP EXTENSION IF EXISTS vector;
