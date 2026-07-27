//! Fournisseur de modèle — le contrat RÉEL, faillible (bloc 2, 28/07/2026).
//!
//! Le trait pur de la machine (`kollega_runtime::machine::ModelProvider`)
//! est infaillible par construction : c'est un modèle réduit. Le monde réel
//! échoue — limite de débit, délai dépassé, réponse tronquée, comptage de
//! jetons différent de l'estimation. Ce crate porte le contrat qui DIT ces
//! échecs, et un double de test qui les rejoue : le code appelant du jalon
//! M3 s'écrira contre des pannes réalistes, pas contre un ciel bleu.
//!
//! Aucune clé d'API n'était disponible dans l'environnement de cette
//! session : AUCUN appel réel n'a été fait, aucune clé n'a été cherchée ni
//! demandée. L'implémentation HTTP réelle arrive au jalon M3.
//!
//! RÈGLE ABSOLUE (le dépôt est public) : une clé d'API ne sort JAMAIS — ni
//! Debug, ni Display, ni erreur, ni journal. [`ApiKey`] rend la fuite
//! inexprimable par les chemins de formatage, et un test le prouve.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::sync::Mutex;

use kollega_core::CompiledPrompt;

/// Clé d'API — un secret qui ne se lit qu'au moment de l'appel.
///
/// `Debug` et `Display` sont VOLONTAIREMENT expurgés : formater une
/// [`ApiKey`] ne peut pas produire le secret. L'octroi (`reveal`) est
/// explicite, nommé, et n'a qu'un seul usage légitime : l'en-tête
/// d'authentification de l'appel HTTP, au dernier moment (invariant 8,
/// même philosophie que les jetons OAuth).
pub struct ApiKey(String);

impl ApiKey {
    /// Enveloppe un secret venu de l'environnement de l'exploitant.
    #[must_use]
    pub fn new(secret: String) -> Self {
        ApiKey(secret)
    }

    /// Livre le secret — à l'en-tête d'authentification, à rien d'autre.
    #[must_use]
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiKey(«expurgée»)")
    }
}

impl std::fmt::Display for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("«expurgée»")
    }
}

/// Jetons réellement consommés par un appel — la FACTURE, pas l'estimation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenUsage {
    /// Jetons d'entrée facturés par le fournisseur.
    pub input_tokens: u64,
    /// Jetons de sortie facturés par le fournisseur.
    pub output_tokens: u64,
}

/// Estimation AVANT appel — ce que le noyau de budget connaît.
///
/// L'écart estimation/facture est un fait de la vie (comptage de jetons
/// côté fournisseur) : le contrat le rend visible au lieu de le cacher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenEstimate {
    /// Entrée estimée (heuristique locale).
    pub input_tokens: u64,
    /// Plafond de sortie demandé.
    pub max_output_tokens: u64,
}

/// Requête de complétion : le message STRUCTURÉ (invariant 7 — les trois
/// champs partent en rôles distincts, jamais concaténés) et l'estimation.
#[derive(Debug, Clone)]
pub struct ModelRequest {
    /// Le message compilé, origines explicites.
    pub prompt: CompiledPrompt,
    /// Estimation soumise au budget avant l'appel.
    pub estimate: TokenEstimate,
}

/// Réponse d'un appel abouti.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelResponse {
    /// Texte produit.
    pub text: String,
    /// Facture réelle en jetons — à réconcilier avec l'estimation.
    pub usage: TokenUsage,
}

/// Modes d'échec RÉELS d'un fournisseur — ceux que M3 devra traiter.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelFailure {
    /// Limite de débit : réessayer après le délai indiqué, jamais en boucle.
    #[error("limite de débit du fournisseur (réessai indiqué : {retry_after_seconds} s)")]
    RateLimited {
        /// Délai suggéré par le fournisseur.
        retry_after_seconds: u32,
    },
    /// Délai dépassé : l'effet de l'appel est INCONNU (peut avoir été
    /// facturé) — même statut épistémique qu'une interruption.
    #[error("délai dépassé après {elapsed_ms} ms — effet de l'appel inconnu")]
    Timeout {
        /// Temps écoulé avant l'abandon.
        elapsed_ms: u64,
    },
    /// Réponse tronquée par le plafond de sortie : PARTIELLE, et facturée.
    #[error("réponse tronquée au plafond de sortie — partielle et facturée")]
    Truncated {
        /// Le fragment produit avant la coupe.
        partial: String,
        /// La facture, due malgré la troncature.
        usage: TokenUsage,
    },
    /// Réponse du fournisseur inintelligible. Le détail ne transporte JAMAIS
    /// de contenu de requête ni de clé.
    #[error("réponse du fournisseur invalide : {detail}")]
    Protocol {
        /// Description SANS contenu sensible.
        detail: String,
    },
}

/// Le contrat réel d'un fournisseur de modèle.
pub trait ModelProvider {
    /// Vrai si l'appel SORT de notre infrastructure (journalisation de
    /// sortie de données exigée — AI Act et RGPD).
    fn is_external(&self) -> bool;

    /// Une complétion. Faillible, facturée en jetons réels.
    fn complete(&self, request: &ModelRequest) -> Result<ModelResponse, ModelFailure>;
}

/// Issue scriptée pour le double de test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptedOutcome {
    /// L'appel aboutit, avec la facture donnée.
    Succeed {
        /// Texte rendu.
        text: String,
        /// Facture réelle — PEUT différer de l'estimation, c'est le but.
        usage: TokenUsage,
    },
    /// L'appel échoue du mode donné.
    Fail(ModelFailure),
}

/// Double de test : rejoue une séquence d'issues, déterministe.
///
/// C'est le fournisseur des tests de M3 : chaque mode d'échec réel est
/// rejouable à volonté, y compris la facture qui diverge de l'estimation.
pub struct ScriptedProvider {
    outcomes: Mutex<std::collections::VecDeque<ScriptedOutcome>>,
    external: bool,
}

impl ScriptedProvider {
    /// Fournisseur rejouant `outcomes` dans l'ordre.
    #[must_use]
    pub fn new(outcomes: Vec<ScriptedOutcome>, external: bool) -> Self {
        ScriptedProvider {
            outcomes: Mutex::new(outcomes.into()),
            external,
        }
    }
}

impl ModelProvider for ScriptedProvider {
    fn is_external(&self) -> bool {
        self.external
    }

    fn complete(&self, _request: &ModelRequest) -> Result<ModelResponse, ModelFailure> {
        let next = self
            .outcomes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pop_front();
        match next {
            Some(ScriptedOutcome::Succeed { text, usage }) => Ok(ModelResponse { text, usage }),
            Some(ScriptedOutcome::Fail(failure)) => Err(failure),
            None => Err(ModelFailure::Protocol {
                detail: "script épuisé : plus d'issue programmée".to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kollega_core::{prompt::compile, AssemblyLimits, Segment};

    fn request() -> ModelRequest {
        let segments = vec![
            Segment::SystemInstruction("Agent de relance.".to_owned()),
            Segment::UserRequest("Relance la facture 42.".to_owned()),
        ];
        ModelRequest {
            prompt: compile(&segments, &AssemblyLimits::default()).expect("assemblage"),
            estimate: TokenEstimate {
                input_tokens: 200,
                max_output_tokens: 300,
            },
        }
    }

    #[test]
    fn scripted_provider_replays_every_real_failure_mode() {
        let provider = ScriptedProvider::new(
            vec![
                ScriptedOutcome::Fail(ModelFailure::RateLimited {
                    retry_after_seconds: 30,
                }),
                ScriptedOutcome::Fail(ModelFailure::Timeout { elapsed_ms: 30_000 }),
                ScriptedOutcome::Fail(ModelFailure::Truncated {
                    partial: "Madame, Monsieur, je me permets de".to_owned(),
                    usage: TokenUsage {
                        input_tokens: 210,
                        output_tokens: 300,
                    },
                }),
                ScriptedOutcome::Succeed {
                    text: "Relance envoyée.".to_owned(),
                    usage: TokenUsage {
                        input_tokens: 187,
                        output_tokens: 42,
                    },
                },
            ],
            true,
        );
        let req = request();
        assert!(matches!(
            provider.complete(&req),
            Err(ModelFailure::RateLimited {
                retry_after_seconds: 30
            })
        ));
        assert!(matches!(
            provider.complete(&req),
            Err(ModelFailure::Timeout { elapsed_ms: 30_000 })
        ));
        match provider.complete(&req) {
            Err(ModelFailure::Truncated { partial, usage }) => {
                assert!(!partial.is_empty(), "le fragment partiel est conservé");
                assert_eq!(usage.output_tokens, 300, "la troncature est FACTURÉE");
            }
            other => panic!("attendu une troncature : {other:?}"),
        }
        let ok = provider.complete(&req).expect("dernier appel abouti");
        // La facture réelle DIFFÈRE de l'estimation : le consommateur doit
        // réconcilier, jamais supposer estimation == facture.
        assert_ne!(ok.usage.input_tokens, req.estimate.input_tokens);
        assert!(provider.complete(&req).is_err(), "script épuisé = erreur");
    }

    #[test]
    fn api_key_never_leaks_through_formatting_or_errors() {
        let secret = "sk-test-EXEMPLE-0000-ne-pas-imiter";
        let key = ApiKey::new(secret.to_owned());
        // Ni Debug, ni Display ne livrent le secret.
        assert!(!format!("{key:?}").contains(secret));
        assert!(!format!("{key}").contains(secret));
        assert!(format!("{key:?}").contains("expurgée"));
        // Une structure de configuration qui dérive Debug ne fuit pas non
        // plus : la rédaction est portée par le TYPE, pas par la discipline.
        #[derive(Debug)]
        #[allow(dead_code)]
        struct ProviderConfig {
            endpoint: String,
            api_key: ApiKey,
        }
        let config = ProviderConfig {
            endpoint: "https://api.exemple.invalid/v1".to_owned(),
            api_key: key,
        };
        assert!(!format!("{config:?}").contains(secret));
        // Les erreurs du contrat ne peuvent pas transporter la clé : aucun
        // constructeur ne l'accepte, et les messages sont des données figées.
        let failure = ModelFailure::Protocol {
            detail: "code 500 sans corps".to_owned(),
        };
        assert!(!failure.to_string().contains(secret));
        // `reveal` reste le seul chemin, explicite.
        assert_eq!(config.api_key.reveal(), secret);
    }
}
