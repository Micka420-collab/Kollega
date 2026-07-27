-- Une seule attestation par (appel d'outil, nature d'événement).
--
-- DÉFAUT CORRIGÉ, trouvé par la CI sur le commit qui a branché le
-- validateur de séquence (run n°32) : l'idempotence des EFFETS ne donne pas
-- la cohérence des ATTESTATIONS. Un état de tâche remis en arrière après un
-- pas déjà committé — ce qui arrive lors d'une RESTAURATION PARTIELLE de
-- sauvegarde — faisait enregistrer une seconde clôture pour le même appel :
-- le journal prétendait alors que l'outil s'était exécuté deux fois, alors
-- que l'idempotence l'avait justement empêché de le faire.
--
-- Un journal dont l'unique valeur est de ne pas mentir ne peut pas se
-- permettre cela. La contrainte le rend IMPOSSIBLE plutôt que détectable.
--
-- `tool_call_id` NULL n'est pas contraint (PostgreSQL admet plusieurs NULL
-- dans une contrainte d'unicité) : les attestations sans appel — début et
-- fin de tâche, validation, purge — restent librement répétables, ce qui
-- est correct : une tâche peut être démarrée, une purge peut avoir lieu
-- plusieurs fois.

ALTER TABLE audit_chain
  ADD CONSTRAINT audit_chain_one_attestation_per_call_action
  UNIQUE (org_id, tool_call_id, action);
