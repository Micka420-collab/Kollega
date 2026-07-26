//! Chaînage et vérification — la chaîne est PAR ORGANISATION.
//!
//! [`OrgChain`] porte l'[`OrgId`] ; les entrées n'en portent pas. Mélanger
//! deux organisations dans une même chaîne est donc impossible par
//! construction, et une entrée hachée pour A ne se vérifie pas dans la
//! chaîne de B (l'organisation entre dans les octets hachés).

use kollega_core::OrgId;
use sha2::{Digest, Sha256};

use crate::canonical::{encode_text, CanonicalValue};

/// Empreinte SHA-256 (32 octets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash32(pub [u8; 32]);

impl Hash32 {
    /// Représentation hexadécimale minuscule (64 caractères).
    #[must_use]
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl core::fmt::Display for Hash32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// Contenu d'une entrée d'audit, avant chaînage.
///
/// Pas d'`org_id` ici : l'organisation est portée par la chaîne
/// ([`OrgChain`]), jamais par l'entrée — c'est ce qui interdit le mélange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryContent {
    /// Qui agit (identifiant d'agent, de tâche, « system », …).
    pub actor: String,
    /// Ce qui s'est produit (`task_started`, `tool_call_intended`, …).
    pub action: String,
    /// Charge utile structurée, en forme canonique.
    pub payload: CanonicalValue,
    /// Microsecondes depuis l'époque Unix (précision de `timestamptz`).
    pub timestamp_micros: i64,
}

/// Entrée chaînée : contenu + lien vers la précédente + empreinte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainedEntry {
    /// Le contenu haché.
    pub content: EntryContent,
    /// Empreinte de l'entrée précédente ; `None` pour la première.
    pub prev_hash: Option<Hash32>,
    /// Empreinte de cette entrée.
    pub hash: Hash32,
}

/// Nature d'une rupture de chaîne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainBreakKind {
    /// Le `prev_hash` de l'entrée ne correspond pas à l'empreinte de
    /// l'entrée précédente (réordonnancement, suppression, insertion).
    BrokenLink,
    /// L'empreinte stockée ne correspond pas au contenu recalculé
    /// (altération du contenu ou de l'horodatage).
    AlteredEntry,
    /// L'empreinte de queue ne correspond pas à l'ancre de confiance
    /// (troncature de queue, ou suffixe réécrit par un attaquant en
    /// écriture). Émise uniquement par [`OrgChain::verify_with_tail`].
    TailMismatch,
}

/// Première rupture détectée dans une chaîne : position et nature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("rupture de chaîne d'audit en position {position} : {kind:?}")]
pub struct ChainBreak {
    /// Position (0-indexée) de la première entrée en rupture.
    pub position: usize,
    /// Nature de la rupture.
    pub kind: ChainBreakKind,
}

/// Chaîne d'audit d'UNE organisation.
///
/// Toutes les opérations (hachage, chaînage, vérification) passent par cette
/// structure : l'organisation est fixée à la construction et entre dans les
/// octets hachés de chaque entrée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrgChain {
    org_id: OrgId,
}

impl OrgChain {
    /// Chaîne de l'organisation donnée.
    #[must_use]
    pub const fn new(org_id: OrgId) -> Self {
        OrgChain { org_id }
    }

    /// L'organisation de cette chaîne.
    #[must_use]
    pub const fn org_id(&self) -> OrgId {
        self.org_id
    }

    /// Octets canoniques de l'enregistrement complet.
    ///
    /// Ordre des champs FIGÉ et documenté :
    /// `{"action":…,"actor":…,"org_id":…,"payload":…}` — l'horodatage n'est
    /// pas dans cet encodage, il est concaténé séparément (définition de
    /// l'empreinte, voir la documentation de crate).
    fn canonical_record(&self, content: &EntryContent) -> String {
        let mut out = String::new();
        out.push_str("{\"action\":");
        encode_text(&content.action, &mut out);
        out.push_str(",\"actor\":");
        encode_text(&content.actor, &mut out);
        out.push_str(",\"org_id\":");
        encode_text(&self.org_id.to_string(), &mut out);
        out.push_str(",\"payload\":");
        content.payload.encode_into(&mut out);
        out.push('}');
        out
    }

    /// Empreinte d'une entrée pour CETTE organisation :
    /// `SHA-256(prev_hash || enregistrement_canonique || horodatage)`.
    ///
    /// Pour la première entrée (`prev_hash = None`), le préfixe est 32
    /// octets à zéro : le préimage a toujours un préfixe de longueur fixe,
    /// la séparation des champs est structurelle, pas computationnelle.
    #[must_use]
    pub fn entry_hash(&self, prev_hash: Option<&Hash32>, content: &EntryContent) -> Hash32 {
        let mut hasher = Sha256::new();
        match prev_hash {
            Some(prev) => hasher.update(prev.0),
            None => hasher.update([0u8; 32]),
        }
        hasher.update(self.canonical_record(content).as_bytes());
        hasher.update(content.timestamp_micros.to_string().as_bytes());
        Hash32(hasher.finalize().into())
    }

    /// Chaîne une nouvelle entrée après `tail` (l'empreinte de la dernière
    /// entrée, ou `None` si la chaîne est vide).
    #[must_use]
    pub fn append(&self, tail: Option<&Hash32>, content: EntryContent) -> ChainedEntry {
        let hash = self.entry_hash(tail, &content);
        ChainedEntry {
            content,
            prev_hash: tail.copied(),
            hash,
        }
    }

    /// Vérifie la **cohérence interne** d'une chaîne ; première rupture.
    ///
    /// Une chaîne vide est valide. Pour chaque position : le lien d'abord
    /// (`prev_hash` doit égaler l'empreinte précédente — `None` en tête),
    /// puis l'empreinte (recalculée depuis le contenu et le lien stocké).
    ///
    /// LIMITE, assumée et documentée (voir le modèle de menace de la crate) :
    /// une troncature de queue ou un suffixe entièrement réécrit par un
    /// attaquant en écriture passent cette vérification. Contre ces deux
    /// classes, utiliser [`OrgChain::verify_with_tail`] avec une ancre de
    /// confiance externe.
    pub fn verify(&self, entries: &[ChainedEntry]) -> Result<(), ChainBreak> {
        let mut expected_prev: Option<Hash32> = None;
        for (position, entry) in entries.iter().enumerate() {
            if entry.prev_hash != expected_prev {
                return Err(ChainBreak {
                    position,
                    kind: ChainBreakKind::BrokenLink,
                });
            }
            let recomputed = self.entry_hash(entry.prev_hash.as_ref(), &entry.content);
            if recomputed != entry.hash {
                return Err(ChainBreak {
                    position,
                    kind: ChainBreakKind::AlteredEntry,
                });
            }
            expected_prev = Some(entry.hash);
        }
        Ok(())
    }

    /// Vérifie la chaîne ET son ancrage : l'empreinte de la dernière entrée
    /// doit égaler `trusted_tail`, l'ancre de confiance conservée hors
    /// d'atteinte d'un attaquant du stockage (`None` = la chaîne de
    /// confiance est vide).
    ///
    /// C'est la seule vérification qui détecte la troncature de queue et la
    /// réécriture complète d'un suffixe. En cas de désaccord, la rupture est
    /// rapportée en position `entries.len()` (l'endroit où la suite attendue
    /// manque), de nature [`ChainBreakKind::TailMismatch`].
    pub fn verify_with_tail(
        &self,
        entries: &[ChainedEntry],
        trusted_tail: Option<&Hash32>,
    ) -> Result<(), ChainBreak> {
        self.verify(entries)?;
        let actual_tail = entries.last().map(|entry| entry.hash);
        if actual_tail.as_ref() != trusted_tail {
            return Err(ChainBreak {
                position: entries.len(),
                kind: ChainBreakKind::TailMismatch,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn chain_a() -> OrgChain {
        OrgChain::new(OrgId::new(Uuid::from_u128(0xA)))
    }

    fn content(i: i64) -> EntryContent {
        let mut payload = BTreeMap::new();
        payload.insert("indice".to_owned(), CanonicalValue::Int(i));
        EntryContent {
            actor: "system".to_owned(),
            action: "test_event".to_owned(),
            payload: CanonicalValue::Object(payload),
            timestamp_micros: 1_753_500_000_000_000 + i,
        }
    }

    fn build(chain: &OrgChain, n: i64) -> Vec<ChainedEntry> {
        let mut entries: Vec<ChainedEntry> = Vec::new();
        for i in 0..n {
            let tail = entries.last().map(|e| e.hash);
            entries.push(chain.append(tail.as_ref(), content(i)));
        }
        entries
    }

    #[test]
    fn empty_chain_is_valid() {
        assert_eq!(chain_a().verify(&[]), Ok(()));
    }

    #[test]
    fn single_entry_chain_is_valid() {
        let chain = chain_a();
        let entries = build(&chain, 1);
        assert_eq!(entries[0].prev_hash, None);
        assert_eq!(chain.verify(&entries), Ok(()));
    }

    #[test]
    fn long_chain_is_valid() {
        let chain = chain_a();
        let entries = build(&chain, 50);
        assert_eq!(chain.verify(&entries), Ok(()));
    }

    #[test]
    fn altered_payload_detected_at_position() {
        let chain = chain_a();
        for altered_at in [0usize, 3, 9] {
            let mut entries = build(&chain, 10);
            entries[altered_at].content.actor = "attaquant".to_owned();
            assert_eq!(
                chain.verify(&entries),
                Err(ChainBreak {
                    position: altered_at,
                    kind: ChainBreakKind::AlteredEntry,
                })
            );
        }
    }

    #[test]
    fn altered_timestamp_detected() {
        let chain = chain_a();
        let mut entries = build(&chain, 5);
        entries[2].content.timestamp_micros += 1;
        assert_eq!(
            chain.verify(&entries),
            Err(ChainBreak {
                position: 2,
                kind: ChainBreakKind::AlteredEntry,
            })
        );
    }

    #[test]
    fn reordered_entries_detected() {
        let chain = chain_a();
        let mut entries = build(&chain, 6);
        entries.swap(2, 3);
        // La première anomalie est le lien de l'entrée déplacée en position 2.
        assert_eq!(
            chain.verify(&entries),
            Err(ChainBreak {
                position: 2,
                kind: ChainBreakKind::BrokenLink,
            })
        );
    }

    #[test]
    fn removed_middle_entry_detected() {
        let chain = chain_a();
        let mut entries = build(&chain, 6);
        entries.remove(3);
        assert_eq!(
            chain.verify(&entries),
            Err(ChainBreak {
                position: 3,
                kind: ChainBreakKind::BrokenLink,
            })
        );
    }

    #[test]
    fn genesis_with_prev_hash_is_broken() {
        let chain = chain_a();
        let entries = build(&chain, 3);
        // Une chaîne qui ne commence pas au commencement (prev != None en
        // tête) est rompue dès la position 0 : on ne peut pas cacher un
        // passé tronqué.
        let truncated = &entries[1..];
        assert_eq!(
            chain.verify(truncated),
            Err(ChainBreak {
                position: 0,
                kind: ChainBreakKind::BrokenLink,
            })
        );
    }

    #[test]
    fn chain_of_org_a_does_not_verify_as_org_b() {
        // L'organisation entre dans les octets hachés : la chaîne de A,
        // intacte, est invalide vue comme chaîne de B. Une organisation ne
        // peut ni s'approprier ni invalider la chaîne d'une autre.
        let a = chain_a();
        let b = OrgChain::new(OrgId::new(Uuid::from_u128(0xB)));
        let entries = build(&a, 4);
        assert_eq!(a.verify(&entries), Ok(()));
        assert_eq!(
            b.verify(&entries),
            Err(ChainBreak {
                position: 0,
                kind: ChainBreakKind::AlteredEntry,
            })
        );
    }

    #[test]
    fn hash_depends_on_prev() {
        let chain = chain_a();
        let c = content(0);
        let h_genesis = chain.entry_hash(None, &c);
        let other = chain.entry_hash(Some(&h_genesis), &c);
        assert_ne!(h_genesis, other);
    }

    #[test]
    fn tail_truncation_is_invisible_to_verify_but_caught_with_anchor() {
        // LIMITE DOCUMENTÉE : une chaîne amputée de sa queue reste
        // intérieurement cohérente — verify seul l'accepte. C'est
        // l'ancre de confiance qui la détecte.
        let chain = chain_a();
        let entries = build(&chain, 6);
        let anchor = entries.last().map(|e| e.hash);
        let truncated = &entries[..4];
        assert_eq!(chain.verify(truncated), Ok(()), "limite assumée de verify");
        assert_eq!(
            chain.verify_with_tail(truncated, anchor.as_ref()),
            Err(ChainBreak {
                position: 4,
                kind: ChainBreakKind::TailMismatch,
            })
        );
        // Avec la bonne ancre et la chaîne entière : valide.
        assert_eq!(chain.verify_with_tail(&entries, anchor.as_ref()), Ok(()));
    }

    #[test]
    fn full_suffix_rewrite_is_caught_by_anchor_only() {
        // Un attaquant en écriture altère l'entrée 2 puis recalcule toutes
        // les empreintes suivantes : la chaîne forgée est intérieurement
        // cohérente. Seule l'ancre externe la trahit.
        let chain = chain_a();
        let entries = build(&chain, 5);
        let anchor = entries.last().map(|e| e.hash);

        let mut forged: Vec<ChainedEntry> = entries[..2].to_vec();
        let mut altered = content(2);
        altered.actor = "attaquant".to_owned();
        let tail = forged.last().map(|e| e.hash);
        forged.push(chain.append(tail.as_ref(), altered));
        for i in 3..5 {
            let tail = forged.last().map(|e| e.hash);
            forged.push(chain.append(tail.as_ref(), content(i)));
        }

        assert_eq!(chain.verify(&forged), Ok(()), "limite assumée de verify");
        assert_eq!(
            chain.verify_with_tail(&forged, anchor.as_ref()),
            Err(ChainBreak {
                position: 5,
                kind: ChainBreakKind::TailMismatch,
            })
        );
    }

    #[test]
    fn verify_with_tail_on_empty_chain() {
        let chain = chain_a();
        assert_eq!(chain.verify_with_tail(&[], None), Ok(()));
        let phantom = chain.entry_hash(None, &content(0));
        assert_eq!(
            chain.verify_with_tail(&[], Some(&phantom)),
            Err(ChainBreak {
                position: 0,
                kind: ChainBreakKind::TailMismatch,
            })
        );
    }
}
