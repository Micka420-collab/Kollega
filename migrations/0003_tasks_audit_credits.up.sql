-- Tranche verticale : tâches, chaîne d'audit persistée, contenu d'audit,
-- crédits. Règles ADR-0002 sur chaque table portant org_id : ENABLE + FORCE
-- ROW LEVEL SECURITY, politique tenant_isolation fermée par défaut, grants
-- explicites par table.
--
-- Décisions de schéma qui PORTENT des invariants (le rôle fait la garantie,
-- pas la discipline) :
--   * audit_chain : PRIMARY KEY (org_id, height) — une FOURCHE de chaîne
--     (deux écrivains à la même hauteur) est une violation d'unicité, pas
--     une corruption silencieuse ; l'écrivain perdant réessaie.
--     GRANT INSERT + SELECT SEULEMENT : ni UPDATE ni DELETE — l'ajout seul
--     (invariant 4) est appliqué par le rôle, pas par convention.
--   * audit_content : PRIMARY KEY (org_id, digest) — le contenu est adressé
--     PAR ORGANISATION, jamais par l'empreinte seule (deux organisations
--     peuvent produire le même octet-à-octet sans se voir). DELETE accordé :
--     c'est la purge RGPD (invariant 12), tracée par une attestation de
--     chaîne ; la chaîne, elle, ne se purge jamais.
--   * audit_chain.timestamp_micros : BIGINT (microsecondes Unix), PAS
--     timestamptz — l'horodatage participe aux octets hachés ; un
--     aller-retour par timestamptz pourrait tronquer et casser la
--     vérification. Le type BIGINT rend la troncature inexprimable.
--   * credits.balance_cents : CHECK (>= 0) — le découvert est impossible
--     au niveau du schéma, en plus de la décision comptable Rust.
--   * tasks : pas de GRANT DELETE (invariant 12, effacement logique).

CREATE TABLE tasks (
  id UUID PRIMARY KEY,
  org_id UUID NOT NULL REFERENCES organizations(id),
  -- L'enveloppe versionnée (TaskStateEnvelope) : LA forme qui se persiste.
  state JSONB NOT NULL,
  status TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ
);

ALTER TABLE tasks ENABLE ROW LEVEL SECURITY;
ALTER TABLE tasks FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON tasks
  USING (org_id = current_setting('app.current_org')::uuid);
GRANT SELECT, INSERT, UPDATE ON tasks TO kollega_app;

CREATE TABLE audit_chain (
  org_id UUID NOT NULL REFERENCES organizations(id),
  height BIGINT NOT NULL,
  prev_hash BYTEA,
  entry_hash BYTEA NOT NULL,
  actor TEXT NOT NULL,
  action TEXT NOT NULL,
  tool_call_id UUID,
  content_digest BYTEA,
  timestamp_micros BIGINT NOT NULL,
  PRIMARY KEY (org_id, height)
);

ALTER TABLE audit_chain ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_chain FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON audit_chain
  USING (org_id = current_setting('app.current_org')::uuid);
GRANT SELECT, INSERT ON audit_chain TO kollega_app;

CREATE TABLE audit_content (
  org_id UUID NOT NULL REFERENCES organizations(id),
  digest BYTEA NOT NULL,
  content TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (org_id, digest)
);

ALTER TABLE audit_content ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_content FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON audit_content
  USING (org_id = current_setting('app.current_org')::uuid);
GRANT SELECT, INSERT, DELETE ON audit_content TO kollega_app;

CREATE TABLE credits (
  org_id UUID PRIMARY KEY REFERENCES organizations(id),
  balance_cents BIGINT NOT NULL CHECK (balance_cents >= 0)
);

ALTER TABLE credits ENABLE ROW LEVEL SECURITY;
ALTER TABLE credits FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON credits
  USING (org_id = current_setting('app.current_org')::uuid);
GRANT SELECT, INSERT, UPDATE ON credits TO kollega_app;
