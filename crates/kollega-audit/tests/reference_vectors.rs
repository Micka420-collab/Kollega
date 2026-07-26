//! VECTEURS DE RÉFÉRENCE — la protection la plus précieuse de la crate.
//!
//! Chaque vecteur fige (entrée connue → empreinte connue). Si l'encodage
//! canonique, l'ordre des champs, l'échappement ou la formule de hachage
//! changent — même « anodinement », même par une montée de version — ces
//! tests échouent bruyamment. Ne JAMAIS mettre à jour une empreinte attendue
//! sans comprendre pourquoi elle a changé : un changement ici invalide
//! toutes les chaînes d'audit déjà écrites en production.
//!
//! Le vecteur 1 a été recoupé indépendamment (SHA-256 calculé hors Rust sur
//! les octets attendus, voir docs/rapport-nuit-2026-07-27.md).

use std::collections::BTreeMap;

use kollega_audit::{CanonicalValue, EntryContent, OrgChain};
use kollega_core::OrgId;
use uuid::Uuid;

fn org(n: u128) -> OrgChain {
    OrgChain::new(OrgId::new(Uuid::from_u128(n)))
}

fn obj(pairs: Vec<(&str, CanonicalValue)>) -> CanonicalValue {
    CanonicalValue::Object(
        pairs
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect::<BTreeMap<_, _>>(),
    )
}

/// Vecteur 1 — le cas minimal, recoupé hors Rust.
/// Enregistrement canonique attendu :
/// `{"action":"task_started","actor":"system","org_id":"00000000-0000-0000-0000-000000000001","payload":{}}`
/// Octets hachés : 32 octets à zéro (genèse), puis cet enregistrement, puis
/// `0` (horodatage).
fn vector_1() -> (OrgChain, EntryContent) {
    (
        org(1),
        EntryContent {
            actor: "system".to_owned(),
            action: "task_started".to_owned(),
            payload: obj(vec![]),
            timestamp_micros: 0,
        },
    )
}

/// Vecteur 2 — échappements : guillemets, antislash, saut de ligne, non-ASCII.
fn vector_2() -> (OrgChain, EntryContent) {
    (
        org(1),
        EntryContent {
            actor: "agent \"alpha\"\\beta".to_owned(),
            action: "tool_call_intended".to_owned(),
            payload: obj(vec![
                ("chemin", CanonicalValue::from("C:\\dossier\nligne")),
                ("texte", CanonicalValue::from("été — 12 €\tfin")),
            ]),
            timestamp_micros: 1_753_500_000_000_000,
        },
    )
}

/// Vecteur 3 — structures imbriquées, entiers extrêmes, horodatage négatif.
fn vector_3() -> (OrgChain, EntryContent) {
    (
        org(1),
        EntryContent {
            actor: "system".to_owned(),
            action: "cost_recorded".to_owned(),
            payload: obj(vec![
                (
                    "couts",
                    CanonicalValue::Array(vec![
                        CanonicalValue::Int(1),
                        CanonicalValue::Int(-2),
                        CanonicalValue::Int(i64::MAX),
                    ]),
                ),
                ("detail", obj(vec![("vide", CanonicalValue::Null)])),
                ("ok", CanonicalValue::Bool(true)),
            ]),
            timestamp_micros: -1,
        },
    )
}

/// Vecteur 4 — entrée chaînée : prev = empreinte du vecteur 1.
fn vector_4() -> (OrgChain, EntryContent) {
    (
        org(1),
        EntryContent {
            actor: "system".to_owned(),
            action: "task_finished".to_owned(),
            payload: obj(vec![("statut", CanonicalValue::from("succeeded"))]),
            timestamp_micros: 1,
        },
    )
}

/// Vecteur 5 — même contenu que le vecteur 1, autre organisation :
/// l'empreinte DOIT différer (l'organisation est dans les octets hachés).
fn vector_5() -> (OrgChain, EntryContent) {
    let (_, content) = vector_1();
    (org(2), content)
}

#[test]
fn reference_vectors_are_stable() {
    // V1 recoupé indépendamment : SHA-256 (hors Rust) de 32 octets à zéro,
    // puis `{"action":"task_started","actor":"system","org_id":"00000000-0000-0000-0000-000000000001","payload":{}}0`.
    let (c1, e1) = vector_1();
    assert_eq!(
        c1.entry_hash(None, &e1).to_hex(),
        "babb08aa2d5ad4c5ac04a9ca04c4689498b1a4b6720df16d82d7a95e2a54f63e"
    );

    let (c2, e2) = vector_2();
    assert_eq!(
        c2.entry_hash(None, &e2).to_hex(),
        "af5f97ec9655dfcbbfdf53d42f4fa3fddef0746ef9f685758242685baf1d9566"
    );

    let (c3, e3) = vector_3();
    assert_eq!(
        c3.entry_hash(None, &e3).to_hex(),
        "9bb264f94f7ee8e8898cbd170831623fe39b971795b2902c0f6dfc845fd0c906"
    );

    let (c4, e4) = vector_4();
    let prev = c1.entry_hash(None, &e1);
    assert_eq!(
        c4.entry_hash(Some(&prev), &e4).to_hex(),
        "595779347b75ee24fa502b4bdd73f16ba2a239a2668ea64544ae5a165d20f03c"
    );

    let (c5, e5) = vector_5();
    let h5 = c5.entry_hash(None, &e5).to_hex();
    assert_eq!(
        h5,
        "7aa752b95a3d2ef45d368bcb3d9889a5ea664507cf727746eeb448f1922113ec"
    );
    assert_ne!(h5, c1.entry_hash(None, &e1).to_hex());
}
