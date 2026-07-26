//! Moteur de politiques — partie **pure** (invariant 2).
//!
//! Évaluation `(règles, appel demandé) -> décision`, sans aucune
//! entrée-sortie : les règles sont passées en paramètre, la lecture en base
//! arrive à un jalon ultérieur. Tout est vérifiable par `cargo test` seul.
//!
//! Sémantique, décidée et FIGÉE ici :
//! - **Refus par défaut** : un outil sans règle explicite est refusé.
//! - **Chaque borne scalaire porte DEUX niveaux** ([`Bound`], bloc 4) : un
//!   **seuil de validation** et une **limite dure** au-dessus. En dessous ou
//!   au seuil : conforme. Entre les deux : validation humaine. Au-delà de la
//!   limite : refus, **quelle que soit la validation** — un agent qui veut
//!   écrire à 500 destinataires au lieu de 10 ne part plus en validation
//!   qu'un dirigeant pressé tamponnerait : il est refusé. Le « souple sans
//!   plafond » n'est plus constructible ; la limite dure est toujours là.
//! - **Aux bornes exactes = le niveau inférieur** : au seuil exact →
//!   conforme ; à la limite exacte → validation (ou conforme sans seuil).
//!   La comparaison est strictement supérieure, comme le plafond de coût.
//! - **Les chemins restent à UN niveau** ([`PathBound`], mode explicite) :
//!   un chemin est dedans ou dehors, il n'y a pas de « à quel point
//!   dehors » — donc pas de deux-étages possible sans inventer un ordre.
//! - **Une limite dure l'emporte toujours** : si un appel franchit à la fois
//!   une limite dure et un seuil de validation, l'issue est le refus.
//! - **Fermé par défaut sur l'inconnu, en dur** : valeur non déclarée sous
//!   une borne (montant, destinataires, chemins), chemin contenant `\`
//!   (séparateur ambigu selon la cible), traversée `..`, ou règle de
//!   chemins portant un préfixe VIDE (`""`/`"/"` — l'ex-« couvre tout »,
//!   un fail-open qu'une ligne de table mal remplie déclenchait) → refus —
//!   ce sont des violations de protocole, pas des exceptions métier.
//!   L'accès universel légitime s'exprime en ne posant PAS de `PathBound`.
//! - Toute issue porte une raison lisible qui dit QUEL niveau a joué,
//!   jamais vide : c'est le futur `tool_calls.decision_reason`.
//!
//! Défauts recommandés par type de borne (documentés, surchargeables champ
//! par champ) : montant et destinataires → deux étages
//! ([`Bound::two_tier`]) ; chemins → dur.

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

/// Ce qui se produit quand une restriction de chemins est franchie.
///
/// Ne s'applique QU'AUX chemins : les bornes scalaires portent leurs deux
/// niveaux dans [`Bound`] directement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceedMode {
    /// Limite dure : ne doit jamais arriver, aucune validation ne
    /// l'autorise. Franchie → refus.
    Deny,
    /// Seuil souple : inhabituel, mérite un humain. Franchi → validation
    /// requise.
    RequireApproval,
}

/// Erreur de construction d'une borne à deux étages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BoundError {
    /// Le seuil de validation dépasse la limite dure : la bande de
    /// validation serait négative, la règle ne veut rien dire.
    #[error("seuil de validation au-dessus de la limite dure")]
    ThresholdAboveHardLimit,
}

/// Borne scalaire à DEUX niveaux (montant, destinataires) — bloc 4.
///
/// Un seuil de validation, et une limite dure au-dessus. En dessous ou au
/// seuil : conforme. Entre les deux : validation humaine. Au-delà de la
/// limite : refus, quelle que soit la validation. Champs privés : une borne
/// dont le seuil dépasserait la limite n'est pas représentable, et une
/// borne SANS limite dure ne l'est pas non plus — le « souple sans
/// plafond » (validation à l'infini, que le dirigeant tamponne) était le
/// mode de défaillance que cette structure ferme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bound<T> {
    /// Seuil de validation (inclus). `None` = pas de bande de validation.
    approval_threshold: Option<T>,
    /// Limite dure (incluse) : au-delà, refus. Toujours présente.
    hard_limit: T,
}

/// Niveau atteint par une valeur face à une borne à deux étages.
enum Tier {
    Within,
    NeedsApproval,
    BeyondHardLimit,
}

impl<T: PartialOrd + Copy> Bound<T> {
    /// Limite dure seule : conforme jusqu'à la limite (incluse), refus
    /// au-delà — pas de bande de validation.
    pub const fn hard(hard_limit: T) -> Self {
        Bound {
            approval_threshold: None,
            hard_limit,
        }
    }

    /// Deux étages : conforme jusqu'au seuil (inclus), validation humaine
    /// du seuil (exclu) à la limite (incluse), refus au-delà. Exige
    /// `approval_threshold <= hard_limit`. Un seuil égal à la limite donne
    /// une bande vide — équivalent à [`Bound::hard`], accepté.
    pub fn two_tier(approval_threshold: T, hard_limit: T) -> Result<Self, BoundError> {
        if approval_threshold > hard_limit {
            return Err(BoundError::ThresholdAboveHardLimit);
        }
        Ok(Bound {
            approval_threshold: Some(approval_threshold),
            hard_limit,
        })
    }

    /// La limite dure (incluse).
    pub const fn hard_limit(&self) -> T {
        self.hard_limit
    }

    /// Le seuil de validation (inclus), s'il y a une bande de validation.
    pub const fn approval_threshold(&self) -> Option<T> {
        self.approval_threshold
    }

    /// Niveau atteint par `value`.
    fn tier(&self, value: T) -> Tier {
        if value > self.hard_limit {
            Tier::BeyondHardLimit
        } else if self
            .approval_threshold
            .is_some_and(|threshold| value > threshold)
        {
            Tier::NeedsApproval
        } else {
            Tier::Within
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
    /// Borne de montant. `None` = pas de borne. Défaut recommandé : deux
    /// étages ([`Bound::two_tier`]).
    pub amount: Option<Bound<Cents>>,
    /// Borne de destinataires. `None` = pas de borne. Défaut recommandé :
    /// deux étages ([`Bound::two_tier`]).
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
///
/// Un préfixe vide ne couvre RIEN : l'ex-sémantique « vide couvre tout »
/// était l'unique fail-open du moteur (une chaîne vide issue d'un
/// formulaire ou d'une ligne de table mal remplie transformait une
/// restriction dure en autorisation totale). Une règle qui porte un
/// préfixe vide est refusée en amont comme violation de protocole ; ici,
/// défense en profondeur : il ne couvre rien.
fn path_is_under(path: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() {
        return false;
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

    // Montant. Valeur non déclarée sous une borne : refus — violation de
    // protocole, pas exception métier.
    if let Some(bound) = &rule.amount {
        match request.amount {
            None => {
                return Evaluation::deny(format!(
                    "l'outil {tool} est borné en montant ({} centimes de limite dure) mais l'appel ne déclare aucun montant",
                    bound.hard_limit().0
                ));
            }
            Some(amount) => {
                let violation = match bound.tier(amount) {
                    Tier::Within => None,
                    Tier::BeyondHardLimit => Some(Violation::Hard(format!(
                        "limite dure dépassée : montant {} centimes > {} centimes pour l'outil {tool} — refus, aucune validation ne l'autorise",
                        amount.0,
                        bound.hard_limit().0
                    ))),
                    Tier::NeedsApproval => Some(Violation::Soft(format!(
                        "seuil de validation franchi : montant {} centimes > {} centimes (limite dure à {} centimes) pour l'outil {tool} — validation requise",
                        amount.0,
                        bound
                            .approval_threshold()
                            .map(|threshold| threshold.0)
                            .unwrap_or(bound.hard_limit().0),
                        bound.hard_limit().0
                    ))),
                };
                if let Some(denied) = violation.and_then(&mut record) {
                    return denied;
                }
            }
        }
    }

    // Destinataires : même logique.
    if let Some(bound) = &rule.recipients {
        match request.recipient_count {
            None => {
                return Evaluation::deny(format!(
                    "l'outil {tool} est borné à {} destinataires (limite dure) mais l'appel n'en déclare aucun",
                    bound.hard_limit()
                ));
            }
            Some(count) => {
                let violation = match bound.tier(count) {
                    Tier::Within => None,
                    Tier::BeyondHardLimit => Some(Violation::Hard(format!(
                        "limite dure dépassée : {count} destinataires > {} pour l'outil {tool} — refus, aucune validation ne l'autorise",
                        bound.hard_limit()
                    ))),
                    Tier::NeedsApproval => Some(Violation::Soft(format!(
                        "seuil de validation franchi : {count} destinataires > {} (limite dure à {}) pour l'outil {tool} — validation requise",
                        bound.approval_threshold().unwrap_or(bound.hard_limit()),
                        bound.hard_limit()
                    ))),
                };
                if let Some(denied) = violation.and_then(&mut record) {
                    return denied;
                }
            }
        }
    }

    // Chemins. Les violations de protocole (aucun chemin déclaré, `\`,
    // `..`, règle au préfixe vide) sont des refus quel que soit le mode de
    // la borne.
    if let Some(path_bound) = &rule.paths {
        if path_bound
            .allowed_prefixes
            .iter()
            .any(|prefix| prefix.trim_end_matches('/').is_empty())
        {
            // Fermé par défaut sur la règle malformée : un préfixe vide
            // (`""`, `"/"`) autorisait TOUT — l'accès universel légitime
            // s'exprime en ne posant pas de restriction de chemins du tout.
            return Evaluation::deny(format!(
                "règle de chemins invalide pour l'outil {tool} : préfixe vide interdit (l'accès universel se déclare en omettant la restriction)"
            ));
        }
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

    // -- Montant : les trois zones, et la limite dure seule ------------------

    #[test]
    fn amount_two_tier_has_three_zones() {
        let mut r = rule("paiement");
        r.amount = Some(Bound::two_tier(Cents(10_000), Cents(50_000)).unwrap());
        let rules = [r];
        // Sous et AU seuil : conforme.
        for amount in [Cents(9_999), Cents(10_000)] {
            let mut req = request("paiement");
            req.amount = Some(amount);
            assert!(
                matches!(decide(&rules, &req).decision, Decision::Allow),
                "montant {amount:?}"
            );
        }
        // Entre seuil et limite (limite incluse) : validation humaine.
        for amount in [Cents(10_001), Cents(50_000)] {
            let mut req = request("paiement");
            req.amount = Some(amount);
            let evaluation = decide(&rules, &req);
            assert_approval(&evaluation);
            assert!(
                evaluation.reason.contains("seuil de validation"),
                "la raison doit nommer le niveau : {evaluation:?}"
            );
        }
        // Au-delà de la limite : refus, quelle que soit la validation.
        let mut req = request("paiement");
        req.amount = Some(Cents(50_001));
        let evaluation = decide(&rules, &req);
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains("limite dure"));
        assert!(evaluation.reason.contains("aucune validation"));
    }

    #[test]
    fn amount_hard_only_has_two_zones() {
        let mut r = rule("paiement");
        r.amount = Some(Bound::hard(Cents(10_000)));
        let rules = [r];
        let mut req = request("paiement");
        req.amount = Some(Cents(10_000));
        assert!(matches!(decide(&rules, &req).decision, Decision::Allow));
        let mut req = request("paiement");
        req.amount = Some(Cents(10_001));
        assert_denied(&decide(&rules, &req));
    }

    #[test]
    fn two_tier_rejects_threshold_above_hard_limit() {
        assert_eq!(
            Bound::two_tier(Cents(11), Cents(10)),
            Err(BoundError::ThresholdAboveHardLimit)
        );
        // Seuil == limite : bande vide, accepté — équivalent au dur.
        let degenerate = Bound::two_tier(5u32, 5u32).unwrap();
        assert_eq!(degenerate.approval_threshold(), Some(5));
        assert_eq!(degenerate.hard_limit(), 5);
    }

    #[test]
    fn undeclared_amount_is_denied_even_with_approval_band() {
        // Violation de protocole : refus, même avec bande de validation.
        let mut r = rule("paiement");
        r.amount = Some(Bound::two_tier(Cents(10_000), Cents(50_000)).unwrap());
        let evaluation = decide(&[r], &request("paiement"));
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains("aucun montant"));
    }

    // -- Destinataires : le cas du brief (10 en libre, 500 en dur) -----------

    #[test]
    fn recipient_two_tier_stops_the_500_mail_blast() {
        // « un agent voulant écrire à 500 destinataires au lieu de 10 part
        // en validation ; si le dirigeant tamponne sans lire, les 500 mails
        // partent » — plus maintenant : au-dessus de la limite dure, refus,
        // aucune validation ne l'autorise.
        let mut r = rule("mail.send");
        r.recipients = Some(Bound::two_tier(10u32, 100).unwrap());
        let rules = [r];
        for count in [9, 10] {
            let mut req = request("mail.send");
            req.recipient_count = Some(count);
            assert!(
                matches!(decide(&rules, &req).decision, Decision::Allow),
                "{count} destinataires"
            );
        }
        for count in [11, 100] {
            let mut req = request("mail.send");
            req.recipient_count = Some(count);
            assert_approval(&decide(&rules, &req));
        }
        let mut req = request("mail.send");
        req.recipient_count = Some(500);
        let evaluation = decide(&rules, &req);
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains("limite dure"));
    }

    #[test]
    fn undeclared_recipients_are_denied_even_with_approval_band() {
        let mut r = rule("mail.send");
        r.recipients = Some(Bound::two_tier(5u32, 50).unwrap());
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
    fn empty_prefix_in_list_is_a_protocol_violation_not_a_wildcard() {
        // L'ex-fail-open : un préfixe vide (ou « / », ou « // ») couvrait
        // TOUT — une ligne de table mal remplie transformait une
        // restriction dure en autorisation totale. Désormais : refus de la
        // règle malformée, quel que soit le chemin demandé et le mode.
        for bad_prefix in ["", "/", "//"] {
            for path_bound in [
                PathBound::hard(vec![bad_prefix.to_owned()]),
                PathBound::soft(vec![bad_prefix.to_owned()]),
                PathBound::hard(vec!["/data".to_owned(), bad_prefix.to_owned()]),
            ] {
                let mut r = rule("doc.write");
                r.paths = Some(path_bound);
                let mut req = request("doc.write");
                req.paths = vec!["/data/ok.txt".to_owned()];
                let evaluation = decide(&[r], &req);
                assert_denied(&evaluation);
                assert!(
                    evaluation.reason.contains("préfixe vide"),
                    "préfixe {bad_prefix:?} : {evaluation:?}"
                );
            }
        }
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
    fn hard_violation_wins_over_approval_band() {
        // Bande de validation du montant franchie ET limite dure des
        // destinataires dépassée : refus.
        let mut r = rule("mail.send");
        r.amount = Some(Bound::two_tier(Cents(100), Cents(1_000)).unwrap());
        r.recipients = Some(Bound::hard(5));
        let mut req = request("mail.send");
        req.amount = Some(Cents(200));
        req.recipient_count = Some(50);
        let evaluation = decide(&[r], &req);
        assert_denied(&evaluation);
        assert!(evaluation.reason.contains("limite dure"));
    }

    #[test]
    fn multiple_approval_bands_merge_into_one_approval() {
        let mut r = rule("mail.send");
        r.amount = Some(Bound::two_tier(Cents(100), Cents(1_000)).unwrap());
        r.recipients = Some(Bound::two_tier(5u32, 50).unwrap());
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
        r.recipients = Some(Bound::two_tier(5u32, 50).unwrap());
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
    fn reasons_name_the_tier_that_played() {
        let mut r = rule("paiement");
        r.amount = Some(Bound::two_tier(Cents(10), Cents(100)).unwrap());
        let rules = [r];

        // Dans la bande : la raison nomme le seuil de validation ET rappelle
        // la limite dure, pour que le dirigeant sache ce qu'il arbitre.
        let mut req = request("paiement");
        req.amount = Some(Cents(11));
        let reason = decide(&rules, &req).reason;
        assert!(reason.contains("seuil de validation"));
        assert!(reason.contains("limite dure"));

        // Au-delà : la raison nomme la limite dure.
        let mut req = request("paiement");
        req.amount = Some(Cents(101));
        assert!(decide(&rules, &req).reason.contains("limite dure dépassée"));
    }

    #[test]
    fn first_matching_rule_wins() {
        let mut first = rule("doc.write");
        first.allowed = false;
        let second = rule("doc.write");
        assert_denied(&decide(&[first, second], &request("doc.write")));
    }
}
