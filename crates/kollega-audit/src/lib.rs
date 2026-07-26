//! Journal d'audit en ajout seul, chaîné par hachage (jalon M1).
//!
//! Vide au jalon M0 : seule la place dans le graphe de dépendances est posée.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    #[test]
    fn crate_compiles() {}
}
