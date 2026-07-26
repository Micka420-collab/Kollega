//! Injectivité de l'encodeur canonique (BLOC 7).
//!
//! Propriété de sécurité : si deux valeurs distinctes produisaient le même
//! encodage, la chaîne d'audit serait forgeable (deux entrées différentes,
//! même empreinte). On le prouve de deux façons complémentaires :
//!
//! 1. **Round-trip** — un décodeur INDÉPENDANT (écrit ici depuis la
//!    spécification `docs/encodage-canonique.md`, jamais depuis le code de
//!    l'encodeur) reconstruit la valeur : `parse(encode(v)) == v`. Un inverse
//!    à gauche existe donc, ce qui prouve l'injectivité de `encode`.
//! 2. **Chasse au séparateur** — la faille classique de ce type d'encodeur :
//!    deux structures dont le contenu contient le séparateur produisent les
//!    mêmes octets. Tests déterministes ciblés + proptest biaisé vers les
//!    caractères dangereux (`"`, `\`, `,`, `:`, `{`, `}`, `[`, `]`).
//!
//! Le décodeur ci-dessous n'existe QUE pour ce test ; le domaine n'a pas de
//! décodeur en production (une entrée d'audit ne se « dé-hache » pas).

use std::collections::BTreeMap;

use kollega_audit::CanonicalValue;
use proptest::prelude::*;

// --- Décodeur indépendant, écrit depuis la spécification -------------------

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(s: &'a str) -> Self {
        Parser {
            bytes: s.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek();
        if b.is_some() {
            self.pos += 1;
        }
        b
    }

    fn expect(&mut self, b: u8) {
        assert_eq!(
            self.bump(),
            Some(b),
            "octet attendu {} à {}",
            b as char,
            self.pos
        );
    }

    fn starts_with(&self, s: &str) -> bool {
        self.bytes[self.pos..].starts_with(s.as_bytes())
    }

    fn value(&mut self) -> CanonicalValue {
        match self.peek() {
            Some(b'n') => {
                assert!(self.starts_with("null"));
                self.pos += 4;
                CanonicalValue::Null
            }
            Some(b't') => {
                assert!(self.starts_with("true"));
                self.pos += 4;
                CanonicalValue::Bool(true)
            }
            Some(b'f') => {
                assert!(self.starts_with("false"));
                self.pos += 5;
                CanonicalValue::Bool(false)
            }
            Some(b'"') => CanonicalValue::Text(self.text()),
            Some(b'[') => self.array(),
            Some(b'{') => self.object(),
            _ => CanonicalValue::Int(self.int()),
        }
    }

    fn text(&mut self) -> String {
        self.expect(b'"');
        let mut out = String::new();
        loop {
            match self.bump().expect("texte non terminé") {
                b'"' => break,
                b'\\' => match self.bump().expect("échappement tronqué") {
                    b'\\' => out.push('\\'),
                    b'"' => out.push('"'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let hex: String = (0..4)
                            .map(|_| self.bump().expect("u sans 4 chiffres") as char)
                            .collect();
                        let code = u32::from_str_radix(&hex, 16).expect("hex invalide");
                        out.push(char::from_u32(code).expect("scalaire invalide"));
                    }
                    other => panic!("échappement inconnu \\{}", other as char),
                },
                // Octet d'un caractère UTF-8 verbatim : on relit le caractère
                // complet depuis la position de départ.
                _ => {
                    let start = self.pos - 1;
                    // Avance jusqu'à la fin du caractère UTF-8 courant.
                    while self.peek().is_some_and(|b| (b & 0xC0) == 0x80) {
                        self.pos += 1;
                    }
                    let s = std::str::from_utf8(&self.bytes[start..self.pos]).unwrap();
                    out.push_str(s);
                }
            }
        }
        out
    }

    fn int(&mut self) -> i64 {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.pos += 1;
        }
        std::str::from_utf8(&self.bytes[start..self.pos])
            .unwrap()
            .parse()
            .expect("entier invalide")
    }

    fn array(&mut self) -> CanonicalValue {
        self.expect(b'[');
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return CanonicalValue::Array(items);
        }
        loop {
            items.push(self.value());
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => break,
                other => panic!("tableau : séparateur inattendu {other:?}"),
            }
        }
        CanonicalValue::Array(items)
    }

    fn object(&mut self) -> CanonicalValue {
        self.expect(b'{');
        let mut map = BTreeMap::new();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return CanonicalValue::Object(map);
        }
        loop {
            let key = self.text();
            self.expect(b':');
            let value = self.value();
            map.insert(key, value);
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => break,
                other => panic!("objet : séparateur inattendu {other:?}"),
            }
        }
        CanonicalValue::Object(map)
    }
}

fn parse(s: &str) -> CanonicalValue {
    let mut p = Parser::new(s);
    let v = p.value();
    assert_eq!(p.pos, s.len(), "reste non consommé");
    v
}

// --- Stratégie proptest, biaisée vers les caractères dangereux -------------

fn dangerous_string() -> impl Strategy<Value = String> {
    // Alphabet volontairement chargé en séparateurs et échappements.
    proptest::collection::vec(
        prop_oneof![
            Just('"'),
            Just('\\'),
            Just(','),
            Just(':'),
            Just('{'),
            Just('}'),
            Just('['),
            Just(']'),
            Just('\n'),
            Just('\t'),
            Just('\u{0}'),
            Just('\u{1f}'),
            Just('é'),
            Just('€'),
            Just('a'),
            Just('0'),
        ],
        0..12,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

fn canonical_value() -> impl Strategy<Value = CanonicalValue> {
    let leaf = prop_oneof![
        Just(CanonicalValue::Null),
        any::<bool>().prop_map(CanonicalValue::Bool),
        any::<i64>().prop_map(CanonicalValue::Int),
        dangerous_string().prop_map(CanonicalValue::Text),
    ];
    leaf.prop_recursive(4, 32, 6, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..5).prop_map(CanonicalValue::Array),
            proptest::collection::vec((dangerous_string(), inner), 0..5)
                .prop_map(|pairs| CanonicalValue::Object(pairs.into_iter().collect())),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    /// Injectivité par round-trip : parse(encode(v)) == v pour toute valeur.
    /// Un inverse à gauche prouve que encode ne collisionne jamais.
    #[test]
    fn encode_is_injective_via_round_trip(v in canonical_value()) {
        let encoded = v.encode();
        let decoded = parse(&encoded);
        prop_assert_eq!(decoded, v);
    }

    /// Deux valeurs distinctes → encodages distincts (formulation directe).
    #[test]
    fn distinct_values_distinct_encodings(a in canonical_value(), b in canonical_value()) {
        if a != b {
            prop_assert_ne!(a.encode(), b.encode());
        }
    }
}

// --- Chasse déterministe au séparateur -------------------------------------

fn obj(pairs: &[(&str, CanonicalValue)]) -> CanonicalValue {
    CanonicalValue::Object(
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    )
}

#[test]
fn key_containing_quote_and_separator_does_not_collide() {
    // Le piège : une clé qui contient `":` et `,"` pourrait, sans
    // échappement, imiter une frontière de champ.
    let sneaky = obj(&[("a\":1,\"b", CanonicalValue::Int(0))]);
    let honest = obj(&[("a", CanonicalValue::Int(1)), ("b", CanonicalValue::Int(0))]);
    assert_ne!(sneaky.encode(), honest.encode());
    // Et chacun se relit en lui-même.
    assert_eq!(parse(&sneaky.encode()), sneaky);
    assert_eq!(parse(&honest.encode()), honest);
}

#[test]
fn text_imitating_container_does_not_collide_with_container() {
    let as_text = CanonicalValue::from("[1,2]");
    let as_array = CanonicalValue::Array(vec![CanonicalValue::Int(1), CanonicalValue::Int(2)]);
    assert_ne!(as_text.encode(), as_array.encode());

    let null_text = CanonicalValue::from("null");
    assert_ne!(null_text.encode(), CanonicalValue::Null.encode());

    let int_text = CanonicalValue::from("42");
    assert_ne!(int_text.encode(), CanonicalValue::Int(42).encode());
}

#[test]
fn backslash_before_quote_is_unambiguous() {
    // "\\" suivi de '"' : sans échappement correct, la frontière du texte
    // serait fausse.
    let tricky = CanonicalValue::from("fin\\");
    let round = parse(&tricky.encode());
    assert_eq!(round, tricky);
    let tricky2 = CanonicalValue::from("a\\\"b");
    assert_eq!(parse(&tricky2.encode()), tricky2);
}

#[test]
fn empty_containers_and_nesting_round_trip() {
    let nested = CanonicalValue::Array(vec![
        CanonicalValue::Array(vec![]),
        obj(&[]),
        obj(&[("k", CanonicalValue::Array(vec![CanonicalValue::Null]))]),
    ]);
    assert_eq!(parse(&nested.encode()), nested);
}
