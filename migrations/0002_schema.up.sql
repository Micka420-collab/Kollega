-- Schéma complet du produit (prompt maître, Partie II §4).
-- Points de vigilance :
--   * max_cost_per_task_cents NOT NULL : le plafond n'est pas une option (invariant 4).
--   * cost_cents porté par chaque tool_call, pas seulement par la tâche.
--   * audit_log.prev_hash : journal inaltérable en une colonne (invariant 3).
--     Aucune commande UPDATE ou DELETE ne doit exister dans le code pour cette table.
--   * deleted_at partout : effacement logique, purge séparée et tracée (invariant 10).
--   * org_id partout : couture multi-tenant, aucune logique métier ne branche dessus
--     (invariant 11).

CREATE TABLE organizations (
  id UUID PRIMARY KEY,
  name TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE users (
  id UUID PRIMARY KEY,
  org_id UUID NOT NULL REFERENCES organizations(id),
  email TEXT NOT NULL UNIQUE,
  role TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ
);

-- Un agent est une configuration versionnée. Ce n'est pas un « employé ».
CREATE TABLE agents (
  id UUID PRIMARY KEY,
  org_id UUID NOT NULL REFERENCES organizations(id),
  name TEXT NOT NULL,
  purpose TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('draft','active','paused')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ
);

CREATE TABLE agent_configs (
  id UUID PRIMARY KEY,
  agent_id UUID NOT NULL REFERENCES agents(id),
  version INT NOT NULL,
  system_prompt TEXT NOT NULL,
  model_provider TEXT NOT NULL,
  temperature REAL NOT NULL DEFAULT 0.2,
  max_cost_per_task_cents BIGINT NOT NULL,
  max_cost_per_call_cents BIGINT NOT NULL,
  max_iterations INT NOT NULL DEFAULT 8,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (agent_id, version)
);

CREATE TABLE policies (
  id UUID PRIMARY KEY,
  agent_id UUID NOT NULL REFERENCES agents(id),
  tool_name TEXT NOT NULL,
  allowed BOOLEAN NOT NULL,
  requires_approval BOOLEAN NOT NULL,
  threshold_json JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (agent_id, tool_name)
);

CREATE TABLE tasks (
  id UUID PRIMARY KEY,
  agent_id UUID NOT NULL REFERENCES agents(id),
  config_version INT NOT NULL,
  goal TEXT NOT NULL,
  input_ref TEXT,
  status TEXT NOT NULL,
  cost_cents BIGINT NOT NULL DEFAULT 0,
  iterations INT NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  started_at TIMESTAMPTZ,
  finished_at TIMESTAMPTZ
);
-- File d'attente : SELECT ... FOR UPDATE SKIP LOCKED (jalon M3).
CREATE INDEX tasks_status_created_at_idx ON tasks (status, created_at);

CREATE TABLE tool_calls (
  id UUID PRIMARY KEY,
  task_id UUID NOT NULL REFERENCES tasks(id),
  tool_name TEXT NOT NULL,
  args_json JSONB NOT NULL,
  decision TEXT NOT NULL,
  decision_reason TEXT,
  result_ref TEXT,
  cost_cents BIGINT NOT NULL DEFAULT 0,
  duration_ms INT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE approvals (
  id UUID PRIMARY KEY,
  task_id UUID NOT NULL REFERENCES tasks(id),
  tool_call_id UUID NOT NULL REFERENCES tool_calls(id),
  requested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  decided_at TIMESTAMPTZ,
  decided_by UUID REFERENCES users(id),
  decision TEXT,
  comment TEXT
);

-- Invariant 3 : ajout seul, chaîné. Aucune commande UPDATE ou DELETE
-- ne doit exister dans le code pour cette table.
CREATE TABLE audit_log (
  id BIGSERIAL PRIMARY KEY,
  org_id UUID NOT NULL,
  actor TEXT NOT NULL,
  action TEXT NOT NULL,
  payload_json JSONB NOT NULL,
  prev_hash BYTEA,
  hash BYTEA NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE documents (
  id UUID PRIMARY KEY,
  org_id UUID NOT NULL REFERENCES organizations(id),
  source TEXT NOT NULL,
  title TEXT NOT NULL,
  owner TEXT,
  classification TEXT NOT NULL CHECK (classification IN
    ('public','internal','confidential','restricted')),
  permissions_json JSONB NOT NULL DEFAULT '[]',
  content_hash BYTEA NOT NULL,
  ingested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ
);

CREATE TABLE chunks (
  id UUID PRIMARY KEY,
  document_id UUID NOT NULL REFERENCES documents(id),
  content TEXT NOT NULL,
  embedding vector(1024),
  position INT NOT NULL
);
CREATE INDEX chunks_embedding_idx ON chunks USING hnsw (embedding vector_cosine_ops);

-- Invariant 6 : une règle n'existe qu'après validation humaine.
CREATE TABLE procedures (
  id UUID PRIMARY KEY,
  org_id UUID NOT NULL REFERENCES organizations(id),
  statement TEXT NOT NULL,
  source_document_id UUID REFERENCES documents(id),
  confidence REAL,
  validated_by UUID REFERENCES users(id),
  validated_at TIMESTAMPTZ,
  valid_until TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
