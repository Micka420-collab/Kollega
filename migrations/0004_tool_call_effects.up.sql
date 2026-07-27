-- Idempotence des effets d'outils (dette n°1 du chemin réel).
--
-- LE PROBLÈME, rendu concret par la tranche verticale : un pas rejoué
-- (conflit de hauteur de chaîne, redémarrage, réessai) ré-exécuterait ses
-- appels d'outils. Avec un vrai connecteur, cela veut dire un SECOND mail
-- envoyé au client. Aucune quantité de prudence à l'écriture ne ferme ce
-- chemin : il faut une mémoire des effets déjà réalisés.
--
-- LA CLÉ EST DÉRIVABLE, et c'est tout le mécanisme : l'identité d'un appel
-- est SHA-256(task_id || iteration) — deux exécutions du même pas
-- calculent donc la MÊME clé, et la seconde reconnaît l'effet de la
-- première. Une clé aléatoire (uuid v4 par tentative) rendrait
-- l'idempotence impossible : c'est pourquoi la machine porte désormais
-- `iteration` dans ses événements (format d'enveloppe v2).
--
-- AJOUT SEUL, appliqué par le rôle (GRANT SELECT, INSERT — ni UPDATE ni
-- DELETE) : on ne « dé-réalise » pas un effet qui a eu lieu dans le monde.
-- Le RÉSULTAT, lui, n'est pas stocké ici : seule son empreinte l'est, et
-- le contenu vit dans audit_content, purgeable (invariant 12). Un rejeu
-- dont le contenu a été purgé échoue explicitement — il ne ré-exécute
-- surtout pas.

CREATE TABLE tool_call_effects (
  org_id UUID NOT NULL REFERENCES organizations(id),
  -- Dérivée de (task_id, iteration) : stable par construction.
  tool_call_id UUID NOT NULL,
  task_id UUID NOT NULL REFERENCES tasks(id),
  iteration BIGINT NOT NULL,
  tool TEXT NOT NULL,
  result_digest BYTEA NOT NULL,
  performed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (org_id, tool_call_id),
  -- Deuxième filet, indépendant de la dérivation : même si le calcul de
  -- l'identité changeait, deux effets ne pourraient pas coexister pour un
  -- même (tâche, itération).
  UNIQUE (org_id, task_id, iteration)
);

ALTER TABLE tool_call_effects ENABLE ROW LEVEL SECURITY;
ALTER TABLE tool_call_effects FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON tool_call_effects
  USING (org_id = current_setting('app.current_org')::uuid);
GRANT SELECT, INSERT ON tool_call_effects TO kollega_app;
