//! Contenu d'audit et son empreinte — des objets de PREUVE (bloc 3).
//!
//! [`ContentDigest`] n'a qu'un seul constructeur de domaine :
//! [`ContentDigest::of`], qui CALCULE l'empreinte du contenu. Pas de
//! `From<[u8; 32]>`, pas de `new(bytes)`, pas de `Deserialize` : un digest
//! qui ne proviendrait pas d'un calcul sur un contenu réel n'est pas
//! représentable depuis le domaine. La relecture depuis le stockage — où
//! l'empreinte EST une donnée brute — passe par la frontière explicite
//! [`ContentDigest::from_storage`], activée par la feature
//! `storage-boundary` que seul le crate de persistance déclare ; une garde
//! textuelle (`crates/kollega-cli/tests/storage_boundary.rs`) échoue si
//! `from_storage` est invoqué ailleurs.
//!
//! [`AuditContent`] (bloc 3c) porte le contenu et son organisation ;
//! l'empreinte est une MÉTHODE, pas un champ : aucun chemin ne permet de
//! construire un objet dont l'empreinte ne correspondrait pas à son
//! contenu.

use sha2::{Digest as _, Sha256};

use kollega_core::OrgId;

/// Charge utile d'un contenu d'audit : les octets réellement produits
/// (l'événement sérialisé, la requête d'outil, le résultat…).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPayload(String);

impl ContentPayload {
    /// Enveloppe le contenu tel qu'il est arrivé — verbatim.
    #[must_use]
    pub fn new(content: String) -> Self {
        ContentPayload(content)
    }

    /// Les octets du contenu.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Empreinte SHA-256 d'un [`ContentPayload`] — un objet de preuve.
///
/// Il n'existe que deux chemins vers une valeur de ce type : le CALCUL
/// ([`ContentDigest::of`]) et la frontière de stockage
/// ([`ContentDigest::from_storage`], restreinte par feature au crate de
/// persistance). Volontairement : ni `Deserialize`, ni `From` de tableaux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentDigest([u8; 32]);

impl ContentDigest {
    /// L'UNIQUE constructeur de domaine : l'empreinte est calculée, jamais
    /// affirmée.
    #[must_use]
    pub fn of(payload: &ContentPayload) -> Self {
        ContentDigest(Sha256::digest(payload.0.as_bytes()).into())
    }

    /// Frontière de stockage : relit une empreinte PERSISTÉE.
    ///
    /// N'existe que sous la feature `storage-boundary`, déclarée par le
    /// seul crate de persistance ; la garde textuelle échoue si un autre
    /// crate l'invoque. Relire une empreinte, c'est faire confiance au
    /// stockage — la vérification de chaîne est ce qui rend cette confiance
    /// honnête.
    #[cfg(feature = "storage-boundary")]
    #[must_use]
    pub fn from_storage(bytes: [u8; 32]) -> Self {
        ContentDigest(bytes)
    }

    /// Les 32 octets de l'empreinte.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Forme hexadécimale minuscule (celle des charges utiles canoniques).
    #[must_use]
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

/// Contenu d'audit attribué à une organisation (bloc 3c).
///
/// L'empreinte n'est PAS un champ : elle se calcule ([`AuditContent::digest`])
/// — un `AuditContent` dont l'empreinte mentirait sur son contenu n'est pas
/// constructible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditContent {
    org: OrgId,
    payload: ContentPayload,
}

impl AuditContent {
    /// Contenu attribué : l'organisation vient du contexte, le contenu des
    /// octets réels.
    #[must_use]
    pub fn new(org: OrgId, payload: ContentPayload) -> Self {
        AuditContent { org, payload }
    }

    /// L'organisation propriétaire.
    #[must_use]
    pub fn org(&self) -> OrgId {
        self.org
    }

    /// Le contenu verbatim.
    #[must_use]
    pub fn payload(&self) -> &ContentPayload {
        &self.payload
    }

    /// L'empreinte, CALCULÉE à la demande — jamais stockée dans l'objet.
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        ContentDigest::of(&self.payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn digest_is_computed_never_asserted() {
        let a = ContentPayload::new("contenu A".to_owned());
        let b = ContentPayload::new("contenu B".to_owned());
        assert_eq!(ContentDigest::of(&a), ContentDigest::of(&a), "déterministe");
        assert_ne!(ContentDigest::of(&a), ContentDigest::of(&b));
        // Vecteur de référence : SHA-256("") — figé, connu, indépendant.
        let empty = ContentPayload::new(String::new());
        assert_eq!(
            ContentDigest::of(&empty).to_hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn audit_content_digest_always_matches_its_payload() {
        let content = AuditContent::new(
            OrgId::new(Uuid::from_u128(0xA)),
            ContentPayload::new("relance facture 42".to_owned()),
        );
        assert_eq!(
            content.digest(),
            ContentDigest::of(content.payload()),
            "l'empreinte est une fonction du contenu, pas un champ"
        );
    }
}
