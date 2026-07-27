-- Retour arrière : tables de la tranche, ordre inverse (aucune FK entre
-- elles, toutes référencent organizations). Les politiques RLS et les
-- grants tombent avec leurs tables.
DROP TABLE IF EXISTS credits;
DROP TABLE IF EXISTS audit_content;
DROP TABLE IF EXISTS audit_chain;
DROP TABLE IF EXISTS tasks;
