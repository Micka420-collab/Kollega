-- Retour arrière : privilèges par défaut, rôle applicatif, extension.
-- kollega_migrate n'est pas supprimé : c'est le rôle qui exécute ce retour.

ALTER DEFAULT PRIVILEGES IN SCHEMA public
  REVOKE SELECT, INSERT, UPDATE, DELETE ON TABLES FROM kollega_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
  REVOKE USAGE, SELECT ON SEQUENCES FROM kollega_app;
REVOKE USAGE ON SCHEMA public FROM kollega_app;

DROP ROLE IF EXISTS kollega_app;

DROP EXTENSION IF EXISTS vector;
