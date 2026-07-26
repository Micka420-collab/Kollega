-- Fondation multi-tenant : organizations, users, Row-Level Security (invariant 1).
--
-- Règles appliquées à toute table portant org_id (ADR-0002) :
--   * ENABLE + FORCE ROW LEVEL SECURITY (FORCE soumet aussi le propriétaire) ;
--   * politique unique tenant_isolation sur current_setting('app.current_org') —
--     la forme SANS missing_ok est volontaire : une requête hors contexte
--     d'organisation ÉCHOUE au lieu de retourner des lignes. Fermé par défaut.
--   * deleted_at : effacement logique seulement (invariant 12).

CREATE TABLE organizations (
  id UUID PRIMARY KEY,
  name TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ
);

ALTER TABLE organizations ENABLE ROW LEVEL SECURITY;
ALTER TABLE organizations FORCE ROW LEVEL SECURITY;
-- Sur organizations, la colonne d'isolation est la clé primaire elle-même.
CREATE POLICY tenant_isolation ON organizations
  USING (id = current_setting('app.current_org')::uuid);

CREATE TABLE users (
  id UUID PRIMARY KEY,
  org_id UUID NOT NULL REFERENCES organizations(id),
  email TEXT NOT NULL,
  role TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  -- Unicité PAR organisation : un email global unique divulguerait l'existence
  -- d'un compte d'un autre tenant et interdirait un même email dans deux orgs.
  UNIQUE (org_id, email)
);

ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE users FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON users
  USING (org_id = current_setting('app.current_org')::uuid);
