//! Ancre de confiance — partie **pure** (défaut retenu : publication au
//! client, révocable — voir `docs/audit-modele-de-menace.md`).
//!
//! Une ancre est la photographie de la tête d'une chaîne à un instant :
//! (organisation, hauteur, empreinte, horodatage). Publiée hors d'atteinte
//! d'un attaquant du stockage — stockage objet en écriture seule ET remise
//! au dirigeant — elle est ce qui rend détectables la troncature de queue
//! et la réécriture d'un suffixe, indétectables par [`OrgChain::verify`]
//! seul.
//!
//! LIMITE HONNÊTE : une ancre protège la chaîne JUSQU'À sa hauteur. Ce qui
//! est écrit après la dernière ancre publiée n'est protégé par rien jusqu'à
//! la publication suivante — c'est la fenêtre d'ancrage, et elle est
//! démontrée par un test, pas cachée.

use kollega_core::OrgId;

use crate::chain::{ChainBreak, Hash32, OrgChain, StoredEntry};

/// Photographie ancrée de la tête d'une chaîne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    /// Organisation dont c'est l'ancre.
    pub org_id: OrgId,
    /// Hauteur de l'entrée ancrée (0 = première entrée).
    pub height: u64,
    /// Empreinte de l'entrée à cette hauteur.
    pub head: Hash32,
    /// Horodatage de la publication (microsecondes Unix).
    pub anchored_at_micros: i64,
}

/// Violation constatée en vérifiant une chaîne contre une ancre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AnchorViolation {
    /// L'ancre appartient à une autre organisation : elle ne prouve rien ici.
    #[error("ancre d'une autre organisation")]
    WrongOrganization,
    /// La chaîne est plus courte que la hauteur ancrée : troncature de queue.
    #[error(
        "chaîne tronquée sous l'ancre (longueur {chain_len}, ancre à la hauteur {anchor_height})"
    )]
    TruncatedBelowAnchor {
        /// Longueur observée de la chaîne.
        chain_len: usize,
        /// Hauteur que l'ancre garantit exister.
        anchor_height: u64,
    },
    /// L'entrée à la hauteur ancrée ne porte pas l'empreinte ancrée :
    /// le suffixe a été réécrit.
    #[error("l'empreinte à la hauteur {height} ne correspond pas à l'ancre")]
    HeadMismatch {
        /// Hauteur ancrée dont l'empreinte diverge.
        height: u64,
    },
    /// La chaîne elle-même est intérieurement incohérente.
    #[error("chaîne incohérente : {0}")]
    Chain(#[from] ChainBreak),
}

impl OrgChain {
    /// Ancre de la tête actuelle de `entries` (None si la chaîne est vide).
    #[must_use]
    pub fn head_anchor(&self, entries: &[StoredEntry], anchored_at_micros: i64) -> Option<Anchor> {
        entries.last().map(|tail| Anchor {
            org_id: self.org_id(),
            height: tail.height,
            head: tail.hash,
            anchored_at_micros,
        })
    }

    /// Vérifie la cohérence interne ET l'accord avec une ancre de confiance.
    ///
    /// L'ancre peut être plus ancienne que la chaîne (la chaîne a grandi
    /// depuis la dernière publication) : elle garantit alors le préfixe
    /// jusqu'à sa hauteur. Ce qui suit la hauteur ancrée n'est garanti que
    /// par la cohérence interne — voir la limite documentée du module.
    pub fn verify_with_anchor(
        &self,
        entries: &[StoredEntry],
        anchor: &Anchor,
    ) -> Result<(), AnchorViolation> {
        if anchor.org_id != self.org_id() {
            return Err(AnchorViolation::WrongOrganization);
        }
        self.verify(entries)?;
        let anchored_position =
            usize::try_from(anchor.height).map_err(|_| AnchorViolation::TruncatedBelowAnchor {
                chain_len: entries.len(),
                anchor_height: anchor.height,
            })?;
        let Some(anchored_entry) = entries.get(anchored_position) else {
            return Err(AnchorViolation::TruncatedBelowAnchor {
                chain_len: entries.len(),
                anchor_height: anchor.height,
            });
        };
        if anchored_entry.hash != anchor.head {
            return Err(AnchorViolation::HeadMismatch {
                height: anchor.height,
            });
        }
        Ok(())
    }
}

/// Publication d'une ancre vers un témoin externe.
///
/// Les implémentations réelles (stockage objet en écriture seule avec
/// rétention, remise quotidienne au dirigeant) arrivent au jalon de
/// persistance. Contrat : **ajout seul** — une ancre publiée ne se remplace
/// pas, et la hauteur publiée pour une organisation ne régresse jamais.
pub trait AnchorPublisher {
    /// Erreur de publication propre à l'implémentation.
    type Error: core::fmt::Debug;

    /// Publie une ancre chez le témoin.
    fn publish(&mut self, anchor: Anchor) -> Result<(), Self::Error>;

    /// Dernière ancre connue pour une organisation.
    fn latest_for(&self, org_id: OrgId) -> Option<&Anchor>;
}

/// Erreur du témoin en mémoire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InMemoryAnchorError {
    /// La hauteur publiée régresse par rapport à la dernière ancre de la
    /// même organisation : un témoin n'accepte jamais de « revenir en
    /// arrière ».
    #[error("hauteur d'ancre en régression")]
    HeightRegressed,
}

/// Témoin de test, en mémoire, en ajout seul.
#[derive(Debug, Default)]
pub struct InMemoryAnchorPublisher {
    anchors: Vec<Anchor>,
}

impl InMemoryAnchorPublisher {
    /// Témoin vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Toutes les ancres publiées, dans l'ordre de publication.
    #[must_use]
    pub fn all(&self) -> &[Anchor] {
        &self.anchors
    }
}

impl AnchorPublisher for InMemoryAnchorPublisher {
    type Error = InMemoryAnchorError;

    fn publish(&mut self, anchor: Anchor) -> Result<(), Self::Error> {
        if let Some(latest) = self.latest_for(anchor.org_id) {
            if anchor.height < latest.height {
                return Err(InMemoryAnchorError::HeightRegressed);
            }
        }
        self.anchors.push(anchor);
        Ok(())
    }

    fn latest_for(&self, org_id: OrgId) -> Option<&Anchor> {
        self.anchors.iter().rev().find(|a| a.org_id == org_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical::CanonicalValue;
    use crate::chain::EntryContent;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn chain(n: u128) -> OrgChain {
        OrgChain::new(OrgId::new(Uuid::from_u128(n)))
    }

    fn content(i: i64) -> EntryContent {
        let mut payload = BTreeMap::new();
        payload.insert("indice".to_owned(), CanonicalValue::Int(i));
        EntryContent {
            actor: "system".to_owned(),
            action: "test_event".to_owned(),
            payload: CanonicalValue::Object(payload),
            timestamp_micros: 1_753_600_000_000_000 + i,
        }
    }

    fn build(c: &OrgChain, n: i64) -> Vec<StoredEntry> {
        let mut entries = Vec::new();
        for i in 0..n {
            let next: StoredEntry = c
                .append(entries.last().map(StoredEntry::tip), content(i))
                .into();
            entries.push(next);
        }
        entries
    }

    #[test]
    fn valid_chain_with_current_anchor_passes() {
        let c = chain(0xA);
        let entries = build(&c, 5);
        let anchor = c.head_anchor(&entries, 1).unwrap();
        assert_eq!(c.verify_with_anchor(&entries, &anchor), Ok(()));
    }

    #[test]
    fn older_anchor_still_validates_a_grown_chain() {
        // L'ancre d'hier reste vraie après que la chaîne a grandi.
        let c = chain(0xA);
        let mut entries = build(&c, 3);
        let anchor = c.head_anchor(&entries, 1).unwrap();
        for i in 3..6 {
            let next: StoredEntry = c
                .append(entries.last().map(StoredEntry::tip), content(i))
                .into();
            entries.push(next);
        }
        assert_eq!(c.verify_with_anchor(&entries, &anchor), Ok(()));
    }

    #[test]
    fn truncations_of_one_n_and_all_are_detected() {
        let c = chain(0xA);
        let entries = build(&c, 6);
        let anchor = c.head_anchor(&entries, 1).unwrap();
        for keep in [5usize, 2, 0] {
            let truncated = &entries[..keep];
            assert_eq!(
                c.verify_with_anchor(truncated, &anchor),
                Err(AnchorViolation::TruncatedBelowAnchor {
                    chain_len: keep,
                    anchor_height: 5,
                }),
                "troncature à {keep} entrées non détectée"
            );
        }
    }

    #[test]
    fn rewritten_suffix_is_detected_by_anchor() {
        // L'attaquant réécrit l'entrée 2 et recoud tout : la cohérence
        // interne passe, l'ancre le trahit.
        let c = chain(0xA);
        let entries = build(&c, 5);
        let anchor = c.head_anchor(&entries, 1).unwrap();

        let mut forged: Vec<_> = entries[..2].to_vec();
        let mut altered = content(2);
        altered.actor = "attaquant".to_owned();
        let next: StoredEntry = c
            .append(forged.last().map(StoredEntry::tip), altered)
            .into();
        forged.push(next);
        for i in 3..5 {
            let next: StoredEntry = c
                .append(forged.last().map(StoredEntry::tip), content(i))
                .into();
            forged.push(next);
        }
        assert_eq!(
            c.verify(&forged),
            Ok(()),
            "cohérence interne : limite connue"
        );
        assert_eq!(
            c.verify_with_anchor(&forged, &anchor),
            Err(AnchorViolation::HeadMismatch { height: 4 })
        );
    }

    #[test]
    fn rewrite_after_anchor_is_invisible_until_next_anchor() {
        // LIMITE DOCUMENTÉE : ce qui est écrit après la dernière ancre n'est
        // protégé que par la cohérence interne. Un suffixe réécrit AU-DELÀ
        // de la hauteur ancrée passe — c'est la fenêtre d'ancrage.
        let c = chain(0xA);
        let mut entries = build(&c, 3);
        let anchor = c.head_anchor(&entries, 1).unwrap(); // hauteur 2
        let next: StoredEntry = c
            .append(entries.last().map(StoredEntry::tip), content(3))
            .into();
        entries.push(next);

        let mut forged: Vec<_> = entries[..3].to_vec();
        let mut altered = content(99);
        altered.actor = "attaquant".to_owned();
        let next: StoredEntry = c
            .append(forged.last().map(StoredEntry::tip), altered)
            .into();
        forged.push(next);

        assert_eq!(
            c.verify_with_anchor(&forged, &anchor),
            Ok(()),
            "fenêtre d'ancrage : assumée et documentée, pas cachée"
        );
    }

    #[test]
    fn incoherent_chain_is_reported_as_chain_break() {
        let c = chain(0xA);
        let mut entries = build(&c, 4);
        let anchor = c.head_anchor(&entries, 1).unwrap();
        entries[1].content.actor = "attaquant".to_owned();
        assert!(matches!(
            c.verify_with_anchor(&entries, &anchor),
            Err(AnchorViolation::Chain(_))
        ));
    }

    #[test]
    fn empty_chain_with_anchor_is_a_truncation() {
        let c = chain(0xA);
        let entries = build(&c, 2);
        let anchor = c.head_anchor(&entries, 1).unwrap();
        assert_eq!(
            c.verify_with_anchor(&[], &anchor),
            Err(AnchorViolation::TruncatedBelowAnchor {
                chain_len: 0,
                anchor_height: 1,
            })
        );
    }

    #[test]
    fn anchor_of_another_organization_is_refused() {
        let a = chain(0xA);
        let b = chain(0xB);
        let entries_a = build(&a, 3);
        let entries_b = build(&b, 3);
        let anchor_b = b.head_anchor(&entries_b, 1).unwrap();
        assert_eq!(
            a.verify_with_anchor(&entries_a, &anchor_b),
            Err(AnchorViolation::WrongOrganization)
        );
    }

    #[test]
    fn publisher_is_append_only_and_monotonic() {
        let c = chain(0xA);
        let entries = build(&c, 4);
        let mut publisher = InMemoryAnchorPublisher::new();

        let early = c.head_anchor(&entries[..2], 1).unwrap();
        let late = c.head_anchor(&entries, 2).unwrap();
        publisher.publish(early).unwrap();
        publisher.publish(late).unwrap();
        assert_eq!(publisher.latest_for(c.org_id()), Some(&late));
        assert_eq!(publisher.all().len(), 2);

        // Régression de hauteur : refusée.
        assert_eq!(
            publisher.publish(early),
            Err(InMemoryAnchorError::HeightRegressed)
        );

        // Une autre organisation ne voit pas ces ancres.
        assert_eq!(publisher.latest_for(chain(0xB).org_id()), None);
    }
}
