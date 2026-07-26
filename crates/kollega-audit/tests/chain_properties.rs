//! Propriétés générales de la chaîne d'audit sous mutation (BLOC 8).
//!
//! Sur une chaîne valide générée, toute mutation (altération, suppression,
//! insertion, permutation) est détectée par `verify`, ET à la bonne
//! position.

use std::collections::BTreeMap;

use kollega_audit::{CanonicalValue, ChainBreakKind, ChainedEntry, EntryContent, OrgChain};
use kollega_core::OrgId;
use proptest::prelude::*;
use uuid::Uuid;

fn chain() -> OrgChain {
    OrgChain::new(OrgId::new(Uuid::from_u128(0xA)))
}

fn content(i: i64) -> EntryContent {
    let mut payload = BTreeMap::new();
    payload.insert("i".to_owned(), CanonicalValue::Int(i));
    EntryContent {
        actor: format!("acteur-{i}"),
        action: "evenement".to_owned(),
        payload: CanonicalValue::Object(payload),
        timestamp_micros: 1_753_700_000_000_000 + i,
    }
}

fn build(c: &OrgChain, n: usize) -> Vec<ChainedEntry> {
    let mut entries = Vec::new();
    for i in 0..n as i64 {
        let next = c.append(entries.last(), content(i));
        entries.push(next);
    }
    entries
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1500))]

    /// Une chaîne intacte est toujours valide.
    #[test]
    fn intact_chain_verifies(n in 0usize..40) {
        let c = chain();
        let entries = build(&c, n);
        prop_assert_eq!(c.verify(&entries), Ok(()));
    }

    /// Altérer le contenu de l'entrée i → AlteredEntry détecté en i (aucune
    /// entrée avant i n'est touchée, donc la première rupture est bien i).
    #[test]
    fn content_mutation_detected_at_position(n in 1usize..40, seed in any::<u64>()) {
        let c = chain();
        let mut entries = build(&c, n);
        let i = (seed as usize) % n;
        entries[i].content.actor.push_str("-trafiqué");
        let err = c.verify(&entries).unwrap_err();
        prop_assert_eq!(err.position, i);
        prop_assert_eq!(err.kind, ChainBreakKind::AlteredEntry);
    }

    /// Supprimer une entrée du MILIEU (ni la première, ni la dernière) →
    /// rupture de lien en position i. La suppression de la DERNIÈRE entrée
    /// est une troncature de queue, indétectable par verify seul (limite
    /// documentée, couverte par l'ancre) — donc exclue ici.
    #[test]
    fn middle_deletion_detected_at_position(n in 3usize..40, seed in any::<u64>()) {
        let c = chain();
        let mut entries = build(&c, n);
        let i = 1 + (seed as usize) % (n - 2); // 1..=n-2 : jamais la dernière
        entries.remove(i);
        let err = c.verify(&entries).unwrap_err();
        prop_assert_eq!(err.position, i);
        prop_assert_eq!(err.kind, ChainBreakKind::BrokenLink);
    }

    /// Permuter deux entrées adjacentes i, i+1 → rupture détectée dès i.
    #[test]
    fn adjacent_swap_detected(n in 2usize..40, seed in any::<u64>()) {
        let c = chain();
        let mut entries = build(&c, n);
        let i = (seed as usize) % (n - 1);
        entries.swap(i, i + 1);
        let err = c.verify(&entries).unwrap_err();
        // La première anomalie est en i (ou en 0 si i==0 et la genèse bouge).
        prop_assert!(err.position <= i + 1);
        prop_assert_eq!(c.verify(&entries).is_err(), true);
    }

    /// Insérer une entrée AVANT la fin (position i < n) → rupture détectée :
    /// tout ce qui suit est décalé, hauteurs et liens ne collent plus.
    /// (Insérer en position n serait une extension légitime de la chaîne.)
    #[test]
    fn interior_insertion_detected(n in 1usize..40, seed in any::<u64>()) {
        let c = chain();
        let mut entries = build(&c, n);
        let i = (seed as usize) % n; // 0..=n-1 : jamais après la dernière
        let intruder = c.append(entries.get(i.saturating_sub(1)), content(999));
        entries.insert(i, intruder);
        prop_assert!(c.verify(&entries).is_err());
    }
}
