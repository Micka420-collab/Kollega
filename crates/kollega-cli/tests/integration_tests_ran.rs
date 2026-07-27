//! Garde : en intégration continue, aucun test d'intégration ne se saute.
//!
//! Sept fichiers de test exigent une base PostgreSQL réelle et se sautent
//! poliment quand `TEST_MIGRATE_DATABASE_URL` est absente — c'est ce qui
//! permet de travailler sans base en local. Mais cette politesse est aussi
//! le plus beau trou du dispositif de preuve : la variable n'est fournie
//! que par UNE ligne de `.github/workflows/ci.yml`. La retirer — un
//! refactor du fichier, un job dupliqué sans son `env:` — rendrait sept
//! tests VERTS en ne prouvant plus rien, et neuf invariants sur treize
//! reposent sur eux. Un vert de complaisance est pire qu'un rouge : il
//! s'affiche comme une preuve.
//!
//! D'où deux gardes complémentaires :
//! 1. en intégration continue, l'absence de la variable est un ÉCHEC, pas
//!    un saut — donc les tests ont réellement tourné ;
//! 2. tout test qui se conditionne le fait par une variable que la garde 1
//!    couvre — sans quoi un futur test se sauterait à jamais, sous une
//!    variable que personne ne surveille.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

/// Variables par lesquelles un test a le droit de se conditionner.
///
/// - `TEST_MIGRATE_DATABASE_URL` : couverte par la garde 1 ci-dessous.
/// - `DIFF_VECTORS_DIR` : le générateur de vecteurs du différentiel
///   Rust ↔ Python. Son exécution est déjà prouvée AILLEURS, par l'étape
///   de CI elle-même : sans vecteurs, `canonical.py` n'a pas de fichier à
///   lire, et l'étape vérifie en plus que le compte dépasse 12 000. Se
///   sauter ne peut donc pas passer pour un succès.
/// - `CI` : ce n'est pas une condition de saut, c'est le détecteur
///   d'intégration continue de la garde 1 — qui se soumet ainsi à sa
///   propre règle plutôt que de s'exempter.
const GATE_VARIABLES: &[&str] = &["TEST_MIGRATE_DATABASE_URL", "DIFF_VECTORS_DIR", "CI"];

/// Motif recherché, assemblé à la compilation pour que le texte cherché
/// n'apparaisse PAS littéralement ici.
///
/// Écrit en clair, il se trouverait lui-même — la garde signalait alors sa
/// propre ligne de recherche comme une variable inconnue. Seule la
/// définition du motif disparaît ainsi du balayage : les vrais appels de ce
/// fichier (`CI`, `TEST_MIGRATE_DATABASE_URL`) restent examinés comme ceux
/// des autres, la garde ne s'exempte pas.
const LOOKUP: &str = concat!("env", "::var");

fn crates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/ doit exister")
        .to_path_buf()
}

/// Tous les fichiers `.rs` sous `crates/*/tests/`.
fn test_sources() -> Vec<PathBuf> {
    fn collect(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(crates_dir())
        .expect("lecture de crates/")
        .flatten()
    {
        collect(&entry.path().join("tests"), &mut out);
    }
    out
}

/// En intégration continue, se sauter est INTERDIT.
///
/// Hors CI la garde ne dit rien : elle n'a rien à vérifier, un poste sans
/// PostgreSQL est un cas de travail légitime. C'est en CI que le saut
/// devient un mensonge, parce que c'est là que le vert est lu comme une
/// preuve.
#[test]
fn in_ci_no_integration_test_may_skip() {
    if std::env::var_os("CI").is_none() {
        eprintln!(
            "hors intégration continue : rien à vérifier ici (les tests de \
             base se sautent légitimement sans PostgreSQL)."
        );
        return;
    }
    let url = std::env::var("TEST_MIGRATE_DATABASE_URL").unwrap_or_default();
    assert!(
        !url.trim().is_empty(),
        "TEST_MIGRATE_DATABASE_URL absente ALORS QU'ON EST EN INTÉGRATION \
         CONTINUE : les sept tests exigeant une base réelle viennent de se \
         sauter en silence, et la CI s'apprête à afficher un vert qui ne \
         prouve rien — isolation RLS, tranche verticale, concurrence du \
         crédit, absence de suppression physique, validateur de séquence, \
         filet structurel RLS, commande audit verify. Rétablir le bloc \
         `env:` du job qui exécute `cargo test` dans .github/workflows/ci.yml."
    );
}

/// Tout saut passe par une variable que la garde ci-dessus couvre.
///
/// Sans cette seconde garde, la première serait contournable par accident :
/// un futur test conditionné à `TEST_DB` ou `POSTGRES_URL` se sauterait
/// éternellement en CI sans que rien ne l'annonce.
#[test]
fn every_test_gate_uses_a_variable_the_ci_guard_covers() {
    let mut sites = 0_usize;
    let mut offenders: BTreeSet<String> = BTreeSet::new();

    for file in test_sources() {
        let content = fs::read_to_string(&file).expect("lecture d'un fichier de test");
        let mut rest = content.as_str();
        while let Some(at) = rest.find(LOOKUP) {
            rest = &rest[at + LOOKUP.len()..];
            // Le nom suit immédiatement — `("NOM")` ou `_os("NOM")`. Au-delà
            // d'une poignée d'octets, ce n'est plus l'argument de cet appel
            // mais une chaîne sans rapport : on ne conclut rien.
            let Some(open) = rest.find('"').filter(|at| *at < 8) else {
                continue;
            };
            let Some(len) = rest[open + 1..].find('"') else {
                break;
            };
            let name = &rest[open + 1..open + 1 + len];
            sites += 1;
            if !GATE_VARIABLES.contains(&name) {
                offenders.insert(format!("{} lit {name}", file.display()));
            }
            rest = &rest[open + 1 + len..];
        }
    }

    assert!(
        offenders.is_empty(),
        "un test se conditionne à une variable NON couverte par la garde \
         « in_ci_no_integration_test_may_skip » : il se sauterait en \
         silence, en CI comme ailleurs. Ajouter la variable à \
         GATE_VARIABLES *et* à la garde, ou gater sur \
         TEST_MIGRATE_DATABASE_URL. Coupables : {offenders:?}"
    );
    assert!(
        sites >= 9,
        "balayage suspect : seulement {sites} conditions trouvées, alors \
         que les tests d'intégration en comptent au moins neuf — le \
         parcours des sources est cassé, et cette garde ne garde plus rien"
    );
}
