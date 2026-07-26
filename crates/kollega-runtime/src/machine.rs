//! Boucle d'agent — machine à états **pure** et reprise-compatible.
//!
//! Le seul algorithme du produit, écrit une fois. Aucune entrée-sortie : le
//! modèle, les outils et la politique sont des traits ; les implémentations
//! réelles (API externe, ponts MCP, lecture des politiques en base) arrivent
//! aux jalons M2/M3. Ici, des implémentations déterministes rejouent des
//! scénarios enregistrés.
//!
//! Propriété cardinale : **rien en mémoire**. Tout l'état d'une tâche vit
//! dans [`TaskState`], entièrement sérialisable. Une tâche suspendue en
//! attente de validation peut être sérialisée, le processus redémarrer,
//! l'état relu, et la tâche reprendre au même point avec le même résultat.
//!
//! Invariants portés ici :
//! - **1** : tout appel d'outil passe par [`PolicyEngine::decide`] — il n'y a
//!   aucune branche d'exécution d'outil qui ne soit précédée d'une décision.
//! - **2** : tout appel d'outil produit `ToolCallIntended` AVANT et
//!   `ToolCallCompleted` APRÈS ; une tâche interrompue garde donc la trace de
//!   son intention (l'`Intended` sans `Completed`).
//! - **6** (et 5) : le plafond de coût et le crédit sont vérifiés après
//!   chaque appel via [`crate::Budget`] ; le dépassement sort proprement en
//!   `AbortedCostCeiling` / `AbortedCredit`.

use kollega_core::{Cents, Decision, TaskStatus};
use serde::{Deserialize, Serialize};

use crate::budget::{Budget, SpendDecision};

/// Action planifiée par le modèle à une itération.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedAction {
    /// Utiliser un outil, à un coût (modèle + outil).
    UseTool {
        /// Nom de l'outil.
        tool: String,
        /// Coût de l'appel au modèle qui a produit ce plan.
        model_cost: Cents,
        /// Coût de l'exécution de l'outil.
        tool_cost: Cents,
    },
    /// Conclure la tâche (plus aucune action nécessaire).
    Conclude {
        /// Coût de l'appel au modèle qui conclut.
        model_cost: Cents,
        /// Réponse produite pour l'humain.
        answer: String,
    },
}

/// Décision d'une validation humaine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalDecision {
    /// L'action suspendue est approuvée : elle s'exécute.
    Approve,
    /// L'action suspendue est refusée : la tâche échoue.
    Reject,
}

/// Fournit le plan à chaque itération (trait, pour découpler du vrai modèle).
pub trait ModelProvider {
    /// Plan pour l'itération `iteration` (0-indexée).
    fn plan(&self, iteration: u32) -> PlannedAction;
}

/// Décide de l'autorisation d'un appel d'outil (invariant 1).
pub trait PolicyEngine {
    /// Décision pour l'outil nommé.
    fn decide(&self, tool: &str) -> Decision;
}

/// Exécute un outil autorisé (trait, pour découpler des vrais outils).
pub trait ToolRunner {
    /// Exécute l'outil ; retourne une trace de résultat (opaque ici).
    fn run(&self, tool: &str) -> String;
}

/// Événement d'audit émis par la boucle (invariant 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEvent {
    /// Début de tâche.
    TaskStarted,
    /// Intention d'appeler un outil — émise AVANT toute exécution.
    ToolCallIntended {
        /// Outil visé.
        tool: String,
    },
    /// Appel d'outil terminé — émis APRÈS l'exécution.
    ToolCallCompleted {
        /// Outil exécuté.
        tool: String,
        /// Coût comptabilisé.
        cost: Cents,
    },
    /// Appel refusé par la politique.
    ToolCallDenied {
        /// Outil refusé.
        tool: String,
        /// Motif.
        reason: String,
    },
    /// Validation humaine demandée.
    ApprovalRequested {
        /// Outil en attente.
        tool: String,
    },
    /// Validation tranchée.
    ApprovalResolved {
        /// Décision humaine.
        decision: ApprovalDecision,
    },
    /// Fin de tâche.
    TaskFinished {
        /// Statut final.
        status: TaskStatus,
    },
}

/// Appel d'outil en attente de validation (état suspendu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingApproval {
    /// Outil en attente.
    pub tool: String,
    /// Coût modèle déjà planifié.
    pub model_cost: Cents,
    /// Coût de l'outil à exécuter si approuvé.
    pub tool_cost: Cents,
}

/// État complet d'une tâche — **entièrement sérialisable** (reprise).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskState {
    /// Statut courant.
    pub status: TaskStatus,
    /// Itération courante.
    pub iteration: u32,
    /// Nombre maximal d'itérations (config figée).
    pub max_iterations: u32,
    /// Budget (plafond, consommé, solde d'organisation).
    pub budget: Budget,
    /// Journal d'audit accumulé.
    pub audit: Vec<AuditEvent>,
    /// Appel en attente de validation, s'il y en a un.
    pub pending: Option<PendingApproval>,
    /// Réponse finale, si la tâche a conclu.
    pub conclusion: Option<String>,
}

impl TaskState {
    /// Crée l'état initial d'une tâche (statut `Pending`).
    #[must_use]
    pub fn new(max_iterations: u32, budget: Budget) -> Self {
        TaskState {
            status: TaskStatus::Pending,
            iteration: 0,
            max_iterations,
            budget,
            audit: Vec::new(),
            pending: None,
            conclusion: None,
        }
    }
}

/// Version courante du format d'enveloppe d'état de tâche (bloc 5).
///
/// À INCRÉMENTER à chaque changement de forme de [`TaskState`] ou de ses
/// composants sérialisés : sans cela, une mise en production rendrait les
/// tâches suspendues illisibles — ou pire, MAL lisibles.
pub const TASK_STATE_FORMAT_VERSION: u32 = 1;

/// Erreur d'ouverture d'une enveloppe d'état.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EnvelopeError {
    /// La version portée par l'enveloppe n'est pas celle que ce binaire
    /// sait lire : refus PROPRE, jamais une désérialisation hasardeuse.
    #[error("version d'enveloppe d'état inconnue : {found} (supportée : {supported})")]
    UnknownVersion {
        /// Version trouvée dans l'enveloppe.
        found: u32,
        /// Version que ce binaire supporte.
        supported: u32,
    },
}

/// Enveloppe versionnée de l'état de tâche — LA forme qui se persiste.
///
/// Champs privés + désérialisation validante : une enveloppe d'une version
/// inconnue ne peut pas exister en mémoire — `serde_json::from_str` échoue
/// avec une erreur explicite au lieu de désérialiser de travers. Toute
/// écriture de reprise passe par [`TaskStateEnvelope::seal`], toute lecture
/// par la désérialisation puis [`TaskStateEnvelope::into_state`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskStateEnvelope {
    version: u32,
    state: TaskState,
}

impl TaskStateEnvelope {
    /// Scelle un état dans une enveloppe à la version courante.
    #[must_use]
    pub const fn seal(state: TaskState) -> Self {
        TaskStateEnvelope {
            version: TASK_STATE_FORMAT_VERSION,
            state,
        }
    }

    /// La version du format (toujours la courante, par construction).
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Rend l'état. Infaillible : la construction et la désérialisation
    /// garantissent la version.
    #[must_use]
    pub fn into_state(self) -> TaskState {
        self.state
    }
}

impl<'de> Deserialize<'de> for TaskStateEnvelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawEnvelope {
            version: u32,
            state: serde_json::Value,
        }
        let raw = RawEnvelope::deserialize(deserializer)?;
        // La version d'abord, le contenu ensuite : une enveloppe d'une
        // autre version n'est JAMAIS interprétée avec le schéma courant —
        // c'est tout l'objet du refus propre (EnvelopeError, bloc 5).
        if raw.version != TASK_STATE_FORMAT_VERSION {
            return Err(serde::de::Error::custom(EnvelopeError::UnknownVersion {
                found: raw.version,
                supported: TASK_STATE_FORMAT_VERSION,
            }));
        }
        let state = TaskState::deserialize(raw.state).map_err(serde::de::Error::custom)?;
        Ok(TaskStateEnvelope {
            version: raw.version,
            state,
        })
    }
}

/// Applique le coût, met à jour le statut si une borne est franchie.
/// Retourne `true` si l'on peut continuer, `false` si la tâche s'arrête.
fn charge_or_stop(state: &mut TaskState, cost: Cents) -> bool {
    // `charge` ne peut pas recevoir de coût négatif ici (coûts de scénario
    // toujours ≥ 0) ; en cas d'erreur d'usage, on arrête proprement en échec.
    match state.budget.charge(cost) {
        Ok(SpendDecision::Proceed) => true,
        Ok(SpendDecision::AbortedCostCeiling { .. }) => {
            state.status = TaskStatus::AbortedCostCeiling;
            false
        }
        Ok(SpendDecision::AbortedCredit { .. }) => {
            // Le crédit épuisé n'a pas de variante dédiée dans TaskStatus :
            // c'est un échec explicite, distinct du plafond (invariant 5).
            state.status = TaskStatus::Failed;
            false
        }
        Err(_) => {
            state.status = TaskStatus::Failed;
            false
        }
    }
}

/// Fait avancer la tâche jusqu'à un état terminal ou une suspension.
///
/// - `approval` : si la tâche était suspendue, la décision humaine à
///   appliquer avant de reprendre ; ignoré sinon.
/// - S'arrête en `Succeeded` / `Failed` / `AbortedCostCeiling`, ou suspend en
///   `WaitingApproval` (état sérialisable, à reprendre par un nouvel appel).
pub fn drive(
    state: &mut TaskState,
    model: &dyn ModelProvider,
    policy: &dyn PolicyEngine,
    tools: &dyn ToolRunner,
    approval: Option<ApprovalDecision>,
) {
    // Démarrage.
    if state.status == TaskStatus::Pending {
        state.audit.push(AuditEvent::TaskStarted);
        state.status = TaskStatus::Running;
    }

    // Reprise d'une suspension : appliquer la validation humaine.
    if state.status == TaskStatus::WaitingApproval {
        let Some(decision) = approval else {
            return; // toujours suspendu, rien à appliquer
        };
        let pending = state
            .pending
            .take()
            .expect("un état WaitingApproval porte toujours un pending");
        state.audit.push(AuditEvent::ApprovalResolved { decision });
        match decision {
            ApprovalDecision::Reject => {
                state.audit.push(AuditEvent::ToolCallDenied {
                    tool: pending.tool.clone(),
                    reason: "refusé par validation humaine".to_owned(),
                });
                finish(state, TaskStatus::Failed);
                return;
            }
            ApprovalDecision::Approve => {
                let total = pending.model_cost.saturating_add(pending.tool_cost);
                if !charge_or_stop(state, total) {
                    finish(state, state.status);
                    return;
                }
                tools.run(&pending.tool);
                state.audit.push(AuditEvent::ToolCallCompleted {
                    tool: pending.tool,
                    cost: total,
                });
                state.iteration += 1;
            }
        }
        state.status = TaskStatus::Running;
    }

    // Boucle principale.
    while state.status == TaskStatus::Running {
        if state.iteration >= state.max_iterations {
            finish(state, TaskStatus::Failed);
            return;
        }
        match model.plan(state.iteration) {
            PlannedAction::Conclude { model_cost, answer } => {
                if !charge_or_stop(state, model_cost) {
                    finish(state, state.status);
                    return;
                }
                state.conclusion = Some(answer);
                finish(state, TaskStatus::Succeeded);
                return;
            }
            PlannedAction::UseTool {
                tool,
                model_cost,
                tool_cost,
            } => {
                // Invariant 2 : intention AVANT toute exécution.
                state
                    .audit
                    .push(AuditEvent::ToolCallIntended { tool: tool.clone() });
                // Invariant 1 : passage obligatoire par la politique.
                match policy.decide(&tool) {
                    Decision::Deny { reason } => {
                        state.audit.push(AuditEvent::ToolCallDenied {
                            tool: tool.clone(),
                            reason,
                        });
                        finish(state, TaskStatus::Failed);
                        return;
                    }
                    Decision::RequireApproval { .. } => {
                        state
                            .audit
                            .push(AuditEvent::ApprovalRequested { tool: tool.clone() });
                        state.pending = Some(PendingApproval {
                            tool,
                            model_cost,
                            tool_cost,
                        });
                        state.status = TaskStatus::WaitingApproval;
                        return; // suspension reprise-compatible
                    }
                    Decision::Allow => {
                        let total = model_cost.saturating_add(tool_cost);
                        if !charge_or_stop(state, total) {
                            // L'intention reste au journal, sans complétion :
                            // trace d'une tâche interrompue (invariant 2).
                            finish(state, state.status);
                            return;
                        }
                        tools.run(&tool);
                        state
                            .audit
                            .push(AuditEvent::ToolCallCompleted { tool, cost: total });
                        state.iteration += 1;
                    }
                }
            }
        }
    }
}

/// Marque la fin de tâche et journalise `TaskFinished`.
fn finish(state: &mut TaskState, status: TaskStatus) {
    state.status = status;
    state.audit.push(AuditEvent::TaskFinished { status });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Modèle déterministe : rejoue une liste d'actions par itération, puis
    /// conclut.
    struct ScriptedModel {
        actions: Vec<PlannedAction>,
    }
    impl ModelProvider for ScriptedModel {
        fn plan(&self, iteration: u32) -> PlannedAction {
            self.actions
                .get(iteration as usize)
                .cloned()
                .unwrap_or(PlannedAction::Conclude {
                    model_cost: Cents(1),
                    answer: "fin".to_owned(),
                })
        }
    }

    /// Politique déterministe par table.
    struct MapPolicy {
        decisions: BTreeMap<String, Decision>,
    }
    impl PolicyEngine for MapPolicy {
        fn decide(&self, tool: &str) -> Decision {
            self.decisions.get(tool).cloned().unwrap_or(Decision::Deny {
                reason: "aucune politique".to_owned(),
            })
        }
    }

    struct EchoTools;
    impl ToolRunner for EchoTools {
        fn run(&self, tool: &str) -> String {
            format!("résultat de {tool}")
        }
    }

    fn allow(tool: &str) -> (String, Decision) {
        (tool.to_owned(), Decision::Allow)
    }

    fn budget(ceiling: i64, balance: i64) -> Budget {
        Budget::new(Cents(ceiling), Cents(balance)).unwrap()
    }

    fn run(
        actions: Vec<PlannedAction>,
        decisions: Vec<(String, Decision)>,
        state: &mut TaskState,
        approval: Option<ApprovalDecision>,
    ) {
        let model = ScriptedModel { actions };
        let policy = MapPolicy {
            decisions: decisions.into_iter().collect(),
        };
        drive(state, &model, &policy, &EchoTools, approval);
    }

    #[test]
    fn scenario_nominal() {
        let mut state = TaskState::new(8, budget(10_000, 10_000));
        run(
            vec![
                PlannedAction::UseTool {
                    tool: "doc.read".to_owned(),
                    model_cost: Cents(10),
                    tool_cost: Cents(20),
                },
                PlannedAction::Conclude {
                    model_cost: Cents(5),
                    answer: "trié".to_owned(),
                },
            ],
            vec![allow("doc.read")],
            &mut state,
            None,
        );
        assert_eq!(state.status, TaskStatus::Succeeded);
        assert_eq!(state.conclusion.as_deref(), Some("trié"));
        assert_eq!(state.budget.consumed(), Cents(35));
        // Invariant 2 : l'appel exécuté a bien deux entrées.
        let intended = state
            .audit
            .iter()
            .filter(|e| matches!(e, AuditEvent::ToolCallIntended { .. }))
            .count();
        let completed = state
            .audit
            .iter()
            .filter(|e| matches!(e, AuditEvent::ToolCallCompleted { .. }))
            .count();
        assert_eq!((intended, completed), (1, 1));
    }

    #[test]
    fn scenario_denied_by_policy() {
        let mut state = TaskState::new(8, budget(10_000, 10_000));
        run(
            vec![PlannedAction::UseTool {
                tool: "mail.send".to_owned(),
                model_cost: Cents(10),
                tool_cost: Cents(20),
            }],
            vec![(
                "mail.send".to_owned(),
                Decision::Deny {
                    reason: "outil interdit".to_owned(),
                },
            )],
            &mut state,
            None,
        );
        assert_eq!(state.status, TaskStatus::Failed);
        // Intention journalisée, pas de complétion : invariant 2.
        assert!(state
            .audit
            .iter()
            .any(|e| matches!(e, AuditEvent::ToolCallIntended { .. })));
        assert!(!state
            .audit
            .iter()
            .any(|e| matches!(e, AuditEvent::ToolCallCompleted { .. })));
    }

    #[test]
    fn scenario_requires_approval_then_suspends() {
        let mut state = TaskState::new(8, budget(10_000, 10_000));
        run(
            vec![PlannedAction::UseTool {
                tool: "doc.write".to_owned(),
                model_cost: Cents(10),
                tool_cost: Cents(20),
            }],
            vec![(
                "doc.write".to_owned(),
                Decision::RequireApproval {
                    threshold: "écriture".to_owned(),
                },
            )],
            &mut state,
            None,
        );
        assert_eq!(state.status, TaskStatus::WaitingApproval);
        assert!(state.pending.is_some());
        // Rien n'a été facturé tant que l'humain n'a pas tranché.
        assert_eq!(state.budget.consumed(), Cents::ZERO);
    }

    #[test]
    fn scenario_cost_ceiling() {
        let mut state = TaskState::new(8, budget(25, 10_000));
        run(
            vec![PlannedAction::UseTool {
                tool: "doc.read".to_owned(),
                model_cost: Cents(10),
                tool_cost: Cents(20), // 30 > plafond 25
            }],
            vec![allow("doc.read")],
            &mut state,
            None,
        );
        assert_eq!(state.status, TaskStatus::AbortedCostCeiling);
        assert_eq!(state.budget.consumed(), Cents::ZERO, "rien facturé");
    }

    #[test]
    fn scenario_credit_exhausted() {
        let mut state = TaskState::new(8, budget(10_000, 25));
        run(
            vec![PlannedAction::UseTool {
                tool: "doc.read".to_owned(),
                model_cost: Cents(10),
                tool_cost: Cents(20), // 30 > solde 25
            }],
            vec![allow("doc.read")],
            &mut state,
            None,
        );
        assert_eq!(state.status, TaskStatus::Failed);
        assert_eq!(state.budget.org_balance(), Cents(25), "solde intact");
    }

    #[test]
    fn scenario_resume_after_interruption_is_identical() {
        // Scénario avec validation : approuver l'action puis conclure.
        let actions = vec![
            PlannedAction::UseTool {
                tool: "doc.write".to_owned(),
                model_cost: Cents(10),
                tool_cost: Cents(20),
            },
            PlannedAction::Conclude {
                model_cost: Cents(5),
                answer: "écrit".to_owned(),
            },
        ];
        let decisions = vec![(
            "doc.write".to_owned(),
            Decision::RequireApproval {
                threshold: "écriture".to_owned(),
            },
        )];

        // (a) Parcours direct : suspension, puis reprise avec approbation.
        let mut direct = TaskState::new(8, budget(10_000, 10_000));
        run(actions.clone(), decisions.clone(), &mut direct, None);
        assert_eq!(direct.status, TaskStatus::WaitingApproval);
        run(
            actions.clone(),
            decisions.clone(),
            &mut direct,
            Some(ApprovalDecision::Approve),
        );

        // (b) Reprise APRÈS sérialisation : le processus « redémarre ». Le
        // chemin de persistance est l'ENVELOPPE versionnée (bloc 5), pas le
        // TaskState nu.
        let mut suspended = TaskState::new(8, budget(10_000, 10_000));
        run(actions.clone(), decisions.clone(), &mut suspended, None);
        let serialized = serde_json::to_string(&TaskStateEnvelope::seal(suspended)).unwrap();
        let envelope: TaskStateEnvelope = serde_json::from_str(&serialized).unwrap();
        let mut rebuilt = envelope.into_state();
        run(
            actions,
            decisions,
            &mut rebuilt,
            Some(ApprovalDecision::Approve),
        );

        // Reprise depuis zéro == parcours direct : rien n'était en mémoire.
        assert_eq!(rebuilt, direct);
        assert_eq!(rebuilt.status, TaskStatus::Succeeded);
        assert_eq!(rebuilt.conclusion.as_deref(), Some("écrit"));
    }

    #[test]
    fn envelope_round_trips_and_carries_the_current_version() {
        let state = TaskState::new(8, budget(10_000, 10_000));
        let sealed = TaskStateEnvelope::seal(state.clone());
        assert_eq!(sealed.version(), TASK_STATE_FORMAT_VERSION);
        let json = serde_json::to_string(&sealed).unwrap();
        assert!(json.contains("\"version\":1"));
        let reopened: TaskStateEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(reopened.into_state(), state);
    }

    #[test]
    fn unknown_envelope_version_is_refused_cleanly_not_misread() {
        // Une enveloppe d'une version future — dont l'état N'A PAS le
        // schéma courant — doit être REFUSÉE avec une erreur qui nomme la
        // version, jamais interprétée avec le schéma courant.
        let future = r#"{"version":2,"state":{"forme":"inconnue de ce binaire"}}"#;
        let error = serde_json::from_str::<TaskStateEnvelope>(future)
            .expect_err("une version inconnue doit être refusée");
        let message = error.to_string();
        assert!(
            message.contains("version d'enveloppe d'état inconnue : 2"),
            "l'erreur doit nommer la version trouvée : {message}"
        );
        assert!(
            message.contains("supportée : 1"),
            "l'erreur doit nommer la version supportée : {message}"
        );

        // Une enveloppe SANS version est refusée aussi : le champ n'est pas
        // optionnel, un état nu d'avant le bloc 5 ne passe pas pour une
        // enveloppe.
        assert!(serde_json::from_str::<TaskStateEnvelope>(r#"{"state":{}}"#).is_err());
    }

    #[test]
    fn rejected_approval_fails_the_task() {
        let actions = vec![PlannedAction::UseTool {
            tool: "doc.write".to_owned(),
            model_cost: Cents(10),
            tool_cost: Cents(20),
        }];
        let decisions = vec![(
            "doc.write".to_owned(),
            Decision::RequireApproval {
                threshold: "écriture".to_owned(),
            },
        )];
        let mut state = TaskState::new(8, budget(10_000, 10_000));
        run(actions.clone(), decisions.clone(), &mut state, None);
        run(
            actions,
            decisions,
            &mut state,
            Some(ApprovalDecision::Reject),
        );
        assert_eq!(state.status, TaskStatus::Failed);
        assert_eq!(state.budget.consumed(), Cents::ZERO);
    }
}
