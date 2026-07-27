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
    drive, ApprovalDecision, AuditEvent, ModelProvider, PlannedAction, TaskState, ToolRunner,
};

// Retiré le 29/07 : un `crate_compiles()` vide comptait comme un test vert
// sans rien prouver de plus que le compilateur. Le runtime est éprouvé par
// `budget.rs` et `machine.rs`.
