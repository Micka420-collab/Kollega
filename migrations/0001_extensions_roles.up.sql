-- Extensions et rôles (ADR-0002 : la RLS se pose à la première migration).
--
-- Deux rôles distincts :
--   * kollega_migrate : applique les migrations et possède les tables.
--     EXIGENCE D'EXPLOITATION : en production, kollega_migrate doit être un
--     propriétaire NON superutilisateur — un superutilisateur contourne toute
--     RLS, FORCE compris, ce qui rendrait la défense en profondeur décorative
--     pour le propriétaire. En CI et en compose, il est le superutilisateur de
--     bootstrap du conteneur : acceptable pour un environnement jetable.
--   * kollega_app : rôle d'exécution de l'application, LOGIN NOBYPASSRLS,
--     non propriétaire — il ne peut ni modifier une politique ni altérer le
--     schéma. L'application se connecte UNIQUEMENT avec lui.
--
-- Les privilèges de kollega_app sont accordés table par table, explicitement,
-- dans chaque migration qui crée une table. Jamais via ALTER DEFAULT
-- PRIVILEGES : son effet dépendrait du rôle qui exécute la migration, et le
-- bootstrap ne serait reproductible que là où ce rôle ne change jamais.
--
-- Aucun mot de passe ici : les mots de passe sont posés par l'exploitant
-- (KOLLEGA_APP_DB_PASSWORD lue par `kollega migrate`). Un secret n'entre
-- jamais dans une migration.

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

-- Rend le flux documenté reproductible même quand cette migration est
-- exécutée par un administrateur de bootstrap distinct (instance gérée) :
-- kollega_migrate doit pouvoir créer les tables des migrations suivantes
-- (depuis PostgreSQL 15, PUBLIC n'a plus CREATE sur le schéma public).
GRANT CREATE, USAGE ON SCHEMA public TO kollega_migrate;
GRANT USAGE ON SCHEMA public TO kollega_app;
