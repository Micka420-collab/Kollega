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
//! Invariants portés ici (numérotation de CLAUDE.md — l'invariant 1 est la
//! RLS, hors de cette crate) :
//! - **2** : tout appel d'outil passe par [`PolicyEngine::decide`] — il n'y a
//!   aucune branche d'exécution d'outil qui ne soit précédée d'une décision.
//! - **3** : tout appel d'outil produit `ToolCallIntended` AVANT et
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

/// Décide de l'autorisation d'un appel d'outil (invariant 2 de CLAUDE.md).
pub trait PolicyEngine {
    /// Décision pour l'outil nommé.
    fn decide(&self, tool: &str) -> Decision;
}

/// Exécute un outil autorisé (trait, pour découpler des vrais outils).
pub trait ToolRunner {
    /// Exécute l'outil de l'itération `iteration` ; retourne une trace de
    /// résultat (opaque ici).
    ///
    /// L'ITÉRATION EST DANS LA SIGNATURE, et ce n'est pas décoratif : un
    /// exécuteur ne peut être IDEMPOTENT que s'il sait QUEL appel il
    /// exécute. Avec `(task_id, iteration)` — dérivable, donc identique
    /// après un rejeu — la périphérie peut reconnaître un effet déjà
    /// réalisé au lieu de l'accomplir deux fois (double envoi de mail).
    /// La v1 du trait ne recevait que le nom de l'outil : elle rendait
    /// l'idempotence inexprimable.
    fn run(&self, tool: &str, iteration: u32) -> String;
}

/// Événement d'audit émis par la boucle (invariant 3 de CLAUDE.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEvent {
    /// Début de tâche.
    TaskStarted,
    /// Intention d'appeler un outil — émise AVANT toute exécution.
    ToolCallIntended {
        /// Outil visé.
        tool: String,
        /// Itération : avec le `task_id`, elle IDENTIFIE l'appel (clé
        /// dérivable, stable après un rejeu — cf. [`ToolRunner::run`]).
        iteration: u32,
    },
    /// Appel d'outil terminé — émis APRÈS l'exécution.
    ToolCallCompleted {
        /// Outil exécuté.
        tool: String,
        /// Itération identifiant l'appel.
        iteration: u32,
        /// Coût comptabilisé.
        cost: Cents,
    },
    /// Appel refusé par la politique.
    ToolCallDenied {
        /// Outil refusé.
        tool: String,
        /// Itération identifiant l'appel.
        iteration: u32,
        /// Motif.
        reason: String,
    },
    /// Validation humaine demandée.
    ApprovalRequested {
        /// Outil en attente.
        tool: String,
        /// Itération identifiant l'appel suspendu.
        iteration: u32,
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
    /// Itération de l'appel suspendu — PERSISTÉE, pas déduite de l'ordre :
    /// après une reprise, c'est elle qui redonne l'identité de l'appel.
    pub iteration: u32,
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
///
/// Historique : v1 (28/07, forme initiale) ; **v2 (nuit du 28 au 29/07 :
/// `iteration` ajoutée aux événements d'appel d'outil et à
/// [`PendingApproval`], pour rendre l'identité d'un appel dérivable —
/// prérequis de l'idempotence).** Le mécanisme a servi dès son premier
/// changement de forme réel : une tâche v1 suspendue est refusée
/// proprement au lieu d'être mal désérialisée.
pub const TASK_STATE_FORMAT_VERSION: u32 = 2;

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
        // Un WaitingApproval sans pending est un état incohérent — mais il
        // est REPRÉSENTABLE (champs publics, état relu depuis une enveloppe
        // persistée) : échec propre de la tâche, jamais une panique du
        // worker (convention du dépôt : pas d'expect hors tests).
        let Some(pending) = state.pending.take() else {
            state.audit.push(AuditEvent::ToolCallDenied {
                tool: "?".to_owned(),
                iteration: state.iteration,
                reason: "état incohérent : suspension sans appel en attente — tâche close en échec"
                    .to_owned(),
            });
            finish(state, TaskStatus::Failed);
            return;
        };
        state.audit.push(AuditEvent::ApprovalResolved { decision });
        match decision {
            ApprovalDecision::Reject => {
                state.audit.push(AuditEvent::ToolCallDenied {
                    tool: pending.tool.clone(),
                    iteration: pending.iteration,
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
                // L'itération vient du PENDING PERSISTÉ, pas du compteur
                // courant : après une reprise, c'est ce qui redonne à
                // l'appel la même identité qu'avant l'interruption.
                tools.run(&pending.tool, pending.iteration);
                state.audit.push(AuditEvent::ToolCallCompleted {
                    tool: pending.tool,
                    iteration: pending.iteration,
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
                let iteration = state.iteration;
                // Invariant 3 : intention AVANT toute exécution.
                state.audit.push(AuditEvent::ToolCallIntended {
                    tool: tool.clone(),
                    iteration,
                });
                // Invariant 2 : passage obligatoire par la politique.
                match policy.decide(&tool) {
                    Decision::Deny { reason } => {
                        state.audit.push(AuditEvent::ToolCallDenied {
                            tool: tool.clone(),
                            iteration,
                            reason,
                        });
                        finish(state, TaskStatus::Failed);
                        return;
                    }
                    Decision::RequireApproval { .. } => {
                        state.audit.push(AuditEvent::ApprovalRequested {
                            tool: tool.clone(),
                            iteration,
                        });
                        state.pending = Some(PendingApproval {
                            tool,
                            iteration,
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
                            // trace d'une tâche interrompue (invariant 3).
                            finish(state, state.status);
                            return;
                        }
                        tools.run(&tool, iteration);
                        state.audit.push(AuditEvent::ToolCallCompleted {
                            tool,
                            iteration,
                            cost: total,
                        });
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

    /// Exécuteur d'écho qui ENREGISTRE ce qu'il a exécuté : c'est ce
    /// registre qui prouve la non-répétition d'un effet.
    #[derive(Default)]
    struct EchoTools {
        executed: std::cell::RefCell<Vec<(String, u32)>>,
    }
    impl ToolRunner for EchoTools {
        fn run(&self, tool: &str, iteration: u32) -> String {
            self.executed
                .borrow_mut()
                .push((tool.to_owned(), iteration));
            format!("résultat de {tool} (itération {iteration})")
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
        drive(state, &model, &policy, &EchoTools::default(), approval);
    }

    /// Comme [`run`], mais rend le registre des exécutions réelles.
    fn run_recording(
        actions: Vec<PlannedAction>,
        decisions: Vec<(String, Decision)>,
        state: &mut TaskState,
        approval: Option<ApprovalDecision>,
    ) -> Vec<(String, u32)> {
        let model = ScriptedModel { actions };
        let policy = MapPolicy {
            decisions: decisions.into_iter().collect(),
        };
        let tools = EchoTools::default();
        drive(state, &model, &policy, &tools, approval);
        tools.executed.into_inner()
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
        // Invariant 3 : l'appel exécuté a bien deux entrées.
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
        // Intention journalisée, pas de complétion : invariant 3.
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
    fn inconsistent_suspension_without_pending_fails_cleanly() {
        // L'état « WaitingApproval sans pending » est incohérent mais
        // REPRÉSENTABLE (champs publics, enveloppe relue) : la machine doit
        // clore la tâche en échec tracé, jamais paniquer le worker.
        let mut state = TaskState::new(8, budget(10_000, 10_000));
        state.status = TaskStatus::WaitingApproval;
        state.pending = None;
        run(vec![], vec![], &mut state, Some(ApprovalDecision::Approve));
        assert_eq!(state.status, TaskStatus::Failed);
        assert!(state.audit.iter().any(|event| matches!(
            event,
            AuditEvent::ToolCallDenied { reason, .. } if reason.contains("incohérent")
        )));
        assert_eq!(state.budget.consumed(), Cents::ZERO);
    }

    #[test]
    fn envelope_round_trips_and_carries_the_current_version() {
        let state = TaskState::new(8, budget(10_000, 10_000));
        let sealed = TaskStateEnvelope::seal(state.clone());
        assert_eq!(sealed.version(), TASK_STATE_FORMAT_VERSION);
        let json = serde_json::to_string(&sealed).unwrap();
        // Comparé à la CONSTANTE, jamais à un littéral : ce test ne doit pas
        // devenir une corvée à chaque changement de format légitime.
        assert!(json.contains(&format!("\"version\":{TASK_STATE_FORMAT_VERSION}")));
        let reopened: TaskStateEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(reopened.into_state(), state);
    }

    #[test]
    fn unknown_envelope_version_is_refused_cleanly_not_misread() {
        // Une enveloppe d'une version future — dont l'état N'A PAS le
        // schéma courant — doit être REFUSÉE avec une erreur qui nomme la
        // version, jamais interprétée avec le schéma courant.
        let unknown = TASK_STATE_FORMAT_VERSION + 1;
        let future =
            format!(r#"{{"version":{unknown},"state":{{"forme":"inconnue de ce binaire"}}}}"#);
        let error = serde_json::from_str::<TaskStateEnvelope>(&future)
            .expect_err("une version inconnue doit être refusée");
        let message = error.to_string();
        assert!(
            message.contains(&format!("version d'enveloppe d'état inconnue : {unknown}")),
            "l'erreur doit nommer la version trouvée : {message}"
        );
        assert!(
            message.contains(&format!("supportée : {TASK_STATE_FORMAT_VERSION}")),
            "l'erreur doit nommer la version supportée : {message}"
        );

        // Une enveloppe SANS version est refusée aussi : le champ n'est pas
        // optionnel, un état nu d'avant le bloc 5 ne passe pas pour une
        // enveloppe.
        assert!(serde_json::from_str::<TaskStateEnvelope>(r#"{"state":{}}"#).is_err());
    }

    #[test]
    fn a_real_v1_envelope_is_refused_not_silently_misread() {
        // LE cas réel, et la raison d'être du dispositif : une tâche
        // suspendue AVANT l'ajout d'`iteration` (v1). Son état est
        // structurellement lisible par serde — les champs manquants sont
        // simplement absents des variantes — donc sans contrôle de version
        // elle serait MAL relue : un appel en attente sans identité, dont
        // l'idempotence ne pourrait plus reconnaître l'effet.
        let v1 = r#"{"version":1,"state":{
            "status":"waiting_approval","iteration":0,"max_iterations":8,
            "budget":{"ceiling":500,"consumed":0,"org_balance":10000},
            "audit":[{"TaskStarted":null}],
            "pending":{"tool":"mail.send","model_cost":30,"tool_cost":20},
            "conclusion":null}}"#;
        let error = serde_json::from_str::<TaskStateEnvelope>(v1)
            .expect_err("une enveloppe v1 doit être refusée par ce binaire v2");
        assert!(error
            .to_string()
            .contains("version d'enveloppe d'état inconnue : 1"));
    }

    #[test]
    fn the_resumed_call_keeps_the_identity_it_had_before_the_interruption() {
        // Prérequis de l'idempotence : après une suspension puis une
        // reprise, l'appel exécuté porte l'itération de sa PLANIFICATION,
        // pas le compteur courant — sinon un rejeu forgerait une autre
        // identité et ne reconnaîtrait jamais l'effet déjà réalisé.
        let actions = vec![
            PlannedAction::UseTool {
                tool: "doc.read".to_owned(),
                model_cost: Cents(10),
                tool_cost: Cents(20),
            },
            PlannedAction::UseTool {
                tool: "mail.send".to_owned(),
                model_cost: Cents(10),
                tool_cost: Cents(20),
            },
            PlannedAction::Conclude {
                model_cost: Cents(5),
                answer: "fini".to_owned(),
            },
        ];
        let decisions = vec![
            allow("doc.read"),
            (
                "mail.send".to_owned(),
                Decision::RequireApproval {
                    threshold: "envoi".to_owned(),
                },
            ),
        ];
        let mut state = TaskState::new(8, budget(10_000, 10_000));
        // Itération 0 exécutée, itération 1 suspendue en validation.
        let first = run_recording(actions.clone(), decisions.clone(), &mut state, None);
        assert_eq!(first, vec![("doc.read".to_owned(), 0)]);
        assert_eq!(state.status, TaskStatus::WaitingApproval);
        assert_eq!(
            state.pending.as_ref().map(|p| p.iteration),
            Some(1),
            "l'itération de l'appel suspendu est PERSISTÉE"
        );
        // Reprise : l'exécution porte bien l'itération 1, celle du plan.
        let resumed = run_recording(
            actions,
            decisions,
            &mut state,
            Some(ApprovalDecision::Approve),
        );
        assert_eq!(resumed, vec![("mail.send".to_owned(), 1)]);
        assert_eq!(state.status, TaskStatus::Succeeded);
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
