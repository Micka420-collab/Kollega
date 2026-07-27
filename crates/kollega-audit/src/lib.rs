//! Journal d'audit chaîné par hachage — partie **pure** (invariant 4).
//!
//! Cette crate ne contient que le calcul et la vérification de chaîne :
//! aucun accès à PostgreSQL (le sink arrive à un jalon ultérieur, dans la
//! couche de persistance). Tout est vérifiable par `cargo test` seul.
//!
//! Définition figée :
//! `hash = SHA-256(prev_hash || payload_canonique || horodatage)` où
//! - `prev_hash` : les 32 octets de l'empreinte précédente, ou **32 octets à
//!   zéro** pour la première entrée de la chaîne — le préfixe est ainsi de
//!   longueur réellement fixe ;
//! - `payload_canonique` : l'encodage canonique (voir [`canonical`] et la
//!   spécification `docs/encodage-canonique.md`) de l'enregistrement complet
//!   — action, actor, height, org_id, payload — dans cet ordre, figé.
//!   Inclure l'acteur, l'action, la hauteur et l'organisation dans les
//!   octets hachés protège l'intégralité de l'enregistrement et sa position :
//!   une entrée déplacée ou rejouée ailleurs invalide la chaîne ;
//! - `horodatage` : microsecondes depuis l'époque Unix (i64), en décimal
//!   ASCII. Ce choix suit la précision de `timestamptz` (PostgreSQL,
//!   microseconde) : l'empreinte survivra à l'aller-retour en base.
//!
//! L'absence d'ambiguïté de la concaténation est garantie par construction :
//! le préfixe fait toujours exactement 32 octets, l'encodage canonique se
//! termine par `}` alors que l'horodatage ne contient que `-` et des
//! chiffres — aucune paire (payload, horodatage) différente ne produit les
//! mêmes octets.
//!
//! La chaîne est **par organisation** : [`chain::OrgChain`] porte l'`OrgId`,
//! les entrées n'en portent pas — il est donc impossible, par construction,
//! de mélanger deux organisations dans une même chaîne, et une entrée hachée
//! pour l'organisation A ne se vérifie pas dans la chaîne de B.
//!
//! ## Modèle de menace — à lire honnêtement
//!
//! Un hachage chaîné SANS clé ni signature garantit la **cohérence interne**
//! d'une suite d'entrées. [`chain::OrgChain::verify`] détecte : altération
//! d'une entrée, réordonnancement, suppression au milieu, troncature de
//! tête. Il ne détecte PAS, seul : la **troncature de queue** (une chaîne
//! amputée de ses dernières entrées reste cohérente) ni la **réécriture
//! complète d'un suffixe** par un attaquant capable d'écrire le stockage
//! (altérer l'entrée i puis recalculer tous les hachages suivants). Ces deux
//! classes ne sont détectables qu'en comparant l'empreinte de queue à une
//! **ancre de confiance** conservée hors d'atteinte de l'attaquant :
//! c'est [`chain::OrgChain::verify_with_tail`]. Où vivra l'ancre (copie
//! hors site, journal d'exploitation, courrier au client) est une décision
//! du jalon de persistance — `audit verify` devra l'utiliser.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod anchor;
pub mod canonical;
pub mod chain;
pub mod content;
pub mod records;
pub mod repository;

pub use anchor::{Anchor, AnchorPublisher, AnchorViolation, InMemoryAnchorPublisher};
pub use canonical::CanonicalValue;
pub use chain::{ChainBreak, ChainBreakKind, ChainedEntry, EntryContent, Hash32, OrgChain};
