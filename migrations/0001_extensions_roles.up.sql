-- Extensions et rôles (ADR-0002 : la RLS se pose à la première migration).
--
-- Deux rôles distincts :
--   * kollega_migrate : propriétaire du schéma, exécute les migrations.
--   * kollega_app     : rôle d'exécution de l'application, LOGIN NOBYPASSRLS,
--                       sans droit de modifier les politiques (non propriétaire).
-- L'application se connecte UNIQUEMENT avec kollega_app.
--
-- Aucun mot de passe ici : les mots de passe sont posés par l'exploitant
-- (variable d'environnement KOLLEGA_APP_DB_PASSWORD lue par `kollega migrate`).
-- Un secret n'entre jamais dans une migration.

CREATE EXTENSION IF NOT EXISTS vector;

DO $$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'kollega_migrate') THEN
    CREATE ROLE kollega_migrate LOGIN;
  END IF;
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'kollega_app') THEN
    CREATE ROLE kollega_app LOGIN NOBYPASSRLS;
  END IF;
END
$$;

GRANT USAGE ON SCHEMA public TO kollega_app;

-- Les tables créées par les migrations futures seront lisibles et modifiables
-- par l'application (la RLS, elle, filtre les lignes) — mais jamais possédées
-- par elle : kollega_app ne peut ni modifier une politique ni altérer le schéma.
ALTER DEFAULT PRIVILEGES IN SCHEMA public
  GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO kollega_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
  GRANT USAGE, SELECT ON SEQUENCES TO kollega_app;
