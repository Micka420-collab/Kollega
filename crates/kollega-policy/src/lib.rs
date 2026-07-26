//! Moteur de politiques — partie **pure** (invariant 2).
//!
//! Évaluation `(règles, appel demandé) -> décision`, sans aucune
//! entrée-sortie : les règles sont passées en paramètre, la lecture en base
//! arrive à un jalon ultérieur. Tout est vérifiable par `cargo test` seul.
//!
//! Sémantique, décidée et FIGÉE ici :
//! - **Refus par défaut** : un outil sans règle explicite est refusé.
//! - **Chaque borne porte son mode**, explicitement ([`Bound`],
//!   [`PathBound`]) : une **limite dure** (`ExceedMode::Deny`) ne doit
//!   jamais être franchie, aucune validation ne l'autorise → refus ; un
//!   **seuil souple** (`ExceedMode::RequireApproval`) marque l'inhabituel
//!   qui mérite un humain → validation requise. C'est la promesse du
//!   produit : un dépassement de seuil est porté devant le dirigeant, pas
//!   tué en silence.
//! - **Au seuil exact = autorisé ; au-delà = le mode de la borne joue.** La
//!   comparaison est strictement supérieure, comme pour le plafond de coût.
//! - **Une limite dure l'emporte toujours** : si un appel viole à la fois
//!   une borne dure et une borne souple, l'issue est le refus.
//! - **Fermé par défaut sur l'inconnu, en dur** : valeur non déclarée sous
//!   une borne (montant, destinataires, chemins), chemin contenant `\`
//!   (séparateur ambigu selon la cible) ou une traversée `..` → refus,
//!   quel que soit le mode de la borne — ce sont des violations de
//!   protocole, pas des exceptions métier qu'un humain peut arbitrer.
//! - Toute issue porte une raison lisible qui dit QUEL mode a joué, jamais
//!   vide : c'est le futur `tool_calls.decision_reason`.
//!
//! Défauts recommandés par type de borne (documentés, surchargeables champ
//! par champ) : montant → souple, destinataires → souple, chemins → dur.

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

/// Ce qui se produit quand une borne est franchie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceedMode {
    /// Limite dure : ne doit jamais arriver, aucune validation ne
    /// l'autorise. Franchie → refus.
    Deny,
    /// Seuil souple : inhabituel, mérite un humain. Franchi → validation
    /// requise.
    RequireApproval,
}

/// Borne scalaire (montant, destinataires) avec son mode, explicite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bound<T> {
    /// Valeur maximale autorisée (incluse).
    pub max: T,
    /// Mode au franchissement — pas de défaut implicite.
    pub on_exceed: ExceedMode,
}

impl<T> Bound<T> {
    /// Limite dure : franchie → refus.
    pub const fn hard(max: T) -> Self {
        Bound {
            max,
            on_exceed: ExceedMode::Deny,
        }
    }

    /// Seuil souple : franchi → validation humaine.
    pub const fn soft(max: T) -> Self {
        Bound {
            max,
            on_exceed: ExceedMode::RequireApproval,
        }
    }
}

/// Restriction de chemins avec son mode, explicite.
///
/// `allowed_prefixes` vide = aucun chemin autorisé. Un chemin est couvert
/// s'il égale un préfixe ou en descend (`prefixe/…`) — `/data` ne couvre pas
/// `/data-autre`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathBound {
    /// Préfixes de chemins autorisés, au sens des segments.
    pub allowed_prefixes: Vec<String>,
    /// Mode quand un chemin sort des préfixes. Les violations de protocole
    /// (`\`, `..`, chemins non déclarés) restent des refus quel que soit ce
    /// mode.
    pub on_exceed: ExceedMode,
}

impl PathBound {
    /// Restriction dure (défaut recommandé pour les chemins) : hors
    /// dossier → refus.
    #[must_use]
    pub fn hard(allowed_prefixes: Vec<String>) -> Self {
        PathBound {
            allowed_prefixes,
            on_exceed: ExceedMode::Deny,
        }
    }

    /// Restriction souple : hors dossier → validation humaine.
    #[must_use]
    pub fn soft(allowed_prefixes: Vec<String>) -> Self {
        PathBound {
            allowed_prefixes,
            on_exceed: ExceedMode::RequireApproval,
        }
    }
}

/// Règle de politique pour UN outil (future ligne de la table `policies`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRule {
    /// Nom de l'outil régi.
    pub tool_name: String,
    /// Faux = l'outil est interdit, quelles que soient les autres bornes.
    pub allowed: bool,
    /// Vrai = toute exécution DANS les bornes passe quand même par une
    /// validation humaine.
    pub requires_approval: bool,
    /// Borne de montant. `None` = pas de borne. Défaut recommandé : souple.
    pub amount: Option<Bound<Cents>>,
    /// Borne de destinataires. `None` = pas de borne. Défaut recommandé :
    /// souple.
    pub recipients: Option<Bound<u32>>,
    /// Restriction de chemins. `None` = pas de restriction. Défaut
    /// recommandé : dure.
    pub paths: Option<PathBound>,
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

/// Vrai si `path` est couvert par `prefix`, au sens des segments.
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

/// Violation constatée pendant l'évaluation, avec le mode de sa borne.
enum Violation {
    Hard(String),
    Soft(String),
}

/// Évalue un appel d'outil contre les règles d'une organisation.
///
/// Ordre : règle trouvée (sinon refus par défaut) → outil autorisé →
/// bornes (montant, destinataires, chemins). Une violation de borne dure ou
/// de protocole refuse immédiatement ; les violations souples s'accumulent
/// et produisent une validation requise ; sinon, le drapeau
/// `requires_approval` de la règle, puis l'autorisation.
///
/// Si plusieurs règles portent le même nom d'outil, la première l'emporte
/// (la table `policies` garantit l'unicité par organisation).
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

    let mut soft_reasons: Vec<String> = Vec::new();
    let mut record = |violation: Violation| -> Option<Evaluation> {
        match violation {
            Violation::Hard(reason) => Some(Evaluation::deny(reason)),
            Violation::Soft(reason) => {
                soft_reasons.push(reason);
                None
            }
        }
    };

    // Montant. Valeur non déclarée sous une borne : refus, quel que soit le
    // mode — violation de protocole, pas exception métier.
    if let Some(bound) = &rule.amount {
        match request.amount {
            None => {
                return Evaluation::deny(format!(
                    "l'outil {tool} est borné en montant ({} centimes) mais l'appel ne déclare aucun montant",
                    bound.max.0
                ));
            }
            Some(amount) if amount > bound.max => {
                let exceeded = match bound.on_exceed {
                    ExceedMode::Deny => Violation::Hard(format!(
                        "limite dure dépassée : montant {} centimes > {} centimes pour l'outil {tool} — refus",
                        amount.0, bound.max.0
                    )),
                    ExceedMode::RequireApproval => Violation::Soft(format!(
                        "seuil souple franchi : montant {} centimes > {} centimes pour l'outil {tool} — validation requise",
                        amount.0, bound.max.0
                    )),
                };
                if let Some(denied) = record(exceeded) {
                    return denied;
                }
            }
            Some(_) => {}
        }
    }

    // Destinataires : même logique.
    if let Some(bound) = &rule.recipients {
        match request.recipient_count {
            None => {
                return Evaluation::deny(format!(
                    "l'outil {tool} est borné à {} destinataires mais l'appel n'en déclare aucun",
                    bound.max
                ));
            }
            Some(count) if count > bound.max => {
                let exceeded = match bound.on_exceed {
                    ExceedMode::Deny => Violation::Hard(format!(
                        "limite dure dépassée : {count} destinataires > {} pour l'outil {tool} — refus",
                        bound.max
                    )),
                    ExceedMode::RequireApproval => Violation::Soft(format!(
                        "seuil souple franchi : {count} destinataires > {} pour l'outil {tool} — validation requise",
                        bound.max
                    )),
                };
                if let Some(denied) = record(exceeded) {
                    return denied;
                }
            }
            Some(_) => {}
        }
    }

    // Chemins. Les violations de protocole (aucun chemin déclaré, `\`,
    // `..`) sont des refus quel que soit le mode de la borne.
    if let Some(path_bound) = &rule.paths {
        if request.paths.is_empty() {
            return Evaluation::deny(format!(
                "l'outil {tool} est restreint par chemins mais l'appel n'en déclare aucun"
            ));
        }
        for path in &request.paths {
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
            if !path_bound
                .allowed_prefixes
                .iter()
                .any(|prefix| path_is_under(path, prefix))
            {
                let outside = match path_bound.on_exceed {
                    ExceedMode::Deny => Violation::Hard(format!(
                        "limite dure : chemin hors des dossiers autorisés pour l'outil {tool} — refus"
                    )),
                    ExceedMode::RequireApproval => Violation::Soft(format!(
                        "seuil souple : chemin hors des dossiers autorisés pour l'outil {tool} — validation requise"
                    )),
                };
                if let Some(denied) = record(outside) {
                    return denied;
                }
            }
        }
    }

    if !soft_reasons.is_empty() {
        return Evaluation::require_approval(soft_reasons.join(" ; "));
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
            amount: None,
            recipients: None,
            paths: None,
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

    fn assert_approval(evaluation: &Evaluation) {
        assert!(
            matches!(evaluation.decision, Decision::RequireApproval { .. }),
            "attendu une validation requise, obtenu : {evaluation:?}"
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
        assert_denied(&decide(&[], &request("doc.read")));
    }

    #[test]
    fn disallowed_tool_is_denied_even_within_limits() {
        let mut r = rule("doc.write");
        r.allowed = false;
        let evaluation = decide(&[r], &request("doc.write"));
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains("interdit"));
    }

    // -- Montant : chaque mode, sous / au / au-delà -------------------------

    #[test]
    fn amount_bound_table_both_modes() {
        for (mode, above_is_denied) in [
            (ExceedMode::Deny, true),
            (ExceedMode::RequireApproval, false),
        ] {
            let mut r = rule("paiement");
            r.amount = Some(Bound {
                max: Cents(10_000),
                on_exceed: mode,
            });
            let rules = [r];
            // Sous et AU seuil : autorisé dans les deux modes.
            for amount in [Cents(9_999), Cents(10_000)] {
                let mut req = request("paiement");
                req.amount = Some(amount);
                let evaluation = decide(&rules, &req);
                assert!(
                    matches!(evaluation.decision, Decision::Allow),
                    "mode {mode:?}, montant {amount:?} : {evaluation:?}"
                );
            }
            // Au-delà : le mode de la borne joue.
            let mut req = request("paiement");
            req.amount = Some(Cents(10_001));
            let evaluation = decide(&rules, &req);
            if above_is_denied {
                assert_denied(&evaluation);
                assert!(evaluation.reason.contains("limite dure"));
            } else {
                assert_approval(&evaluation);
                assert!(evaluation.reason.contains("seuil souple"));
            }
        }
    }

    #[test]
    fn undeclared_amount_is_denied_even_with_soft_bound() {
        // Violation de protocole : refus, même si la borne est souple.
        let mut r = rule("paiement");
        r.amount = Some(Bound::soft(Cents(10_000)));
        let evaluation = decide(&[r], &request("paiement"));
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains("aucun montant"));
    }

    // -- Destinataires ------------------------------------------------------

    #[test]
    fn recipient_bound_table_both_modes() {
        for (mode, above_is_denied) in [
            (ExceedMode::Deny, true),
            (ExceedMode::RequireApproval, false),
        ] {
            let mut r = rule("mail.send");
            r.recipients = Some(Bound {
                max: 5,
                on_exceed: mode,
            });
            let rules = [r];
            for count in [4, 5] {
                let mut req = request("mail.send");
                req.recipient_count = Some(count);
                assert!(
                    matches!(decide(&rules, &req).decision, Decision::Allow),
                    "mode {mode:?}, {count} destinataires"
                );
            }
            let mut req = request("mail.send");
            req.recipient_count = Some(6);
            let evaluation = decide(&rules, &req);
            if above_is_denied {
                assert_denied(&evaluation);
            } else {
                assert_approval(&evaluation);
            }
        }
    }

    #[test]
    fn undeclared_recipients_are_denied_even_with_soft_bound() {
        let mut r = rule("mail.send");
        r.recipients = Some(Bound::soft(5));
        assert_denied(&decide(&[r], &request("mail.send")));
    }

    // -- Chemins ------------------------------------------------------------

    #[test]
    fn path_bound_table_both_modes() {
        for (mode, outside_is_denied) in [
            (ExceedMode::Deny, true),
            (ExceedMode::RequireApproval, false),
        ] {
            let mut r = rule("doc.write");
            r.paths = Some(PathBound {
                allowed_prefixes: vec!["/data/clients".to_owned()],
                on_exceed: mode,
            });
            let rules = [r];
            // Couverts : exactement le préfixe, et un descendant.
            for path in ["/data/clients", "/data/clients/2026/devis.pdf"] {
                let mut req = request("doc.write");
                req.paths = vec![path.to_owned()];
                assert!(
                    matches!(decide(&rules, &req).decision, Decision::Allow),
                    "mode {mode:?}, chemin {path}"
                );
            }
            // Hors dossier : le mode joue ; le faux frère /data/clients-bis
            // et le parent /data sont bien « hors ».
            for path in ["/data/clients-bis/x", "/data", "/tmp/evasion"] {
                let mut req = request("doc.write");
                req.paths = vec![path.to_owned()];
                let evaluation = decide(&rules, &req);
                if outside_is_denied {
                    assert_denied(&evaluation);
                } else {
                    assert_approval(&evaluation);
                }
            }
        }
    }

    #[test]
    fn protocol_violations_deny_even_with_soft_path_bound() {
        // `\`, `..` et l'absence de chemins déclarés restent des refus,
        // même quand la borne de chemins est souple.
        let mut r = rule("doc.write");
        r.paths = Some(PathBound::soft(vec!["/data".to_owned()]));
        let rules = [r];

        let mut req = request("doc.write");
        req.paths = vec!["/data/..\\..\\secret".to_owned()];
        let evaluation = decide(&rules, &req);
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains('\\'));

        let mut req = request("doc.write");
        req.paths = vec!["/data/../etc/passwd".to_owned()];
        let evaluation = decide(&rules, &req);
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains(".."));

        let evaluation = decide(&rules, &request("doc.write"));
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains("aucun"));
    }

    #[test]
    fn empty_prefix_list_denies_all_paths() {
        let mut r = rule("doc.write");
        r.paths = Some(PathBound::hard(vec![]));
        let mut req = request("doc.write");
        req.paths = vec!["/nimporte/ou".to_owned()];
        assert_denied(&decide(&[r], &req));
    }

    #[test]
    fn one_bad_path_among_good_ones_applies_the_mode() {
        let mut r = rule("doc.write");
        r.paths = Some(PathBound::hard(vec!["/data".to_owned()]));
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

    // -- Combinaisons de modes ----------------------------------------------

    #[test]
    fn hard_violation_wins_over_soft_violation() {
        // Montant souple franchi ET destinataires durs dépassés : refus.
        let mut r = rule("mail.send");
        r.amount = Some(Bound::soft(Cents(100)));
        r.recipients = Some(Bound::hard(5));
        let mut req = request("mail.send");
        req.amount = Some(Cents(200));
        req.recipient_count = Some(50);
        let evaluation = decide(&[r], &req);
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains("limite dure"));
    }

    #[test]
    fn multiple_soft_violations_merge_into_one_approval() {
        let mut r = rule("mail.send");
        r.amount = Some(Bound::soft(Cents(100)));
        r.recipients = Some(Bound::soft(5));
        let mut req = request("mail.send");
        req.amount = Some(Cents(200));
        req.recipient_count = Some(8);
        let evaluation = decide(&[r], &req);
        assert_approval(&evaluation);
        assert!(evaluation.reason.contains("montant"));
        assert!(evaluation.reason.contains("destinataires"));
    }

    // -- Validation humaine par drapeau -------------------------------------

    #[test]
    fn requires_approval_after_bounds_pass() {
        let mut r = rule("mail.send");
        r.requires_approval = true;
        r.recipients = Some(Bound::soft(5));
        let mut req = request("mail.send");
        req.recipient_count = Some(3);
        let evaluation = decide(&[r], &req);
        assert_approval(&evaluation);
        assert!(evaluation.reason.contains("validation humaine"));
    }

    #[test]
    fn hard_limit_wins_over_approval_flag() {
        let mut r = rule("mail.send");
        r.requires_approval = true;
        r.recipients = Some(Bound::hard(5));
        let mut req = request("mail.send");
        req.recipient_count = Some(50);
        assert_denied(&decide(&[r], &req));
    }

    // -- Raisons ------------------------------------------------------------

    #[test]
    fn every_outcome_carries_a_nonempty_matching_reason() {
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
    fn reasons_name_the_mode_that_played() {
        let mut hard = rule("dur");
        hard.amount = Some(Bound::hard(Cents(10)));
        let mut soft = rule("souple");
        soft.amount = Some(Bound::soft(Cents(10)));
        let rules = [hard, soft];

        let mut req = request("dur");
        req.amount = Some(Cents(11));
        assert!(decide(&rules, &req).reason.contains("limite dure"));

        let mut req = request("souple");
        req.amount = Some(Cents(11));
        assert!(decide(&rules, &req).reason.contains("seuil souple"));
    }

    #[test]
    fn first_matching_rule_wins() {
        let mut first = rule("doc.write");
        first.allowed = false;
        let second = rule("doc.write");
        assert_denied(&decide(&[first, second], &request("doc.write")));
    }
}
