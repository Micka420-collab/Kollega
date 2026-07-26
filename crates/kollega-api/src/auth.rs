//! Hachage et vérification de mots de passe — argon2id.
//!
//! PARAMÈTRES, explicites et figés (recommandation OWASP pour argon2id,
//! profil « faible mémoire » adapté à un serveur mutualisé modeste) :
//! mémoire 19 456 KiB (19 Mio), 2 itérations, parallélisme 1, empreinte de
//! 32 octets. Ordre de grandeur : ~30-80 ms par hachage sur un vCPU
//! contemporain — assez lent pour gêner la force brute, assez rapide pour
//! une connexion interactive.
//!
//! FORMAT DE STOCKAGE : chaîne PHC complète, par exemple
//! `$argon2id$v=19$m=19456,t=2,p=1$<sel base64>$<empreinte base64>`,
//! destinée à une colonne TEXT (`users.password_hash`, jalon suivant).
//! Le sel (16 octets aléatoires, OS RNG) et les paramètres vivent dans la
//! chaîne : la vérification relit les paramètres depuis la chaîne stockée,
//! ce qui permettra de durcir les paramètres sans invalider l'existant.
//!
//! RÈGLE ABSOLUE : aucun mot de passe ni fragment de mot de passe ne sort de
//! ces fonctions — ni dans les erreurs, ni dans les journaux. Les erreurs
//! amont d'argon2 sont volontairement écrasées en variantes muettes.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};

/// Erreurs de hachage/vérification. Volontairement muettes : jamais le mot
/// de passe, jamais le détail amont.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PasswordError {
    /// Le hachage a échoué (paramètres ou moteur).
    #[error("échec du hachage du mot de passe")]
    Hash,
    /// L'empreinte stockée n'est pas une chaîne PHC valide.
    #[error("empreinte de mot de passe stockée invalide ou corrompue")]
    InvalidStoredHash,
}

/// Mémoire en KiB (19 Mio), itérations, parallélisme — voir doc de module.
const MEMORY_KIB: u32 = 19_456;
const ITERATIONS: u32 = 2;
const PARALLELISM: u32 = 1;
const OUTPUT_LEN: usize = 32;

fn hasher() -> Result<Argon2<'static>, PasswordError> {
    let params = Params::new(MEMORY_KIB, ITERATIONS, PARALLELISM, Some(OUTPUT_LEN))
        .map_err(|_| PasswordError::Hash)?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

/// Hache un mot de passe avec un sel aléatoire frais.
///
/// Retourne la chaîne PHC complète à stocker. Deux appels avec le même mot
/// de passe produisent deux chaînes différentes (sel aléatoire).
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = hasher()?
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| PasswordError::Hash)?;
    Ok(hash.to_string())
}

/// Vérifie un mot de passe contre une chaîne PHC stockée.
///
/// `Ok(true)` : correspond. `Ok(false)` : ne correspond pas. `Err(…)` : la
/// chaîne stockée est illisible — à traiter comme une anomalie, pas comme un
/// simple mauvais mot de passe.
pub fn verify_password(password: &str, stored_hash: &str) -> Result<bool, PasswordError> {
    let parsed = PasswordHash::new(stored_hash).map_err(|_| PasswordError::InvalidStoredHash)?;
    // Une chaîne PHC sans empreinte (sel seul) est structurellement valide
    // mais inutilisable comme crédential stocké : c'est une corruption, pas
    // une non-correspondance.
    if parsed.hash.is_none() {
        return Err(PasswordError::InvalidStoredHash);
    }
    match hasher()?.verify_password(password.as_bytes(), &parsed) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(_) => Err(PasswordError::InvalidStoredHash),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE : aucun test n'imprime ni ne journalise un mot de passe. Les
    // valeurs utilisées ici sont des constantes de test, pas des secrets.

    #[test]
    fn round_trip_accepts_correct_password() {
        let stored = hash_password("grande phrase de passe 42!").unwrap();
        assert_eq!(
            verify_password("grande phrase de passe 42!", &stored),
            Ok(true)
        );
    }

    #[test]
    fn wrong_password_is_rejected_without_error() {
        let stored = hash_password("bon mot de passe").unwrap();
        assert_eq!(verify_password("mauvais mot de passe", &stored), Ok(false));
    }

    #[test]
    fn stored_format_is_argon2id_phc_with_fixed_params() {
        let stored = hash_password("x").unwrap();
        assert!(
            stored.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"),
            "format PHC inattendu : {stored}"
        );
    }

    #[test]
    fn same_password_gives_different_hashes() {
        // Sel aléatoire : deux hachages du même mot de passe diffèrent.
        let a = hash_password("identique").unwrap();
        let b = hash_password("identique").unwrap();
        assert_ne!(a, b);
        assert_eq!(verify_password("identique", &a), Ok(true));
        assert_eq!(verify_password("identique", &b), Ok(true));
    }

    #[test]
    fn corrupt_stored_hash_is_an_error_not_a_mismatch() {
        for corrupt in [
            "",
            "pas-un-phc",
            "$argon2id$v=19$m=19456,t=2,p=1$tronque",
            "$inconnu$v=19$abc",
        ] {
            assert_eq!(
                verify_password("peu importe", corrupt),
                Err(PasswordError::InvalidStoredHash),
                "entrée corrompue acceptée : {corrupt:?}"
            );
        }
    }

    #[test]
    fn errors_never_echo_the_password() {
        let secret = "SecretUnique#2026";
        let err = verify_password(secret, "corrompu").unwrap_err();
        let shown = format!("{err} / {err:?}");
        assert!(!shown.contains(secret));
        assert!(!shown.to_lowercase().contains("secretunique"));
    }
}
