//! Horodatage du domaine — la MICROSECONDE, par construction (bloc 3b).
//!
//! L'horodatage participe aux octets hachés de la chaîne d'audit, et
//! PostgreSQL (`timestamptz`) ne connaît que la microseconde. Tant que la
//! troncature était une discipline (« pense à tronquer avant de hacher »),
//! l'écart entre ce qui est haché et ce qui fait l'aller-retour restait
//! EXPRIMABLE — donc un jour, exprimé. [`Timestamp`] tronque à la
//! construction, quelle que soit la voie d'entrée : il n'existe pas de
//! valeur de ce type plus précise que la microseconde.
//!
//! Le domaine ne lit JAMAIS l'horloge (pas de `now()` ici) : la périphérie
//! horodate, le domaine transporte.

/// Microsecondes Unix, signées — la précision de `timestamptz`, exactement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Depuis des microsecondes Unix : déjà à la précision du type.
    #[must_use]
    pub const fn from_unix_micros(micros: i64) -> Self {
        Timestamp(micros)
    }

    /// Depuis des nanosecondes Unix : TRONQUÉES à la microseconde
    /// (division euclidienne — vers le bas, y compris avant 1970, pour que
    /// deux chemins d'entrée ne puissent pas différer d'une microseconde
    /// autour d'une frontière).
    #[must_use]
    pub const fn from_unix_nanos(nanos: i128) -> Self {
        let micros = nanos.div_euclid(1_000);
        // Saturation aux bornes i64 : un horodatage hors bornes est un bug
        // d'horloge, pas une raison de paniquer le domaine.
        if micros > i64::MAX as i128 {
            Timestamp(i64::MAX)
        } else if micros < i64::MIN as i128 {
            Timestamp(i64::MIN)
        } else {
            Timestamp(micros as i64)
        }
    }

    /// Les microsecondes Unix — la seule sortie, sans perte possible.
    #[must_use]
    pub const fn as_micros(&self) -> i64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_to_the_microsecond_from_every_entry_path() {
        // Nanosecondes : tronquées vers le bas.
        assert_eq!(Timestamp::from_unix_nanos(1_999).as_micros(), 1);
        assert_eq!(Timestamp::from_unix_nanos(2_000).as_micros(), 2);
        // Avant 1970 : division euclidienne, toujours vers le bas —
        // -1 ns est DANS la microseconde -1, pas dans la microseconde 0.
        assert_eq!(Timestamp::from_unix_nanos(-1).as_micros(), -1);
        assert_eq!(Timestamp::from_unix_nanos(-1_000).as_micros(), -1);
        assert_eq!(Timestamp::from_unix_nanos(-1_001).as_micros(), -2);
        // Microsecondes : identité.
        assert_eq!(Timestamp::from_unix_micros(42).as_micros(), 42);
    }

    #[test]
    fn out_of_range_clocks_saturate_instead_of_panicking() {
        assert_eq!(Timestamp::from_unix_nanos(i128::MAX).as_micros(), i64::MAX);
        assert_eq!(Timestamp::from_unix_nanos(i128::MIN).as_micros(), i64::MIN);
    }

    #[test]
    fn round_trip_is_lossless_at_type_precision() {
        // Ce qui est haché (as_micros) et ce qui fait l'aller-retour
        // (from_unix_micros) sont le MÊME nombre : l'écart est inexprimable.
        for micros in [i64::MIN, -1, 0, 1, 1_753_651_200_000_000, i64::MAX] {
            assert_eq!(
                Timestamp::from_unix_micros(Timestamp::from_unix_micros(micros).as_micros())
                    .as_micros(),
                micros
            );
        }
    }
}
