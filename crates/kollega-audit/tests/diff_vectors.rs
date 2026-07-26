//! Générateur de vecteurs pour le test différentiel Rust ↔ Python.
//!
//! La référence indépendante (`tools/reference/canonical.py`, écrite depuis
//! la spécification `docs/encodage-canonique.md` sans regarder ce code) doit
//! produire les mêmes octets que l'implémentation Rust. Ce test génère les
//! vecteurs ; la comparaison octet à octet est faite par la CI (`diff`),
//! afin que ce test ne dépende pas d'un interpréteur Python.
//!
//! Sauté sans `DIFF_VECTORS_DIR` (exécuté en CI, où Python est disponible).
//! Écrit dans ce répertoire :
//! - `vectors.jsonl` : une valeur taguée par ligne (forme JSON de transport
//!   lue par `canonical.py`) ; `rust.out` : l'encodage canonique Rust ;
//! - `hashes.jsonl` : un enregistrement complet par ligne (prev, action,
//!   actor, height, org_id, payload, ts) ; `rust-hashes.out` : l'empreinte
//!   SHA-256 hexadécimale calculée par `OrgChain::entry_hash`.
//!
//! Le générateur est DÉTERMINISTE (SplitMix64, graine figée) : deux
//! exécutions produisent les mêmes vecteurs, une divergence est rejouable.
//! Toute divergence Rust/Python est d'abord un défaut de SPÉCIFICATION à
//! documenter dans `docs/encodage-canonique.md`, jamais une correction
//! silencieuse (règle de `tools/reference/README.md`).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use kollega_audit::{CanonicalValue, EntryContent, Hash32, OrgChain};
use kollega_core::OrgId;
use uuid::Uuid;

/// Nombre de vecteurs d'encodage générés (hors préambule fixe).
const ENCODE_VECTORS: usize = 12_000;
/// Nombre d'empreintes complètes générées.
const HASH_VECTORS: usize = 2_000;

/// PRNG déterministe minimal (SplitMix64) — aucune dépendance, rejouable.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Caractères sur-représentés : séparateurs du format, échappements,
/// contrôles, invisibles Unicode, multi-octets — là où une divergence
/// d'échappement ou de tri d'octets se verrait.
const ALPHABET: &[char] = &[
    'a', 'b', 'z', '0', ' ', '"', '\\', '\n', '\r', '\t', '\u{0}', '\u{1}', '\u{1f}', '\u{7f}',
    '{', '}', '[', ']', ':', ',', 'u', 'é', '€', 'Ω', '𝄞', '中', '\u{200b}', '\u{202e}',
    '\u{feff}', '\u{ad}',
];

fn gen_text(rng: &mut SplitMix64) -> String {
    let len = rng.below(12);
    let mut s = String::new();
    for _ in 0..len {
        if rng.below(8) == 0 {
            // Point de code arbitraire ; from_u32 refuse les surrogates.
            if let Some(c) = char::from_u32((rng.next() % 0x0011_0000) as u32) {
                s.push(c);
            }
        } else {
            let idx = rng.below(ALPHABET.len() as u64) as usize;
            s.push(ALPHABET[idx]);
        }
    }
    s
}

fn gen_int(rng: &mut SplitMix64) -> i64 {
    match rng.below(6) {
        0 => i64::MIN,
        1 => i64::MAX,
        2 => 0,
        3 => -1,
        _ => rng.next() as i64,
    }
}

fn gen_value(rng: &mut SplitMix64, depth: u32) -> CanonicalValue {
    let pick = if depth == 0 {
        rng.below(4)
    } else {
        rng.below(6)
    };
    match pick {
        0 => CanonicalValue::Null,
        1 => CanonicalValue::Bool(rng.next() & 1 == 0),
        2 => CanonicalValue::Int(gen_int(rng)),
        3 => CanonicalValue::Text(gen_text(rng)),
        4 => {
            let n = rng.below(5);
            CanonicalValue::Array((0..n).map(|_| gen_value(rng, depth - 1)).collect())
        }
        _ => {
            let n = rng.below(5);
            let mut map = BTreeMap::new();
            for _ in 0..n {
                map.insert(gen_text(rng), gen_value(rng, depth - 1));
            }
            CanonicalValue::Object(map)
        }
    }
}

/// Cas figés : les pièges historiques de l'injectivité et du tri d'octets.
fn fixed_cases() -> Vec<CanonicalValue> {
    let text = |s: &str| CanonicalValue::Text(s.to_owned());
    let obj = |pairs: &[(&str, CanonicalValue)]| {
        CanonicalValue::Object(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect(),
        )
    };
    vec![
        text(""),
        text("clé avec \" et ,"),
        text("antislash avant guillemet \\\""),
        text("l1\nl2\rl3\tfin"),
        text("x\u{0}y\u{1f}z\u{7f}"),
        text("bidi \u{202e} zwsp \u{200b} bom \u{feff}"),
        CanonicalValue::Int(i64::MIN),
        CanonicalValue::Int(i64::MAX),
        CanonicalValue::Array(vec![]),
        CanonicalValue::Object(BTreeMap::new()),
        // Tri par OCTETS UTF-8 : 'é' (0xC3A9) trie APRÈS 'z' (0x7A).
        obj(&[("é", CanonicalValue::Int(1)), ("z", CanonicalValue::Int(2))]),
        obj(&[
            ("", CanonicalValue::Null),
            ("\"", CanonicalValue::Bool(true)),
        ]),
        obj(&[(
            "a",
            CanonicalValue::Array(vec![obj(&[(
                "b",
                CanonicalValue::Array(vec![CanonicalValue::Array(vec![])]),
            )])]),
        )]),
        CanonicalValue::Array(vec![
            text("],"),
            text(",["),
            text("\":"),
            CanonicalValue::Null,
        ]),
    ]
}

/// Forme JSON de transport lue par `canonical.py` (`_from_json`).
fn transport(value: &CanonicalValue) -> serde_json::Value {
    use serde_json::json;
    match value {
        CanonicalValue::Null => json!(["null"]),
        CanonicalValue::Bool(b) => json!(["bool", b]),
        CanonicalValue::Int(i) => json!(["int", i]),
        CanonicalValue::Text(t) => json!(["text", t]),
        CanonicalValue::Array(items) => {
            json!(["array", items.iter().map(transport).collect::<Vec<_>>()])
        }
        CanonicalValue::Object(map) => {
            let obj: serde_json::Map<String, serde_json::Value> =
                map.iter().map(|(k, v)| (k.clone(), transport(v))).collect();
            json!(["object", obj])
        }
    }
}

#[test]
fn emit_differential_vectors() {
    let Ok(dir) = std::env::var("DIFF_VECTORS_DIR") else {
        eprintln!(
            "IGNORÉ : DIFF_VECTORS_DIR absent — le générateur de vecteurs \
             différentiels n'écrit qu'en CI (comparaison Python ensuite)."
        );
        return;
    };
    let dir = std::path::Path::new(&dir);
    std::fs::create_dir_all(dir).expect("création du répertoire de vecteurs");

    // ---- Vecteurs d'encodage : préambule figé + génération déterministe.
    let mut rng = SplitMix64(0xC0FF_EE00_2026_0728);
    let mut values = fixed_cases();
    values.extend((0..ENCODE_VECTORS).map(|_| gen_value(&mut rng, 3)));

    let mut vectors = String::new();
    let mut rust_out = String::new();
    for value in &values {
        let line = serde_json::json!({ "value": transport(value) });
        writeln!(vectors, "{line}").expect("écriture en mémoire");
        rust_out.push_str(&value.encode());
        rust_out.push('\n');
    }
    std::fs::write(dir.join("vectors.jsonl"), vectors).expect("vectors.jsonl");
    std::fs::write(dir.join("rust.out"), rust_out).expect("rust.out");

    // ---- Empreintes complètes : prev × contenu × hauteur × horodatage.
    let mut hashes = String::new();
    let mut rust_hashes = String::new();
    for _ in 0..HASH_VECTORS {
        let org_id = OrgId::new(Uuid::from_u128(
            (u128::from(rng.next()) << 64) | u128::from(rng.next()),
        ));
        let chain = OrgChain::new(org_id);
        let prev = if rng.below(4) == 0 {
            None
        } else {
            let mut bytes = [0u8; 32];
            for chunk in bytes.chunks_mut(8) {
                chunk.copy_from_slice(&rng.next().to_le_bytes());
            }
            Some(Hash32(bytes))
        };
        let height = match rng.below(4) {
            0 => 0,
            1 => u64::MAX,
            _ => rng.next(),
        };
        let content = EntryContent {
            actor: gen_text(&mut rng),
            action: gen_text(&mut rng),
            payload: gen_value(&mut rng, 2),
            timestamp_micros: gen_int(&mut rng),
        };
        let hash = chain.entry_hash(height, prev.as_ref(), &content);

        let line = serde_json::json!({
            "prev": prev.as_ref().map(kollega_audit::Hash32::to_hex),
            "action": content.action,
            "actor": content.actor,
            "height": height,
            "org_id": org_id.to_string(),
            "payload": transport(&content.payload),
            "ts": content.timestamp_micros,
        });
        writeln!(hashes, "{line}").expect("écriture en mémoire");
        rust_hashes.push_str(&hash.to_hex());
        rust_hashes.push('\n');
    }
    std::fs::write(dir.join("hashes.jsonl"), hashes).expect("hashes.jsonl");
    std::fs::write(dir.join("rust-hashes.out"), rust_hashes).expect("rust-hashes.out");
}
