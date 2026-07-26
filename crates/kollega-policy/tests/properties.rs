//! Propriétés générales du moteur de politiques (BLOC 8).

use kollega_core::{Cents, Decision};
use kollega_policy::{decide, Bound, PathBound, ToolCallRequest, ToolRule};
use proptest::prelude::*;

/// Borne de montant valide : dure seule, ou deux étages (seuil ≤ limite,
/// garanti par construction du générateur).
fn amount_bound_strategy() -> impl Strategy<Value = Bound<Cents>> {
    (
        0i64..1_000_000,
        proptest::option::of(any::<proptest::sample::Index>()),
    )
        .prop_map(|(hard, threshold_pick)| match threshold_pick {
            None => Bound::hard(Cents(hard)),
            Some(index) => {
                let threshold = index.index(hard as usize + 1) as i64;
                Bound::two_tier(Cents(threshold), Cents(hard))
                    .expect("seuil borné par la limite, par construction")
            }
        })
}

/// Borne de destinataires valide, même construction.
fn recipient_bound_strategy() -> impl Strategy<Value = Bound<u32>> {
    (
        0u32..1000,
        proptest::option::of(any::<proptest::sample::Index>()),
    )
        .prop_map(|(hard, threshold_pick)| match threshold_pick {
            None => Bound::hard(hard),
            Some(index) => {
                let threshold = index.index(hard as usize + 1) as u32;
                Bound::two_tier(threshold, hard)
                    .expect("seuil borné par la limite, par construction")
            }
        })
}

fn rule_strategy() -> impl Strategy<Value = ToolRule> {
    (
        "[a-z.]{1,10}",
        any::<bool>(),
        any::<bool>(),
        proptest::option::of(amount_bound_strategy()),
        proptest::option::of(recipient_bound_strategy()),
    )
        .prop_map(
            |(tool_name, allowed, requires_approval, amount, recipients)| ToolRule {
                tool_name,
                allowed,
                requires_approval,
                amount,
                recipients,
                paths: None,
            },
        )
}

fn request_strategy() -> impl Strategy<Value = ToolCallRequest> {
    (
        "[a-z.]{1,10}",
        proptest::option::of(0i64..2_000_000),
        proptest::option::of(0u32..2000),
    )
        .prop_map(|(tool_name, amount, recipient_count)| ToolCallRequest {
            tool_name,
            amount: amount.map(Cents),
            recipient_count,
            paths: Vec::new(),
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(3000))]

    /// Déterminisme : deux évaluations identiques donnent le même résultat.
    #[test]
    fn decide_is_deterministic(
        rules in proptest::collection::vec(rule_strategy(), 0..6),
        req in request_strategy(),
    ) {
        prop_assert_eq!(decide(&rules, &req), decide(&rules, &req));
    }

    /// Refus par défaut : sans AUCUNE règle pour l'outil demandé, l'issue est
    /// toujours un refus, quel que soit le reste des règles.
    #[test]
    fn no_matching_rule_always_denies(
        rules in proptest::collection::vec(rule_strategy(), 0..6),
        req in request_strategy(),
    ) {
        if !rules.iter().any(|r| r.tool_name == req.tool_name) {
            let denied = matches!(decide(&rules, &req).decision, Decision::Deny { .. });
            prop_assert!(denied);
        }
    }

    /// Toute issue porte une raison non vide.
    #[test]
    fn every_decision_has_a_reason(
        rules in proptest::collection::vec(rule_strategy(), 0..6),
        req in request_strategy(),
    ) {
        prop_assert!(!decide(&rules, &req).reason.trim().is_empty());
    }

    /// Monotonie : comme la première règle correspondante l'emporte, AJOUTER
    /// des règles APRÈS ne change jamais la décision d'un outil qui avait
    /// déjà une règle correspondante. (Un refus ne devient pas autorisation
    /// par simple ajout en fin de liste.)
    #[test]
    fn appending_rules_never_changes_an_existing_match(
        rules in proptest::collection::vec(rule_strategy(), 1..6),
        extra in proptest::collection::vec(rule_strategy(), 0..4),
        req in request_strategy(),
    ) {
        if rules.iter().any(|r| r.tool_name == req.tool_name) {
            let before = decide(&rules, &req);
            let mut extended = rules.clone();
            extended.extend(extra);
            prop_assert_eq!(before, decide(&extended, &req));
        }
    }

    /// Bloc 4 — la propriété du produit : AU-DELÀ de la limite dure d'un
    /// montant, l'issue est un REFUS, jamais une validation — quel que soit
    /// le drapeau `requires_approval` et quelle que soit la bande de
    /// validation. Aucun tampon du dirigeant ne fait partir l'appel.
    #[test]
    fn beyond_hard_limit_is_always_denied_never_approvable(
        bound in amount_bound_strategy(),
        requires_approval in any::<bool>(),
        excess in 1i64..1_000_000,
    ) {
        let rule = ToolRule {
            tool_name: "outil".to_owned(),
            allowed: true,
            requires_approval,
            amount: Some(bound),
            recipients: None,
            paths: None,
        };
        let req = ToolCallRequest {
            tool_name: "outil".to_owned(),
            amount: Some(Cents(bound.hard_limit().0 + excess)),
            recipient_count: None,
            paths: Vec::new(),
        };
        let denied = matches!(decide(&[rule], &req).decision, Decision::Deny { .. });
        prop_assert!(denied);
    }
}

// Un chemin restreint : proptest ciblé sur l'absence de fail-open.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Sous restriction de chemins, un chemin contenant `..` ou `\` est
    /// TOUJOURS refusé, quel que soit le mode de la borne.
    #[test]
    fn protocol_violations_always_deny(
        hard in any::<bool>(),
        evil in prop_oneof![
            Just("/data/../secret".to_owned()),
            Just("/data/..\\x".to_owned()),
            Just("a\\b".to_owned()),
        ],
    ) {
        let rule = ToolRule {
            tool_name: "doc.write".to_owned(),
            allowed: true,
            requires_approval: false,
            amount: None,
            recipients: None,
            paths: Some(if hard {
                PathBound::hard(vec!["/data".to_owned()])
            } else {
                PathBound::soft(vec!["/data".to_owned()])
            }),
        };
        let req = ToolCallRequest {
            tool_name: "doc.write".to_owned(),
            amount: None,
            recipient_count: None,
            paths: vec![evil],
        };
        let denied = matches!(decide(&[rule], &req).decision, Decision::Deny { .. });
        prop_assert!(denied);
    }
}
