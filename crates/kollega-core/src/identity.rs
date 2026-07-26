//! Identité : rôle d'un utilisateur et adresse électronique validée.

use serde::{Deserialize, Deserializer, Serialize};

/// Rôle d'un utilisateur dans son organisation. Deux rôles, rien de plus (M1).
///
/// Formes sérialisées : `proprietaire`, `membre` — les valeurs de la colonne
/// `users.role` (sans accent, comme tout identifiant SQL).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Propriétaire de l'organisation : administre, invite, valide.
    Proprietaire,
    /// Membre : utilise.
    Membre,
}

/// Erreur de validation d'une adresse électronique.
///
/// Volontairement sans la valeur fautive : une adresse est une donnée
/// personnelle, elle ne doit pas se retrouver dans un journal via un
/// message d'erreur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EmailError {
    /// Adresse vide (après retrait des blancs de tête et de queue).
    #[error("adresse vide")]
    Empty,
    /// L'adresse ne contient pas exactement un `@`.
    #[error("l'adresse doit contenir exactement un @")]
    AtCount,
    /// Partie locale absente ou dépassant 64 octets.
    #[error("partie locale invalide (1 à 64 octets)")]
    InvalidLocalPart,
    /// Domaine absent, sans point, ou mal borné.
    #[error("domaine invalide")]
    InvalidDomain,
    /// Adresse dépassant 254 octets après normalisation.
    #[error("adresse trop longue (254 octets maximum)")]
    TooLong,
    /// Caractère d'espacement ou de contrôle à l'intérieur de l'adresse.
    #[error("caractère interdit dans l'adresse")]
    ForbiddenCharacter,
}

/// Adresse électronique validée et normalisée en minuscules.
///
/// Construction uniquement par [`Email::parse`] : une valeur de ce type est
/// toujours structurellement valide, et la désérialisation repasse par la
/// même validation — impossible de la contourner par un JSON.
///
/// Règles, délibérément simples et strictes (choix consignés dans
/// `docs/questions-nuit.md`) :
/// - blancs de tête et de queue retirés, puis interdits partout ;
/// - exactement un `@` ;
/// - partie locale : 1 à 64 octets ;
/// - domaine : au moins un point ; chaque étiquette est non vide (pas de
///   point en tête, en queue, ni doublé), composée de lettres, chiffres et
///   tirets, sans tiret en tête ni en queue d'étiquette ;
/// - longueur totale ≤ 254 octets après normalisation ;
/// - normalisation : adresse entière en minuscules (pratique universelle,
///   même si la RFC autorise une partie locale sensible à la casse).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Email(String);

impl Email {
    /// Valide et normalise une adresse.
    pub fn parse(input: &str) -> Result<Email, EmailError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(EmailError::Empty);
        }
        if trimmed.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return Err(EmailError::ForbiddenCharacter);
        }
        let lowered = trimmed.to_lowercase();
        if lowered.len() > 254 {
            return Err(EmailError::TooLong);
        }
        let mut parts = lowered.split('@');
        let (local, domain) = match (parts.next(), parts.next(), parts.next()) {
            (Some(local), Some(domain), None) => (local, domain),
            _ => return Err(EmailError::AtCount),
        };
        if local.is_empty() || local.len() > 64 {
            return Err(EmailError::InvalidLocalPart);
        }
        let valid_label = |label: &str| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_alphanumeric() || c == '-')
        };
        if !domain.contains('.') || !domain.split('.').all(valid_label) {
            return Err(EmailError::InvalidDomain);
        }
        Ok(Email(lowered))
    }

    /// L'adresse normalisée.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for Email {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Email {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Email::parse(&raw).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_matches_schema_strings() {
        assert_eq!(
            serde_json::to_value(Role::Proprietaire).unwrap(),
            serde_json::json!("proprietaire")
        );
        assert_eq!(
            serde_json::to_value(Role::Membre).unwrap(),
            serde_json::json!("membre")
        );
    }

    #[test]
    fn email_accepts_and_normalizes() {
        let cases = [
            ("simple@example.com", "simple@example.com"),
            ("MiXeD@ExAmPle.COM", "mixed@example.com"),
            ("  entoure@example.com  ", "entoure@example.com"),
            (
                "point.local+tag@sous.domaine.fr",
                "point.local+tag@sous.domaine.fr",
            ),
            ("chiffres123@exa-mple.org", "chiffres123@exa-mple.org"),
        ];
        for (input, expected) in cases {
            let email = Email::parse(input).unwrap_or_else(|e| panic!("{input} rejeté : {e}"));
            assert_eq!(email.as_str(), expected);
        }
    }

    #[test]
    fn email_rejects_invalid() {
        let cases: [(&str, EmailError); 14] = [
            ("", EmailError::Empty),
            ("   ", EmailError::Empty),
            ("sans-arobase.example.com", EmailError::AtCount),
            ("deux@@example.com", EmailError::AtCount),
            ("a@b@c.com", EmailError::AtCount),
            ("@example.com", EmailError::InvalidLocalPart),
            ("local@", EmailError::InvalidDomain),
            ("local@sanspoint", EmailError::InvalidDomain),
            ("local@.commence.par.point", EmailError::InvalidDomain),
            ("local@finit.par.point.", EmailError::InvalidDomain),
            ("local@double..point.com", EmailError::InvalidDomain),
            ("local@symbole!.com", EmailError::InvalidDomain),
            ("local@-tiret.com", EmailError::InvalidDomain),
            ("es pace@example.com", EmailError::ForbiddenCharacter),
        ];
        for (input, expected) in cases {
            assert_eq!(Email::parse(input), Err(expected), "entrée : {input:?}");
        }
        assert_eq!(
            Email::parse("tab\t@example.com"),
            Err(EmailError::ForbiddenCharacter)
        );
        assert_eq!(
            Email::parse("controle\u{7}@example.com"),
            Err(EmailError::ForbiddenCharacter)
        );
    }

    #[test]
    fn email_boundary_lengths() {
        // Partie locale : 64 octets acceptés, 65 refusés.
        let local_64 = format!("{}@example.com", "a".repeat(64));
        assert!(Email::parse(&local_64).is_ok());
        let local_65 = format!("{}@example.com", "a".repeat(65));
        assert_eq!(Email::parse(&local_65), Err(EmailError::InvalidLocalPart));

        // Longueur totale : 254 octets acceptés, 255 refusés.
        // 64 (local) + 1 (@) + 189 (domaine) = 254.
        let domain_189 = format!("{}.{}.{}", "b".repeat(63), "b".repeat(63), "b".repeat(61));
        assert_eq!(domain_189.len(), 189);
        let total_254 = format!("{}@{}", "a".repeat(64), domain_189);
        assert_eq!(total_254.len(), 254);
        assert!(Email::parse(&total_254).is_ok());
        let total_255 = format!("{}@{}x", "a".repeat(64), domain_189);
        assert_eq!(Email::parse(&total_255), Err(EmailError::TooLong));
    }

    #[test]
    fn email_deserialization_revalidates() {
        // La désérialisation passe par parse : un JSON invalide est refusé,
        // un JSON valide est normalisé.
        let ok: Email = serde_json::from_value(serde_json::json!("Upper@Example.COM")).unwrap();
        assert_eq!(ok.as_str(), "upper@example.com");
        assert!(serde_json::from_value::<Email>(serde_json::json!("pas-un-email")).is_err());
        assert!(serde_json::from_value::<Email>(serde_json::json!(42)).is_err());
    }

    #[test]
    fn email_error_messages_do_not_echo_input() {
        // Une adresse est une donnée personnelle : l'erreur ne la répète pas.
        let input = "Adresse.Privee@Entreprise-Confidentielle.fr!!";
        let err = Email::parse(input).unwrap_err();
        let shown = format!("{err} / {err:?}");
        assert!(!shown.to_lowercase().contains("privee"));
    }
}
