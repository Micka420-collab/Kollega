//! La forme des dépôts PORTE le cycle de vie (bloc 3f).
//!
//! [`AuditChainRepository`] : `append`, `read` — RIEN d'autre. Aucune
//! méthode capable de retirer quoi que ce soit n'existe sur le dépôt de
//! chaîne : l'ajout seul n'est pas une discipline, c'est la surface du
//! trait (doublée, côté base, par des GRANT sans UPDATE ni DELETE). Une
//! garde textuelle (`crates/kollega-cli/tests/repository_shape.rs`) échoue
//! si une méthode de suppression apparaît un jour ici.
//!
//! [`AuditContentRepository`] : `put`, `read`, `purge_org` — le contenu,
//! lui, est purgeable par organisation (invariant 12, RGPD), et la purge
//! est un acte nommé, pas un `delete` générique.

#![allow(async_fn_in_trait)] // contrats internes : pas de bornes d'envoi à figer ici

use crate::chain::StoredEntry;
use crate::content::{AuditContent, ContentDigest, ContentPayload};

/// Dépôt de la chaîne d'attestations — AJOUT SEUL, par construction.
pub trait AuditChainRepository {
    /// Erreur du support de persistance.
    type Error;

    /// Ajoute une attestation en queue de chaîne (hauteur et lien calculés
    /// par l'implémentation, dans sa transaction).
    async fn append(
        &mut self,
        actor: &str,
        action: &str,
        content: &AuditContent,
    ) -> Result<(), Self::Error>;

    /// Relit la chaîne entière de l'organisation, ordonnée par hauteur.
    ///
    /// Rend des [`StoredEntry`] : ce qui sort du stockage n'est pas une
    /// preuve, c'est une prétention — à soumettre à `OrgChain::verify`.
    async fn read(&mut self) -> Result<Vec<StoredEntry>, Self::Error>;
}

/// Dépôt du contenu d'audit — adressé par `(organisation, empreinte)`.
pub trait AuditContentRepository {
    /// Erreur du support de persistance.
    type Error;

    /// Dépose un contenu (idempotent : même empreinte, même ligne).
    async fn put(&mut self, content: &AuditContent) -> Result<(), Self::Error>;

    /// Relit un contenu par son empreinte, s'il n'a pas été purgé.
    async fn read(&mut self, digest: ContentDigest) -> Result<Option<ContentPayload>, Self::Error>;

    /// Purge RGPD : TOUT le contenu de l'organisation. L'acte est nommé —
    /// il n'existe pas de suppression unitaire anonyme.
    async fn purge_org(&mut self) -> Result<u64, Self::Error>;
}
