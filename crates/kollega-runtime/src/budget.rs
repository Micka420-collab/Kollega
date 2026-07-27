//! Plafond de coût et crédit — noyau **pur** (invariants 5 et 6).
//!
//! Aucune entrée-sortie : la concurrence réelle (verrou de solde, débit
//! atomique) est du ressort de la base, spécifiée dans
//! `docs/credits-concurrence.md`. Ici, la décision comptable seule.
//!
//! Deux protections distinctes, vérifiées AVANT de facturer un appel — un
//! appel qui franchirait une borne est refusé sans être facturé :
//! - **Plafond de coût de la tâche** (invariant 6) : dépassement → arrêt
//!   propre en [`SpendDecision::AbortedCostCeiling`], distinct d'un échec ;
//! - **Crédit prépayé de l'organisation** (invariant 5) : solde insuffisant
//!   → arrêt immédiat en [`SpendDecision::AbortedCredit`]. Un client ne
//!   consomme jamais à découvert.
//!
//! Sémantique des bornes, cohérente avec le moteur de politiques (BLOC 3) :
//! **atteindre exactement une borne est permis, la dépasser ne l'est
//! jamais**. `consumed + cost == ceiling` passe ; `> ceiling` arrête. Un
//! `cost == org_balance` passe et met le solde à zéro ; `> org_balance`
//! arrête.

use kollega_core::Cents;
use serde::{Deserialize, Serialize};

/// Décision comptable pour un appel de coût donné.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpendDecision {
    /// L'appel est facturé ; la tâche continue.
    Proceed,
    /// Le plafond de tâche serait franchi : arrêt propre, appel NON facturé.
    AbortedCostCeiling {
        /// Raison lisible.
        reason: String,
    },
    /// Le crédit de l'organisation serait dépassé : arrêt, appel NON facturé.
    AbortedCredit {
        /// Raison lisible.
        reason: String,
    },
}

/// Erreur d'usage du budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BudgetError {
    /// Un coût négatif est un bug amont : le budget ne crédite jamais.
    #[error("coût négatif interdit")]
    NegativeCost,
    /// État initial incohérent (plafond ou solde négatif).
    #[error("état de budget négatif")]
    NegativeState,
}

/// État de budget d'une tâche : ce qui est consommé, le plafond de tâche,
/// le solde prépayé de l'organisation.
///
/// Entièrement sérialisable : c'est ce qui rend une tâche reprise-compatible
/// (rien en mémoire, tout reconstructible depuis l'état persistant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Budget {
    ceiling: Cents,
    consumed: Cents,
    org_balance: Cents,
}

impl Budget {
    /// Nouveau budget. Plafond et solde doivent être positifs ou nuls.
    pub fn new(ceiling: Cents, org_balance: Cents) -> Result<Self, BudgetError> {
        if ceiling.0 < 0 || org_balance.0 < 0 {
            return Err(BudgetError::NegativeState);
        }
        Ok(Budget {
            ceiling,
            consumed: Cents::ZERO,
            org_balance,
        })
    }

    /// Recharge le solde d'organisation depuis l'état PERSISTANT, en
    /// conservant plafond et consommé de la tâche.
    ///
    /// C'est la règle de `docs/credits-concurrence.md` rendue praticable :
    /// le `org_balance` embarqué dans un état sérialisé est un INSTANTANÉ —
    /// re-facturer contre lui, c'est débiter contre un solde périmé que
    /// d'autres tâches ont pu consommer. Toute reprise reconstruit le budget
    /// avec le solde LU (et verrouillé) en base dans la même transaction.
    pub fn refreshed(self, org_balance: Cents) -> Result<Self, BudgetError> {
        if org_balance.0 < 0 {
            return Err(BudgetError::NegativeState);
        }
        Ok(Budget {
            ceiling: self.ceiling,
            consumed: self.consumed,
            org_balance,
        })
    }

    /// Coût déjà facturé sur cette tâche.
    #[must_use]
    pub fn consumed(&self) -> Cents {
        self.consumed
    }

    /// Solde prépayé restant de l'organisation (jamais négatif).
    #[must_use]
    pub fn org_balance(&self) -> Cents {
        self.org_balance
    }

    /// Marge restante avant le plafond de tâche.
    #[must_use]
    pub fn remaining_ceiling(&self) -> Cents {
        Cents(self.ceiling.0 - self.consumed.0)
    }

    /// Facture un appel de coût `cost`, ou refuse sans facturer.
    ///
    /// Ordre des refus : le plafond de tâche d'abord (protection de coût),
    /// le crédit ensuite. Aucun débit n'a lieu en cas de refus — c'est ce
    /// qui garantit que le solde ne passe jamais sous zéro et que le
    /// consommé ne dépasse jamais le plafond.
    pub fn charge(&mut self, cost: Cents) -> Result<SpendDecision, BudgetError> {
        if cost.0 < 0 {
            return Err(BudgetError::NegativeCost);
        }
        // Plafond de tâche. Un coût qui déborderait i64 dépasse a fortiori
        // tout plafond (≤ i64::MAX) : c'est un franchissement de plafond, pas
        // une erreur — il ne doit surtout pas « repasser » sous le plafond.
        let would_consume = match self.consumed.checked_add(cost) {
            Some(v) if v.0 <= self.ceiling.0 => v,
            _ => {
                return Ok(SpendDecision::AbortedCostCeiling {
                    reason: format!(
                        "plafond de tâche atteint : {} + {} centimes > {} centimes",
                        self.consumed.0, cost.0, self.ceiling.0
                    ),
                });
            }
        };
        // Crédit de l'organisation.
        if cost.0 > self.org_balance.0 {
            return Ok(SpendDecision::AbortedCredit {
                reason: format!(
                    "crédit insuffisant : coût {} centimes > solde {} centimes",
                    cost.0, self.org_balance.0
                ),
            });
        }
        // Débit effectif — seulement ici.
        self.consumed = would_consume;
        self.org_balance = Cents(self.org_balance.0 - cost.0);
        Ok(SpendDecision::Proceed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn proceeds_within_both_bounds() {
        let mut b = Budget::new(Cents(1_000), Cents(1_000)).unwrap();
        assert_eq!(b.charge(Cents(300)).unwrap(), SpendDecision::Proceed);
        assert_eq!(b.consumed(), Cents(300));
        assert_eq!(b.org_balance(), Cents(700));
        assert_eq!(b.remaining_ceiling(), Cents(700));
    }

    #[test]
    fn exact_ceiling_is_allowed_but_one_more_aborts() {
        let mut b = Budget::new(Cents(500), Cents(10_000)).unwrap();
        assert_eq!(b.charge(Cents(500)).unwrap(), SpendDecision::Proceed);
        assert_eq!(b.remaining_ceiling(), Cents::ZERO);
        // L'appel suivant, même de 1 centime, est refusé sans facturation.
        let before = b.consumed();
        assert!(matches!(
            b.charge(Cents(1)).unwrap(),
            SpendDecision::AbortedCostCeiling { .. }
        ));
        assert_eq!(b.consumed(), before, "un refus ne facture pas");
    }

    #[test]
    fn exact_balance_is_allowed_and_reaches_zero() {
        let mut b = Budget::new(Cents(10_000), Cents(400)).unwrap();
        assert_eq!(b.charge(Cents(400)).unwrap(), SpendDecision::Proceed);
        assert_eq!(b.org_balance(), Cents::ZERO);
    }

    #[test]
    fn insufficient_credit_aborts_without_billing() {
        let mut b = Budget::new(Cents(10_000), Cents(100)).unwrap();
        assert!(matches!(
            b.charge(Cents(101)).unwrap(),
            SpendDecision::AbortedCredit { .. }
        ));
        assert_eq!(b.org_balance(), Cents(100), "solde intact après refus");
        assert_eq!(b.consumed(), Cents::ZERO);
    }

    #[test]
    fn ceiling_checked_before_credit() {
        // Coût qui viole les deux : c'est le plafond (protection de coût)
        // qui est rapporté, distinct d'un manque de crédit.
        let mut b = Budget::new(Cents(100), Cents(50)).unwrap();
        assert!(matches!(
            b.charge(Cents(200)).unwrap(),
            SpendDecision::AbortedCostCeiling { .. }
        ));
    }

    #[test]
    fn negative_cost_is_a_usage_error() {
        let mut b = Budget::new(Cents(100), Cents(100)).unwrap();
        assert_eq!(b.charge(Cents(-1)), Err(BudgetError::NegativeCost));
    }

    #[test]
    fn huge_cost_does_not_overflow_past_the_ceiling() {
        let mut b = Budget::new(Cents(i64::MAX), Cents(i64::MAX)).unwrap();
        b.charge(Cents(i64::MAX - 1)).unwrap();
        // consumed = MAX-1 ; un nouvel appel de 10 déborderait i64 :
        // il doit être refusé, pas « repasser » sous le plafond.
        assert!(matches!(
            b.charge(Cents(10)).unwrap(),
            SpendDecision::AbortedCostCeiling { .. }
        ));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(2000))]

        /// Sur toute séquence de coûts non négatifs, le solde final est ≥ 0,
        /// le consommé ≤ plafond, et la somme des débits effectifs égale
        /// exactement solde initial − solde final.
        #[test]
        fn balance_never_negative_and_debits_are_conserved(
            ceiling in 0i64..1_000_000,
            balance in 0i64..1_000_000,
            costs in proptest::collection::vec(0i64..100_000, 0..50),
        ) {
            let initial_balance = Cents(balance);
            let mut budget = Budget::new(Cents(ceiling), initial_balance).unwrap();
            let mut total_billed = 0i64;
            for c in costs {
                let before_balance = budget.org_balance().0;
                match budget.charge(Cents(c)).unwrap() {
                    SpendDecision::Proceed => {
                        let billed = before_balance - budget.org_balance().0;
                        prop_assert_eq!(billed, c, "un Proceed facture exactement le coût");
                        total_billed += billed;
                    }
                    SpendDecision::AbortedCostCeiling { .. }
                    | SpendDecision::AbortedCredit { .. } => {
                        // Un refus ne modifie rien.
                        prop_assert_eq!(budget.org_balance().0, before_balance);
                    }
                }
                // Invariants après chaque appel.
                prop_assert!(budget.org_balance().0 >= 0);
                prop_assert!(budget.consumed().0 <= ceiling);
            }
            // Conservation : somme des débits == initial − final.
            prop_assert_eq!(total_billed, initial_balance.0 - budget.org_balance().0);
            // Le consommé égale aussi la somme des débits (débit = coût).
            prop_assert_eq!(total_billed, budget.consumed().0);
        }
    }
}
