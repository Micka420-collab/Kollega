ALTER TABLE audit_chain
  DROP CONSTRAINT IF EXISTS audit_chain_one_attestation_per_call_action;
