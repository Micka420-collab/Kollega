//! Journal d'audit chaîné par hachage — partie **pure** (invariant 4).
//!
//! Cette crate ne contient que le calcul et la vérification de chaîne :
//! aucun accès à PostgreSQL (le sink arrive à un jalon ultérieur, dans la
//! couche de persistance). Tout est vérifiable par `cargo test` seul.
//!
//! Définition figée :
//! `hash = SHA-256(prev_hash || payload_canonique || horodatage)` où
//! - `prev_hash` : les 32 octets de l'empreinte précédente, ou rien pour la
//!   première entrée de la chaîne ;
//! - `payload_canonique` : l'encodage canonique (voir [`canonical`]) de
//!   l'enregistrement complet — action, actor, org_id, payload — dans cet
//!   ordre, figé. Inclure l'acteur, l'action et l'organisation dans les
//!   octets hachés protège l'intégralité de l'enregistrement, pas seulement
//!   sa charge utile ;
//! - `horodatage` : microsecondes depuis l'époque Unix (i64), en décimal
//!   ASCII. Ce choix suit la précision de `timestamptz` (PostgreSQL,
//!   microseconde) : l'empreinte survivra à l'aller-retour en base.
//!
//! L'absence d'ambiguïté de la concaténation est garantie par construction :
//! `prev_hash` est de longueur fixe (0 ou 32 octets), et l'encodage canonique
//! se termine par `}` alors que l'horodatage ne contient que `-` et des
//! chiffres — aucune paire (payload, horodatage) différente ne produit les
//! mêmes octets.
//!
//! La chaîne est **par organisation** : [`chain::OrgChain`] porte l'`OrgId`,
//! les entrées n'en portent pas — il est donc impossible, par construction,
//! de mélanger deux organisations dans une même chaîne, et une entrée hachée
//! pour l'organisation A ne se vérifie pas dans la chaîne de B.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod canonical;
pub mod chain;

pub use canonical::CanonicalValue;
pub use chain::{ChainBreak, ChainBreakKind, ChainedEntry, EntryContent, Hash32, OrgChain};
