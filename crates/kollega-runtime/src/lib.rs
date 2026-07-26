//! La boucle d'agent (jalon M3) — noyau pur.
//!
//! Au stade actuel : le noyau comptable ([`budget`], invariants 5 et 6). La
//! machine à états complète (perception → plan → outil → vérification →
//! journal) et sa reprise arrivent avec un `ModelProvider` de test.

#![forbid(unsafe_code)]

pub mod budget;
pub mod machine;

pub use budget::{Budget, BudgetError, SpendDecision};
pub use machine::{
    drive, ApprovalDecision, AuditEvent, ModelProvider, PlannedAction, PolicyEngine, TaskState,
    ToolRunner,
};

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
