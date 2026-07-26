//! Moteur de politiques — partie **pure** (invariant 2).
//!
//! Évaluation `(règles, appel demandé) -> décision`, sans aucune
//! entrée-sortie : les règles sont passées en paramètre, la lecture en base
//! arrive à un jalon ultérieur. Tout est vérifiable par `cargo test` seul.
//!
//! Sémantique, décidée et FIGÉE ici (choix consignés dans
//! `docs/questions-nuit.md`) :
//! - **Refus par défaut** : un outil sans règle explicite est refusé.
//! - **Au seuil exact = autorisé ; au-delà = violation.** La comparaison est
//!   strictement supérieure, comme pour le plafond de coût
//!   ([`kollega_core::CostCeiling`]) : atteindre un maximum est permis, le
//!   dépasser ne l'est jamais.
//! - **Dépasser un maximum = refus**, pas une demande de validation : la
//!   validation humaine est demandée par le drapeau `requires_approval` de la
//!   règle, jamais déclenchée par une violation de limite.
//! - **Fermé par défaut sur l'inconnu** : si une règle borne le montant, les
//!   destinataires ou les chemins et que l'appel ne déclare pas la valeur
//!   correspondante, l'appel est refusé. Un chemin contenant `\` (séparateur
//!   ambigu selon la cible) ou une traversée `..` est refusé d'office.
//! - Toute issue porte une raison lisible par un humain, jamais vide :
//!   c'est le futur `tool_calls.decision_reason`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use kollega_core::{Cents, Decision};

/// Description d'un appel d'outil soumis au moteur.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolCallRequest {
    /// Nom de l'outil demandé (ex. `doc.write`, `mail.send`).
    pub tool_name: String,
    /// Montant engagé par l'appel, s'il y en a un.
    pub amount: Option<Cents>,
    /// Nombre de destinataires, si l'appel en a.
    pub recipient_count: Option<u32>,
    /// Chemins touchés par l'appel, séparés par `/`. Vide si sans objet.
    pub paths: Vec<String>,
}

/// Règle de politique pour UN outil (future ligne de la table `policies`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRule {
    /// Nom de l'outil régi.
    pub tool_name: String,
    /// Faux = l'outil est interdit, quelles que soient les autres bornes.
    pub allowed: bool,
    /// Vrai = toute exécution autorisée passe d'abord par une validation
    /// humaine.
    pub requires_approval: bool,
    /// Montant maximal autorisé (inclus). `None` = pas de borne de montant.
    pub max_amount: Option<Cents>,
    /// Nombre maximal de destinataires (inclus). `None` = pas de borne.
    pub max_recipients: Option<u32>,
    /// Préfixes de chemins autorisés. `None` = pas de restriction de chemin ;
    /// `Some(vide)` = aucun chemin autorisé.
    pub allowed_path_prefixes: Option<Vec<String>>,
}

/// Issue de l'évaluation : la décision du domaine et sa raison.
///
/// `reason` n'est jamais vide — c'est ce qui alimente
/// `tool_calls.decision_reason`, y compris pour une autorisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    /// Décision du domaine (`allow` / `deny` / `require_approval`).
    pub decision: Decision,
    /// Raison lisible par un humain, jamais vide.
    pub reason: String,
}

impl Evaluation {
    fn deny(reason: String) -> Evaluation {
        Evaluation {
            decision: Decision::Deny {
                reason: reason.clone(),
            },
            reason,
        }
    }

    fn require_approval(reason: String) -> Evaluation {
        Evaluation {
            decision: Decision::RequireApproval {
                threshold: reason.clone(),
            },
            reason,
        }
    }

    fn allow(reason: String) -> Evaluation {
        Evaluation {
            decision: Decision::Allow,
            reason,
        }
    }
}

/// Vrai si `path` est couvert par `prefix`, au sens des segments :
/// exactement le préfixe, ou un descendant (`prefix/…`). `"/data"` ne couvre
/// pas `"/data-autre"`.
fn path_is_under(path: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() {
        // Un préfixe vide ou « / » couvre tout : à réserver aux règles
        // volontairement larges.
        return true;
    }
    path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Évalue un appel d'outil contre les règles d'une organisation.
///
/// Les règles sont celles de l'organisation courante (la lecture filtrée par
/// la RLS arrive à un jalon ultérieur). Si plusieurs règles portent le même
/// nom d'outil, la première l'emporte (la table `policies` garantit
/// l'unicité par organisation ; ici, en pur, on documente le choix).
#[must_use]
pub fn decide(rules: &[ToolRule], request: &ToolCallRequest) -> Evaluation {
    let tool = &request.tool_name;

    // Refus par défaut : pas de règle, pas d'exécution.
    let Some(rule) = rules.iter().find(|r| &r.tool_name == tool) else {
        return Evaluation::deny(format!(
            "aucune politique déclarée pour l'outil {tool} : refus par défaut"
        ));
    };

    if !rule.allowed {
        return Evaluation::deny(format!("l'outil {tool} est interdit par la politique"));
    }

    // Montant : borne inclusive ; valeur non déclarée = refus (fermé par
    // défaut sur l'inconnu).
    if let Some(max_amount) = rule.max_amount {
        match request.amount {
            None => {
                return Evaluation::deny(format!(
                    "l'outil {tool} est plafonné à {} centimes mais l'appel ne déclare aucun montant",
                    max_amount.0
                ));
            }
            Some(amount) if amount > max_amount => {
                return Evaluation::deny(format!(
                    "montant demandé {} centimes > plafond {} centimes pour l'outil {tool}",
                    amount.0, max_amount.0
                ));
            }
            Some(_) => {}
        }
    }

    // Destinataires : même logique.
    if let Some(max_recipients) = rule.max_recipients {
        match request.recipient_count {
            None => {
                return Evaluation::deny(format!(
                    "l'outil {tool} est limité à {max_recipients} destinataires mais l'appel n'en déclare aucun"
                ));
            }
            Some(count) if count > max_recipients => {
                return Evaluation::deny(format!(
                    "{count} destinataires demandés > maximum {max_recipients} pour l'outil {tool}"
                ));
            }
            Some(_) => {}
        }
    }

    // Chemins : chaque chemin touché doit être couvert par un préfixe
    // autorisé. Fermé par défaut sur l'inconnu, ici aussi : un outil
    // restreint par chemins qui n'en déclare aucun est refusé — sinon un
    // adaptateur qui omet la déclaration contournerait la restriction.
    if let Some(prefixes) = &rule.allowed_path_prefixes {
        if request.paths.is_empty() {
            return Evaluation::deny(format!(
                "l'outil {tool} est restreint par chemins mais l'appel n'en déclare aucun"
            ));
        }
        for path in &request.paths {
            // L'antislash est un séparateur réel sur certaines cibles
            // (Windows, SharePoint) : un chemin qui en contient est ambigu
            // et pourrait s'évader du dossier autorisé une fois interprété
            // là-bas. Refus, comme « .. ».
            if path.contains('\\') {
                return Evaluation::deny(format!(
                    "chemin refusé pour l'outil {tool} : séparateur « \\ » ambigu interdit, utiliser « / »"
                ));
            }
            if path.split('/').any(|segment| segment == "..") {
                return Evaluation::deny(format!(
                    "chemin refusé pour l'outil {tool} : traversée « .. » interdite"
                ));
            }
            if !prefixes.iter().any(|prefix| path_is_under(path, prefix)) {
                return Evaluation::deny(format!(
                    "chemin hors des dossiers autorisés pour l'outil {tool}"
                ));
            }
        }
    }

    if rule.requires_approval {
        return Evaluation::require_approval(format!(
            "la politique de l'outil {tool} exige une validation humaine avant exécution"
        ));
    }

    Evaluation::allow(format!(
        "appel de l'outil {tool} conforme à la politique déclarée"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(tool: &str) -> ToolRule {
        ToolRule {
            tool_name: tool.to_owned(),
            allowed: true,
            requires_approval: false,
            max_amount: None,
            max_recipients: None,
            allowed_path_prefixes: None,
        }
    }

    fn request(tool: &str) -> ToolCallRequest {
        ToolCallRequest {
            tool_name: tool.to_owned(),
            ..ToolCallRequest::default()
        }
    }

    fn assert_denied(evaluation: &Evaluation) {
        assert!(
            matches!(evaluation.decision, Decision::Deny { .. }),
            "attendu un refus, obtenu : {evaluation:?}"
        );
    }

    // -- Refus par défaut ---------------------------------------------------

    #[test]
    fn unknown_tool_is_denied_by_default() {
        let rules = [rule("doc.read")];
        let evaluation = decide(&rules, &request("mail.send"));
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains("refus par défaut"));
    }

    #[test]
    fn no_rules_at_all_denies_everything() {
        let evaluation = decide(&[], &request("doc.read"));
        assert_denied(&evaluation);
    }

    #[test]
    fn disallowed_tool_is_denied_even_within_limits() {
        let mut r = rule("doc.write");
        r.allowed = false;
        let evaluation = decide(&[r], &request("doc.write"));
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains("interdit"));
    }

    // -- Montant : sous / au seuil / au-delà / non déclaré ------------------

    #[test]
    fn amount_threshold_table() {
        let mut r = rule("paiement");
        r.max_amount = Some(Cents(10_000));
        let rules = [r];
        let cases = [
            (Some(Cents(9_999)), true),   // sous le seuil
            (Some(Cents(10_000)), true),  // AU seuil : autorisé (inclusif)
            (Some(Cents(10_001)), false), // au-delà : refusé
        ];
        for (amount, expected_allowed) in cases {
            let mut req = request("paiement");
            req.amount = amount;
            let evaluation = decide(&rules, &req);
            assert_eq!(
                matches!(evaluation.decision, Decision::Allow),
                expected_allowed,
                "montant {amount:?} : {evaluation:?}"
            );
        }
    }

    #[test]
    fn undeclared_amount_with_cap_is_denied() {
        let mut r = rule("paiement");
        r.max_amount = Some(Cents(10_000));
        let evaluation = decide(&[r], &request("paiement"));
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains("aucun montant"));
    }

    // -- Destinataires ------------------------------------------------------

    #[test]
    fn recipient_threshold_table() {
        let mut r = rule("mail.send");
        r.max_recipients = Some(5);
        let rules = [r];
        let cases = [(Some(4), true), (Some(5), true), (Some(6), false)];
        for (count, expected_allowed) in cases {
            let mut req = request("mail.send");
            req.recipient_count = count;
            let evaluation = decide(&rules, &req);
            assert_eq!(
                matches!(evaluation.decision, Decision::Allow),
                expected_allowed,
                "destinataires {count:?} : {evaluation:?}"
            );
        }
    }

    #[test]
    fn undeclared_recipients_with_cap_is_denied() {
        let mut r = rule("mail.send");
        r.max_recipients = Some(5);
        let evaluation = decide(&[r], &request("mail.send"));
        assert_denied(&evaluation);
    }

    // -- Chemins ------------------------------------------------------------

    #[test]
    fn path_prefix_table() {
        let mut r = rule("doc.write");
        r.allowed_path_prefixes = Some(vec!["/data/clients".to_owned()]);
        let rules = [r];
        let cases = [
            ("/data/clients", true),                // exactement le préfixe
            ("/data/clients/2026/devis.pdf", true), // descendant
            ("/data/clients-bis/x", false),         // faux frère : refusé
            ("/data", false),                       // parent : refusé
            ("/tmp/evasion", false),                // ailleurs : refusé
        ];
        for (path, expected_allowed) in cases {
            let mut req = request("doc.write");
            req.paths = vec![path.to_owned()];
            let evaluation = decide(&rules, &req);
            assert_eq!(
                matches!(evaluation.decision, Decision::Allow),
                expected_allowed,
                "chemin {path} : {evaluation:?}"
            );
        }
    }

    #[test]
    fn path_traversal_is_denied() {
        let mut r = rule("doc.write");
        r.allowed_path_prefixes = Some(vec!["/data".to_owned()]);
        let mut req = request("doc.write");
        req.paths = vec!["/data/../etc/passwd".to_owned()];
        let evaluation = decide(&[r], &req);
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains(".."));
    }

    #[test]
    fn backslash_traversal_is_denied() {
        // « /data/..\..\secret » : aucun segment séparé par '/' ne vaut
        // exactement « .. », mais sur une cible où '\' sépare, c'est une
        // évasion. L'antislash est donc refusé d'office.
        let mut r = rule("doc.write");
        r.allowed_path_prefixes = Some(vec!["/data".to_owned()]);
        let mut req = request("doc.write");
        req.paths = vec!["/data/..\\..\\secret".to_owned()];
        let evaluation = decide(&[r], &req);
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains('\\'));
    }

    #[test]
    fn undeclared_paths_with_restriction_are_denied() {
        // Fermé sur l'inconnu, comme pour le montant : une règle qui
        // restreint les chemins refuse un appel qui n'en déclare aucun.
        let mut r = rule("doc.write");
        r.allowed_path_prefixes = Some(vec!["/data".to_owned()]);
        let evaluation = decide(&[r], &request("doc.write"));
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains("aucun"));
    }

    #[test]
    fn empty_prefix_list_denies_all_paths() {
        let mut r = rule("doc.write");
        r.allowed_path_prefixes = Some(vec![]);
        let mut req = request("doc.write");
        req.paths = vec!["/nimporte/ou".to_owned()];
        assert_denied(&decide(&[r], &req));
    }

    #[test]
    fn one_bad_path_among_good_ones_denies() {
        let mut r = rule("doc.write");
        r.allowed_path_prefixes = Some(vec!["/data".to_owned()]);
        let mut req = request("doc.write");
        req.paths = vec!["/data/ok.txt".to_owned(), "/etc/interdit".to_owned()];
        assert_denied(&decide(&[r], &req));
    }

    #[test]
    fn no_path_restriction_allows_any_path() {
        let r = rule("doc.read");
        let mut req = request("doc.read");
        req.paths = vec!["/ou/on/veut".to_owned()];
        assert!(matches!(decide(&[r], &req).decision, Decision::Allow));
    }

    // -- Validation humaine -------------------------------------------------

    #[test]
    fn requires_approval_after_limits_pass() {
        let mut r = rule("mail.send");
        r.requires_approval = true;
        r.max_recipients = Some(5);
        let mut req = request("mail.send");
        req.recipient_count = Some(3);
        let evaluation = decide(&[r], &req);
        assert!(matches!(
            evaluation.decision,
            Decision::RequireApproval { .. }
        ));
        assert!(evaluation.reason.contains("validation humaine"));
    }

    #[test]
    fn limit_violation_wins_over_approval_flag() {
        // Une violation de limite est un refus, jamais une simple demande de
        // validation : le drapeau requires_approval ne « rattrape » pas un
        // dépassement.
        let mut r = rule("mail.send");
        r.requires_approval = true;
        r.max_recipients = Some(5);
        let mut req = request("mail.send");
        req.recipient_count = Some(50);
        assert_denied(&decide(&[r], &req));
    }

    // -- Raisons ------------------------------------------------------------

    #[test]
    fn every_outcome_carries_a_nonempty_reason() {
        let mut denied_rule = rule("a");
        denied_rule.allowed = false;
        let mut approval_rule = rule("b");
        approval_rule.requires_approval = true;
        let rules = [denied_rule, approval_rule, rule("c")];
        for tool in ["a", "b", "c", "outil-sans-regle"] {
            let evaluation = decide(&rules, &request(tool));
            assert!(
                !evaluation.reason.trim().is_empty(),
                "raison vide pour {tool}"
            );
            // La raison portée par la variante du domaine est la même.
            match &evaluation.decision {
                Decision::Allow => {}
                Decision::Deny { reason } => assert_eq!(reason, &evaluation.reason),
                Decision::RequireApproval { threshold } => {
                    assert_eq!(threshold, &evaluation.reason);
                }
            }
        }
    }

    #[test]
    fn first_matching_rule_wins() {
        let mut first = rule("doc.write");
        first.allowed = false;
        let second = rule("doc.write");
        let evaluation = decide(&[first, second], &request("doc.write"));
        assert_denied(&evaluation);
    }
}
