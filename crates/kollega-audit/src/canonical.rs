//! Encodage canonique — écrit à la main, jamais délégué à `serde_json`.
//!
//! POURQUOI À LA MAIN : l'empreinte de chaque entrée d'audit est calculée sur
//! ces octets. L'ordre des clés et le formatage de `serde_json` ne sont pas
//! contractuellement stables entre versions : une simple montée de version
//! pourrait invalider toutes les chaînes existantes, silencieusement. Le
//! format est donc défini ici, FIGÉ, et verrouillé par des vecteurs de
//! référence dans les tests (`tests/reference_vectors.rs`).
//!
//! FORMAT (syntaxe proche de JSON, définie indépendamment) :
//! - `Null` → `null` ; `Bool` → `true` / `false` ;
//! - `Int` → décimal ASCII signé (`i64`). Pas de flottant, par construction :
//!   l'argent est un entier, et un flottant n'a pas de forme canonique sûre ;
//! - `Text` → `"…"` avec échappement : `\` → `\\`, `"` → `\"`, puis `\n`,
//!   `\r`, `\t`, et tout autre caractère de contrôle U+0000..U+001F en
//!   `\u00XX` (hexadécimal minuscule, quatre chiffres). Tout le reste est
//!   émis verbatim en UTF-8 — pas d'échappement du non-ASCII ;
//! - `Array` → `[v1,v2]`, sans aucune espace ;
//! - `Object` → `{"k1":v1,"k2":v2}`, sans aucune espace, clés échappées comme
//!   `Text` et triées par ordre lexicographique d'octets UTF-8 (l'ordre
//!   naturel de `BTreeMap<String, _>`).

use std::collections::BTreeMap;

/// Valeur canonique d'une charge utile d'audit.
///
/// Volontairement restreinte : pas de flottant, pas de valeur non ordonnable.
/// Tout ce qui entre dans le journal d'audit passe par ce type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalValue {
    /// Absence de valeur.
    Null,
    /// Booléen.
    Bool(bool),
    /// Entier signé 64 bits. L'argent, les compteurs, les durées.
    Int(i64),
    /// Texte UTF-8.
    Text(String),
    /// Suite ordonnée de valeurs.
    Array(Vec<CanonicalValue>),
    /// Table clé → valeur ; les clés sont uniques et triées (BTreeMap).
    Object(BTreeMap<String, CanonicalValue>),
}

impl CanonicalValue {
    /// Encode la valeur dans `out`, selon le format figé du module.
    pub fn encode_into(&self, out: &mut String) {
        match self {
            CanonicalValue::Null => out.push_str("null"),
            CanonicalValue::Bool(true) => out.push_str("true"),
            CanonicalValue::Bool(false) => out.push_str("false"),
            CanonicalValue::Int(i) => {
                // `i64` en décimal : la mise en forme des entiers de la
                // bibliothèque standard est stable et sans variante locale.
                out.push_str(&i.to_string());
            }
            CanonicalValue::Text(t) => encode_text(t, out),
            CanonicalValue::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.encode_into(out);
                }
                out.push(']');
            }
            CanonicalValue::Object(map) => {
                out.push('{');
                for (i, (key, value)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    encode_text(key, out);
                    out.push(':');
                    value.encode_into(out);
                }
                out.push('}');
            }
        }
    }

    /// Encodage canonique complet, en chaîne.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut out = String::new();
        self.encode_into(&mut out);
        out
    }
}

/// Échappe et délimite un texte selon le format figé.
pub(crate) fn encode_text(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

impl From<i64> for CanonicalValue {
    fn from(value: i64) -> Self {
        CanonicalValue::Int(value)
    }
}

impl From<bool> for CanonicalValue {
    fn from(value: bool) -> Self {
        CanonicalValue::Bool(value)
    }
}

impl From<&str> for CanonicalValue {
    fn from(value: &str) -> Self {
        CanonicalValue::Text(value.to_owned())
    }
}

impl From<String> for CanonicalValue {
    fn from(value: String) -> Self {
        CanonicalValue::Text(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(pairs: &[(&str, CanonicalValue)]) -> CanonicalValue {
        CanonicalValue::Object(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect(),
        )
    }

    #[test]
    fn scalars_encode_exactly() {
        assert_eq!(CanonicalValue::Null.encode(), "null");
        assert_eq!(CanonicalValue::Bool(true).encode(), "true");
        assert_eq!(CanonicalValue::Bool(false).encode(), "false");
        assert_eq!(CanonicalValue::Int(0).encode(), "0");
        assert_eq!(CanonicalValue::Int(-42).encode(), "-42");
        assert_eq!(
            CanonicalValue::Int(i64::MAX).encode(),
            "9223372036854775807"
        );
        assert_eq!(
            CanonicalValue::Int(i64::MIN).encode(),
            "-9223372036854775808"
        );
    }

    #[test]
    fn text_escaping_is_fixed() {
        assert_eq!(CanonicalValue::from("simple").encode(), r#""simple""#);
        assert_eq!(CanonicalValue::from("a\"b\\c").encode(), r#""a\"b\\c""#);
        assert_eq!(
            CanonicalValue::from("l1\nl2\rl3\tfin").encode(),
            r#""l1\nl2\rl3\tfin""#
        );
        // Contrôle hors \n \r \t : \u00xx, hex minuscule, quatre chiffres.
        assert_eq!(
            CanonicalValue::from("x\u{0}y\u{1f}z").encode(),
            "\"x\\u0000y\\u001fz\""
        );
        // Non-ASCII verbatim : jamais d'échappement unicode du lisible.
        assert_eq!(
            CanonicalValue::from("cout : 12 € — été").encode(),
            "\"cout : 12 € — été\""
        );
    }

    #[test]
    fn object_keys_are_sorted_bytewise() {
        let value = obj(&[
            ("zeta", CanonicalValue::Int(1)),
            ("alpha", CanonicalValue::Int(2)),
            ("mike", CanonicalValue::Int(3)),
        ]);
        assert_eq!(value.encode(), r#"{"alpha":2,"mike":3,"zeta":1}"#);
    }

    #[test]
    fn nested_structures_encode_without_spaces() {
        let value = obj(&[
            (
                "items",
                CanonicalValue::Array(vec![
                    CanonicalValue::Int(1),
                    CanonicalValue::Null,
                    CanonicalValue::from("deux"),
                ]),
            ),
            ("ok", CanonicalValue::Bool(true)),
        ]);
        assert_eq!(value.encode(), r#"{"items":[1,null,"deux"],"ok":true}"#);
    }

    #[test]
    fn empty_containers() {
        assert_eq!(CanonicalValue::Array(vec![]).encode(), "[]");
        assert_eq!(CanonicalValue::Object(BTreeMap::new()).encode(), "{}");
    }
}
