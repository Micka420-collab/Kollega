//! Identifiants typés du domaine.
//!
//! Quatre newtypes distincts sur [`uuid::Uuid`] : le compilateur interdit de
//! passer l'identifiant d'une organisation là où celui d'un utilisateur est
//! attendu. Aucune conversion implicite n'existe entre eux — c'est la
//! garantie, vérifiée par le test de non-compilation ci-dessous.
//!
//! ```compile_fail
//! use kollega_core::{OrgId, UserId};
//! fn expects_org(_: OrgId) {}
//! let user = UserId::new(uuid::Uuid::from_u128(1));
//! expects_org(user); // erreur de type : les identifiants ne sont pas interchangeables
//! ```

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! typed_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Construit l'identifiant à partir d'un UUID existant.
            #[must_use]
            pub const fn new(id: Uuid) -> Self {
                Self(id)
            }

            /// UUID sous-jacent (pour le stockage et les liaisons SQL).
            #[must_use]
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

typed_id!(
    /// Identifiant d'une organisation (tenant). Alimente `app.current_org`.
    OrgId
);
typed_id!(
    /// Identifiant d'un utilisateur.
    UserId
);
typed_id!(
    /// Identifiant d'une tâche.
    TaskId
);
typed_id!(
    /// Identifiant d'un appel d'outil.
    ToolCallId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_serialize_as_bare_uuid_strings() {
        // Forme stable : l'UUID nu, prêt pour une colonne UUID.
        let org = OrgId::new(Uuid::from_u128(7));
        assert_eq!(
            serde_json::to_value(org).unwrap(),
            serde_json::json!("00000000-0000-0000-0000-000000000007")
        );
        let back: OrgId =
            serde_json::from_value(serde_json::json!("00000000-0000-0000-0000-000000000007"))
                .unwrap();
        assert_eq!(back, org);
    }

    #[test]
    fn ids_display_as_uuid() {
        let task = TaskId::new(Uuid::from_u128(0xAB));
        assert_eq!(task.to_string(), "00000000-0000-0000-0000-0000000000ab");
    }

    #[test]
    fn same_uuid_different_types_are_still_distinct_types() {
        // À l'exécution, deux identifiants de types différents peuvent porter
        // le même UUID : c'est le système de types qui interdit la confusion
        // (voir le doctest compile_fail du module), pas la valeur.
        let raw = Uuid::from_u128(42);
        let user = UserId::new(raw);
        let tool_call = ToolCallId::new(raw);
        assert_eq!(user.as_uuid(), tool_call.as_uuid());
    }
}
