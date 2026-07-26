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
//! VÉRIFICATION (ADR-0006) : avec les paramètres STOCKÉS dans la chaîne,
//! bornés par un plancher (contre le cassé) et un plafond (contre le déni de
//! service mémoire), jamais par une liste blanche de profils ; un profil
//! stocké différent du courant est accepté et signalé pour re-hachage à la
//! connexion — le parc migre seul, personne n'est verrouillé.
//!
//! RÈGLE ABSOLUE : aucun mot de passe ni fragment de mot de passe ne sort de
//! ces fonctions — ni dans les erreurs, ni dans les journaux. Les erreurs
//! amont d'argon2 sont volontairement écrasées en variantes muettes.

use std::sync::{Condvar, Mutex};

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
    // Un hachage alloue la même mémoire qu'une vérification : même borne.
    let _permit = GATE.acquire();
    let salt = SaltString::generate(&mut OsRng);
    let hash = hasher()?
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| PasswordError::Hash)?;
    Ok(hash.to_string())
}

/// Bornes des paramètres acceptés à la VÉRIFICATION (ADR-0006).
///
/// La liste blanche stricte de profils, essayée d'abord, était un défaut :
/// elle ne ferme aucun chemin réel (un attaquant capable d'écrire la colonne
/// y met un argon2id conforme d'un mot de passe qu'il connaît) et son coût
/// était certain (durcir les paramètres sans penser à la liste = tout le
/// parc verrouillé). Forme correcte : vérifier avec les paramètres STOCKÉS —
/// le format PHC est auto-descriptif, c'est son intérêt — bornés par :
/// - un PLANCHER, contre le réellement cassé (m < 8 Mio) ;
/// - un PLAFOND, contre le déni de service (une chaîne forgée m=4 Gio ferait
///   allouer 4 Gio à chaque tentative de connexion) — seul argument valable
///   pour contraindre les paramètres d'une chaîne stockée.
const MIN_MEMORY_KIB: u32 = 8_192; // 8 Mio : en dessous, c'est cassé.
/// 64 Mio (amendement ADR-0006 du 28/07) : défend contre une empreinte
/// stockée EMPOISONNÉE — ~3,4× le profil courant, marge de durcissement
/// réelle sans laisser à une chaîne forgée de quoi coûter cher. Le plafond
/// seul ne défend PAS contre le VOLUME (deux cents tentatives simultanées à
/// 19 Mio conformes font ~3,8 Gio) : c'est le rôle du sémaphore [`GATE`].
/// Les deux, pas l'un ou l'autre.
const MAX_MEMORY_KIB: u32 = 65_536;
const MAX_ITERATIONS: u32 = 64; // garde CPU, même logique que la mémoire.
const MAX_PARALLELISM: u32 = 16;

/// Nombre maximal d'opérations argon2 SIMULTANÉES (hachage + vérification).
///
/// Lien avec la mémoire disponible, documenté (amendement ADR-0006) : pire
/// cas légitime = 4 × 64 Mio (chaînes stockées au plafond) = 256 Mio ; cas
/// courant = 4 × 19 Mio ≈ 76 Mio — dimensionné pour le « serveur mutualisé
/// modeste » du profil de tête de module, où l'empreinte mémoire totale
/// doit rester sous quelques centaines de Mio. Au-delà de la borne, les
/// appels ATTENDENT leur tour, ils n'échouent pas : une pointe de
/// connexions légitimes se sérialise (quelques dizaines de ms chacune), un
/// déluge hostile ne fait plus tomber le serveur — et la file borne le
/// débit de force brute au passage.
const MAX_CONCURRENT_ARGON2: u32 = 4;

/// Sémaphore compteur minimal (Mutex + Condvar), sans dépendance ni async :
/// la borne vit au plus près des fonctions qu'elle protège.
struct Gate {
    available: Mutex<u32>,
    freed: Condvar,
}

/// Permis pris sur [`Gate`] — rendu automatiquement, même en cas de panique
/// pendant l'opération protégée (le `Drop` s'exécute au déroulement).
struct GatePermit<'a> {
    gate: &'a Gate,
}

impl Gate {
    const fn new(permits: u32) -> Self {
        Gate {
            available: Mutex::new(permits),
            freed: Condvar::new(),
        }
    }

    /// Attend qu'un permis soit libre, puis le prend. Bloquant, jamais une
    /// erreur : c'est le contrat (« les vérifications attendent, elles
    /// n'échouent pas »). Un mutex empoisonné est récupéré tel quel — le
    /// compteur reste cohérent, le `Drop` du permis ayant rendu le sien
    /// pendant le déroulement de la panique.
    fn acquire(&self) -> GatePermit<'_> {
        let mut available = self
            .available
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while *available == 0 {
            available = self
                .freed
                .wait(available)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        *available -= 1;
        GatePermit { gate: self }
    }
}

impl Drop for GatePermit<'_> {
    fn drop(&mut self) {
        let mut available = self
            .gate
            .available
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *available += 1;
        self.gate.freed.notify_one();
    }
}

static GATE: Gate = Gate::new(MAX_CONCURRENT_ARGON2);

/// Issue d'une vérification de mot de passe réussie à parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordCheck {
    /// Mot de passe correct, empreinte au profil courant.
    Valid,
    /// Mot de passe correct, mais l'empreinte n'est pas au profil courant :
    /// re-hacher maintenant (le mot de passe en clair est disponible) et
    /// remplacer la colonne — le parc migre tout seul à la connexion.
    ValidNeedsRehash,
    /// Mot de passe incorrect.
    Invalid,
}

/// Vérifie un mot de passe contre une chaîne PHC stockée (ADR-0006).
///
/// Accepte tout argon2id v19 dont les paramètres stockés sont entre plancher
/// et plafond ; signale [`PasswordCheck::ValidNeedsRehash`] quand le profil
/// stocké diffère du profil courant. `Err(…)` : chaîne illisible, algorithme
/// étranger (`argon2i`/`argon2d` — jamais produits par nous), paramètres
/// cassés ou absurdes — une anomalie à traiter, pas un mauvais mot de passe.
///
/// Le contrôle des bornes précède toute vérification : une chaîne forgée
/// m=4 Gio est refusée SANS que la mémoire demandée soit allouée.
pub fn check_password(password: &str, stored_hash: &str) -> Result<PasswordCheck, PasswordError> {
    let parsed = PasswordHash::new(stored_hash).map_err(|_| PasswordError::InvalidStoredHash)?;
    // Une chaîne PHC sans empreinte (sel seul) est structurellement valide
    // mais inutilisable comme crédential stocké : c'est une corruption, pas
    // une non-correspondance.
    if parsed.hash.is_none() {
        return Err(PasswordError::InvalidStoredHash);
    }
    // Cohérence de format : nous n'avons jamais produit autre chose que de
    // l'argon2id v19 — toute autre valeur est une anomalie de données.
    if parsed.algorithm.as_str() != "argon2id" {
        return Err(PasswordError::InvalidStoredHash);
    }
    if parsed.version != Some(Version::V0x13.into()) {
        return Err(PasswordError::InvalidStoredHash);
    }
    let params = Params::try_from(&parsed).map_err(|_| PasswordError::InvalidStoredHash)?;
    if params.m_cost() < MIN_MEMORY_KIB
        || params.m_cost() > MAX_MEMORY_KIB
        || params.t_cost() == 0
        || params.t_cost() > MAX_ITERATIONS
        || params.p_cost() == 0
        || params.p_cost() > MAX_PARALLELISM
    {
        return Err(PasswordError::InvalidStoredHash);
    }
    // Le permis n'est pris qu'ICI, après les bornes : une chaîne forgée
    // absurde est refusée sans consommer de place dans la file — seul le
    // travail mémoire/CPU réel est contingenté.
    let _permit = GATE.acquire();
    match hasher()?.verify_password(password.as_bytes(), &parsed) {
        Ok(()) => {
            let current = (MEMORY_KIB, ITERATIONS, PARALLELISM);
            let stored = (params.m_cost(), params.t_cost(), params.p_cost());
            if stored == current {
                Ok(PasswordCheck::Valid)
            } else {
                Ok(PasswordCheck::ValidNeedsRehash)
            }
        }
        Err(argon2::password_hash::Error::Password) => Ok(PasswordCheck::Invalid),
        Err(_) => Err(PasswordError::InvalidStoredHash),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE : aucun test n'imprime ni ne journalise un mot de passe. Les
    // valeurs utilisées ici sont des constantes de test, pas des secrets.

    /// Chaîne PHC réelle produite avec un profil arbitraire (pour les tests
    /// de bornes et de re-hachage).
    fn phc_with(algorithm: Algorithm, m: u32, t: u32, p: u32, password: &str) -> String {
        let salt = SaltString::generate(&mut OsRng);
        let params = Params::new(m, t, p, Some(OUTPUT_LEN)).unwrap();
        Argon2::new(algorithm, Version::V0x13, params)
            .hash_password(password.as_bytes(), &salt)
            .unwrap()
            .to_string()
    }

    #[test]
    fn round_trip_accepts_correct_password() {
        let stored = hash_password("grande phrase de passe 42!").unwrap();
        assert_eq!(
            check_password("grande phrase de passe 42!", &stored),
            Ok(PasswordCheck::Valid)
        );
    }

    #[test]
    fn wrong_password_is_invalid_without_error() {
        let stored = hash_password("bon mot de passe").unwrap();
        assert_eq!(
            check_password("mauvais mot de passe", &stored),
            Ok(PasswordCheck::Invalid)
        );
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
        assert_eq!(check_password("identique", &a), Ok(PasswordCheck::Valid));
        assert_eq!(check_password("identique", &b), Ok(PasswordCheck::Valid));
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
                check_password("peu importe", corrupt),
                Err(PasswordError::InvalidStoredHash),
                "entrée corrompue acceptée : {corrupt:?}"
            );
        }
    }

    #[test]
    fn foreign_algorithm_is_an_anomaly() {
        // Nous n'avons jamais produit d'argon2i : une telle chaîne dans la
        // colonne est une anomalie de données, pas un compte légitime.
        let phc = phc_with(Algorithm::Argon2i, MEMORY_KIB, ITERATIONS, PARALLELISM, "x");
        assert!(phc.starts_with("$argon2i$"));
        assert_eq!(
            check_password("x", &phc),
            Err(PasswordError::InvalidStoredHash)
        );
    }

    #[test]
    fn legacy_weak_profile_above_floor_is_accepted_and_flagged_for_rehash() {
        // ADR-0006 : un ancien profil plus faible que la politique courante
        // mais au-dessus du plancher reste accepté — et signalé pour
        // re-hachage, afin que le parc migre seul à la connexion.
        let phc = phc_with(Algorithm::Argon2id, 12_288, 1, 1, "ancien compte");
        assert_eq!(
            check_password("ancien compte", &phc),
            Ok(PasswordCheck::ValidNeedsRehash)
        );
        // Un mauvais mot de passe sur ce même profil reste Invalid, jamais
        // « à re-hacher ».
        assert_eq!(check_password("mauvais", &phc), Ok(PasswordCheck::Invalid));
    }

    #[test]
    fn below_floor_is_rejected() {
        // m=8 KiB : réellement cassé, sous le plancher documenté.
        let phc = phc_with(Algorithm::Argon2id, 8, 1, 1, "x");
        assert_eq!(
            check_password("x", &phc),
            Err(PasswordError::InvalidStoredHash)
        );
    }

    #[test]
    fn ceiling_is_64_mib_exactly() {
        // À la borne exacte (64 Mio) : accepté — et signalé pour re-hachage
        // puisque ce n'est pas le profil courant.
        let at_max = phc_with(Algorithm::Argon2id, MAX_MEMORY_KIB, 1, 1, "x");
        assert_eq!(
            check_password("x", &at_max),
            Ok(PasswordCheck::ValidNeedsRehash)
        );
        // 128 Mio : légal sous l'ancien plafond (256 Mio), refusé depuis
        // l'amendement ADR-0006 — une empreinte stockée empoisonnée ne peut
        // plus coûter que 64 Mio par tentative.
        let stored = hash_password("x").unwrap();
        let forged = stored.replace("m=19456", "m=131072");
        assert!(forged.contains("m=131072"));
        assert_eq!(
            check_password("x", &forged),
            Err(PasswordError::InvalidStoredHash)
        );
    }

    #[test]
    fn beyond_the_gate_verifications_wait_they_do_not_fail() {
        use std::sync::mpsc;
        use std::time::Duration;

        // Tous les permis sont pris à la main : la vérification suivante
        // doit ATTENDRE (pas d'erreur, pas de refus), puis aboutir dès
        // qu'un permis se libère.
        //
        // Le profil est VOLONTAIREMENT rapide (plancher, t=1 : quelques ms)
        // pour que seul le blocage de la porte puisse expliquer l'attente —
        // avec le profil courant (~centaines de ms sur une machine chargée),
        // ce test passait AUSSI avec une porte cassée : la lenteur d'argon2
        // masquait l'absence de blocage. Attrapé par sabotage volontaire.
        let stored = phc_with(
            Algorithm::Argon2id,
            MIN_MEMORY_KIB,
            1,
            1,
            "attente disciplinée",
        );
        let permits: Vec<_> = (0..MAX_CONCURRENT_ARGON2).map(|_| GATE.acquire()).collect();

        let (sender, receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let result = check_password("attente disciplinée", &stored);
            sender.send(()).expect("canal de test");
            result
        });

        // Porte saturée : la vérification (quelques ms une fois libérée) ne
        // doit PAS aboutir pendant une attente de 500 ms.
        assert!(
            receiver.recv_timeout(Duration::from_millis(500)).is_err(),
            "la vérification aurait dû attendre, la porte étant saturée"
        );

        drop(permits);
        // Porte libérée : elle aboutit, correctement, sans erreur.
        assert!(
            receiver.recv_timeout(Duration::from_secs(10)).is_ok(),
            "la vérification aurait dû aboutir après libération"
        );
        assert_eq!(
            handle.join().expect("jointure du fil de test"),
            Ok(PasswordCheck::ValidNeedsRehash)
        );
    }

    #[test]
    fn saturation_never_turns_into_failure() {
        // 3× la borne en parallèle : tout le monde finit, tout le monde a
        // la bonne réponse — la contention se paie en attente, jamais en
        // erreur ni en faux « invalide ».
        let stored = hash_password("charge de pointe").unwrap();
        let handles: Vec<_> = (0..MAX_CONCURRENT_ARGON2 * 3)
            .map(|i| {
                let stored = stored.clone();
                std::thread::spawn(move || {
                    if i % 2 == 0 {
                        (check_password("charge de pointe", &stored), true)
                    } else {
                        (check_password("mauvais mot de passe", &stored), false)
                    }
                })
            })
            .collect();
        for handle in handles {
            let (result, expected_valid) = handle.join().expect("jointure");
            let expected = if expected_valid {
                PasswordCheck::Valid
            } else {
                PasswordCheck::Invalid
            };
            assert_eq!(result, Ok(expected));
        }
    }

    #[test]
    fn absurd_memory_is_rejected_without_allocating() {
        // Chaîne forgée m=4 Gio : refusée par la borne AVANT toute
        // vérification — le test échouerait par le temps et la mémoire si
        // l'allocation avait lieu.
        let stored = hash_password("x").unwrap();
        let forged = stored.replace("m=19456", "m=4194304");
        assert!(forged.contains("m=4194304"));
        let started = std::time::Instant::now();
        assert_eq!(
            check_password("x", &forged),
            Err(PasswordError::InvalidStoredHash)
        );
        assert!(
            started.elapsed() < std::time::Duration::from_millis(200),
            "le refus doit précéder toute allocation"
        );
    }

    #[test]
    fn rehash_produces_current_profile() {
        // Le re-hachage recommandé par ValidNeedsRehash produit bien le
        // profil courant : une fois re-haché, plus de signalement.
        let rehashed = hash_password("ancien compte").unwrap();
        assert_eq!(
            check_password("ancien compte", &rehashed),
            Ok(PasswordCheck::Valid)
        );
    }

    #[test]
    fn errors_never_echo_the_password() {
        let secret = "SecretUnique#2026";
        let err = check_password(secret, "corrompu").unwrap_err();
        let shown = format!("{err} / {err:?}");
        assert!(!shown.contains(secret));
        assert!(!shown.to_lowercase().contains("secretunique"));
    }
}
