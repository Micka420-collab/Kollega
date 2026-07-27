//! Enregistrements d'appels d'outils et validateur de séquence (blocs 3d, 3e).
//!
//! Trois variantes, pas deux champs : un appel INTERROMPU dont l'effet réel
//! est inconnu se clôt en [`AuditRecord::Abandoned`] — pas en échec. Écrire
//! un résultat en échec serait un mensonge inscrit dans une chaîne dont
//! l'unique valeur est de ne pas mentir.
//!
//! Le validateur est ASYMÉTRIQUE (bloc 3e) : la règle « tout Intent a
//! exactement un Outcome » serait fausse et dangereuse — elle rendrait une
//! panne indistinguable d'une falsification et présenterait un redémarrage
//! banal comme un journal corrompu. Un Intent sans clôture est un APPEL
//! OUVERT : une information, jamais une violation. Seules invalident : une
//! clôture sans Intent, un doublon d'Intent, un doublon de clôture, tout
//! enregistrement après clôture.

use std::collections::BTreeMap;

use kollega_core::ToolCallId;

use crate::content::ContentDigest;

/// Pourquoi un appel a été clos sans résultat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbandonReason {
    /// Redémarrage du processus : l'appel était ouvert, son effet réel est
    /// INCONNU (il a pu s'exécuter, être facturé, ou rien).
    RestartWithUnknownEffect,
    /// Rejeu d'un pas après conflit d'écriture de chaîne : le pas perdu a
    /// PU exécuter l'appel avant que sa transaction ne soit annulée.
    StepReplayAfterChainConflict,
}

/// Un enregistrement du cycle de vie d'un appel d'outil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditRecord {
    /// L'intention, AVANT toute exécution (invariant 3).
    Intent {
        /// L'appel concerné.
        tool_call_id: ToolCallId,
        /// Empreinte de la requête réelle (contenu séparé, purgeable).
        request: ContentDigest,
    },
    /// Le résultat, APRÈS exécution.
    Outcome {
        /// L'appel concerné.
        tool_call_id: ToolCallId,
        /// Empreinte du résultat réel.
        result: ContentDigest,
    },
    /// Écrit à la reprise : l'appel a été interrompu, son effet réel est
    /// INCONNU. Ce n'est pas un échec, et il ne faut pas l'écrire comme tel.
    Abandoned {
        /// L'appel concerné.
        tool_call_id: ToolCallId,
        /// Pourquoi l'appel est clos sans résultat.
        reason: AbandonReason,
    },
}

impl AuditRecord {
    /// L'appel auquel ce enregistrement appartient.
    #[must_use]
    pub fn tool_call_id(&self) -> ToolCallId {
        match self {
            AuditRecord::Intent { tool_call_id, .. }
            | AuditRecord::Outcome { tool_call_id, .. }
            | AuditRecord::Abandoned { tool_call_id, .. } => *tool_call_id,
        }
    }
}

/// Nature d'une violation de séquence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationKind {
    /// Une clôture (Outcome ou Abandoned) sans Intent préalable.
    ClosureWithoutIntent,
    /// Un second Intent pour un appel encore ouvert.
    DuplicateIntent,
    /// Une seconde clôture pour un appel déjà clos.
    DuplicateClosure,
    /// Tout enregistrement (y compris un Intent) après la clôture.
    RecordAfterClosure,
}

/// Une violation, avec sa position et l'appel concerné.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceViolation {
    /// Position (0-indexée) de l'enregistrement fautif dans la séquence.
    pub position: usize,
    /// L'appel concerné.
    pub tool_call_id: ToolCallId,
    /// Nature de la faute.
    pub kind: ViolationKind,
}

/// Rapport de vérification — PAS un booléen.
///
/// Les appels ouverts sont une information (une panne, un redémarrage, une
/// validation en attente) ; seules les [`SequenceViolation`] invalident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceReport {
    /// Appels dont l'Intent n'a pas (encore) de clôture — légitimes.
    pub open_calls: Vec<ToolCallId>,
    /// Les violations, chacune avec position et nature.
    pub violations: Vec<SequenceViolation>,
}

impl SequenceReport {
    /// Valide ssi AUCUNE violation — les appels ouverts ne comptent pas.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.violations.is_empty()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CallState {
    Open,
    Closed,
}

/// Vérifie une séquence d'enregistrements, dans l'ordre du journal.
#[must_use]
pub fn verify_sequence(records: &[AuditRecord]) -> SequenceReport {
    let mut states: BTreeMap<u128, (ToolCallId, CallState)> = BTreeMap::new();
    let mut violations = Vec::new();

    for (position, record) in records.iter().enumerate() {
        let id = record.tool_call_id();
        let key = id.as_uuid().as_u128();
        let state = states.get(&key).map(|(_, s)| *s);
        let fault = match (record, state) {
            // Ouverture propre.
            (AuditRecord::Intent { .. }, None) => {
                states.insert(key, (id, CallState::Open));
                None
            }
            // Clôture propre (Outcome ou Abandoned, indifféremment).
            (
                AuditRecord::Outcome { .. } | AuditRecord::Abandoned { .. },
                Some(CallState::Open),
            ) => {
                states.insert(key, (id, CallState::Closed));
                None
            }
            // Après clôture, TOUT enregistrement est une violation — une
            // seconde clôture porte son nom propre, le reste est générique.
            (
                AuditRecord::Outcome { .. } | AuditRecord::Abandoned { .. },
                Some(CallState::Closed),
            ) => Some(ViolationKind::DuplicateClosure),
            (AuditRecord::Intent { .. }, Some(CallState::Closed)) => {
                Some(ViolationKind::RecordAfterClosure)
            }
            // Second Intent sur un appel ouvert.
            (AuditRecord::Intent { .. }, Some(CallState::Open)) => {
                Some(ViolationKind::DuplicateIntent)
            }
            // Clôture sans Intent.
            (AuditRecord::Outcome { .. } | AuditRecord::Abandoned { .. }, None) => {
                Some(ViolationKind::ClosureWithoutIntent)
            }
        };
        if let Some(kind) = fault {
            violations.push(SequenceViolation {
                position,
                tool_call_id: id,
                kind,
            });
        }
    }

    let open_calls = states
        .into_values()
        .filter_map(|(id, state)| (state == CallState::Open).then_some(id))
        .collect();
    SequenceReport {
        open_calls,
        violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::ContentPayload;
    use uuid::Uuid;

    fn id(n: u128) -> ToolCallId {
        ToolCallId::new(Uuid::from_u128(n))
    }

    fn digest(s: &str) -> ContentDigest {
        ContentDigest::of(&ContentPayload::new(s.to_owned()))
    }

    fn intent(n: u128) -> AuditRecord {
        AuditRecord::Intent {
            tool_call_id: id(n),
            request: digest("requête"),
        }
    }

    fn outcome(n: u128) -> AuditRecord {
        AuditRecord::Outcome {
            tool_call_id: id(n),
            result: digest("résultat"),
        }
    }

    fn abandoned(n: u128) -> AuditRecord {
        AuditRecord::Abandoned {
            tool_call_id: id(n),
            reason: AbandonReason::RestartWithUnknownEffect,
        }
    }

    /// Les DEUX motifs d'abandon closent un appel de la même façon.
    ///
    /// Trouvé le 29/07 en recensant les variantes qu'aucun test n'atteint :
    /// l'utilitaire `abandoned` ci-dessus fige `RestartWithUnknownEffect`,
    /// si bien que `StepReplayAfterChainConflict` n'était jamais passé au
    /// validateur. Le motif est une INFORMATION pour qui lit le journal ;
    /// il ne doit rien changer à la sémantique de séquence. Si quelqu'un le
    /// traitait un jour à part — « un abandon de rejeu ne clôt pas vraiment
    /// l'appel » —, les deux motifs divergeraient sans qu'aucun rouge ne le
    /// dise, et un appel clos passerait pour resté ouvert.
    #[test]
    fn both_abandon_reasons_close_a_call_identically() {
        let motifs = [
            AbandonReason::RestartWithUnknownEffect,
            AbandonReason::StepReplayAfterChainConflict,
        ];
        for reason in motifs {
            let report = verify_sequence(&[
                intent(1),
                AuditRecord::Abandoned {
                    tool_call_id: id(1),
                    reason,
                },
            ]);
            assert!(
                report.is_valid(),
                "un abandon ({reason:?}) est une clôture LÉGITIME : {report:?}"
            );
            assert!(
                report.open_calls.is_empty(),
                "l'appel doit être clos par l'abandon ({reason:?}), pas rester ouvert"
            );
        }
        // Et dans l'autre sens : un abandon SANS intention préalable reste
        // une violation, quel que soit son motif — sinon il suffirait de
        // choisir le bon motif pour inscrire une clôture orpheline.
        for reason in motifs {
            let report = verify_sequence(&[AuditRecord::Abandoned {
                tool_call_id: id(2),
                reason,
            }]);
            assert_eq!(
                report.violations.len(),
                1,
                "clôture orpheline attendue pour {reason:?} : {report:?}"
            );
            assert_eq!(
                report.violations[0].kind,
                ViolationKind::ClosureWithoutIntent
            );
        }
    }

    #[test]
    fn open_intent_is_information_not_violation() {
        let report = verify_sequence(&[intent(1)]);
        assert!(
            report.is_valid(),
            "un appel ouvert est LÉGITIME : {report:?}"
        );
        assert_eq!(report.open_calls, vec![id(1)]);
    }

    #[test]
    fn intent_then_abandoned_is_valid() {
        let report = verify_sequence(&[intent(1), abandoned(1)]);
        assert!(report.is_valid(), "{report:?}");
        assert!(report.open_calls.is_empty());
    }

    #[test]
    fn closure_without_intent_is_a_violation() {
        for closure in [outcome(1), abandoned(1)] {
            let report = verify_sequence(std::slice::from_ref(&closure));
            assert_eq!(report.violations.len(), 1);
            assert_eq!(
                report.violations[0].kind,
                ViolationKind::ClosureWithoutIntent
            );
            assert_eq!(report.violations[0].position, 0);
        }
    }

    #[test]
    fn duplicate_intent_is_a_violation() {
        let report = verify_sequence(&[intent(1), intent(1)]);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].kind, ViolationKind::DuplicateIntent);
        assert_eq!(report.violations[0].position, 1);
        // L'appel reste OUVERT (le premier Intent est légitime).
        assert_eq!(report.open_calls, vec![id(1)]);
    }

    #[test]
    fn duplicate_closure_is_a_violation() {
        let report = verify_sequence(&[intent(1), outcome(1), abandoned(1)]);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].kind, ViolationKind::DuplicateClosure);
        assert_eq!(report.violations[0].position, 2);
    }

    #[test]
    fn any_record_after_closure_is_a_violation_including_intent() {
        let report = verify_sequence(&[intent(1), abandoned(1), intent(1)]);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].kind, ViolationKind::RecordAfterClosure);
    }

    #[test]
    fn mixed_sequence_reports_everything_with_positions() {
        // Appel 1 : complet. Appel 2 : ouvert. Appel 3 : clôture orpheline.
        let records = [intent(1), outcome(1), intent(2), outcome(3)];
        let report = verify_sequence(&records);
        assert!(!report.is_valid());
        assert_eq!(report.open_calls, vec![id(2)]);
        assert_eq!(
            report.violations,
            vec![SequenceViolation {
                position: 3,
                tool_call_id: id(3),
                kind: ViolationKind::ClosureWithoutIntent,
            }]
        );
    }

    #[test]
    fn a_restart_looks_like_operations_not_like_forgery() {
        // Le scénario qui a motivé l'asymétrie : une panne au milieu d'un
        // appel, un redémarrage, l'appel clos en Abandoned, la vie continue.
        let records = [intent(1), abandoned(1), intent(2), outcome(2), intent(3)];
        let report = verify_sequence(&records);
        assert!(
            report.is_valid(),
            "un redémarrage banal n'est pas une corruption"
        );
        assert_eq!(report.open_calls, vec![id(3)]);
    }
}
