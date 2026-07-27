//! Garde de forme des dépôts d'audit (bloc 3f).
//!
//! Le dépôt de CHAÎNE ne doit jamais gagner une méthode de retrait : ce
//! test lit la définition du trait `AuditChainRepository` et échoue si un
//! identifiant de suppression y apparaît. La purge n'existe que sur le
//! dépôt de CONTENU, sous son nom propre (`purge_org`).

use std::fs;
use std::path::PathBuf;

#[test]
fn the_chain_repository_cannot_express_removal() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("kollega-audit/src/repository.rs");
    let source =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("lecture de {} : {e}", path.display()));

    let chain_trait = source
        .split("pub trait AuditChainRepository")
        .nth(1)
        .and_then(|after| after.split("pub trait").next())
        .expect("le trait AuditChainRepository doit exister");

    for forbidden in [
        "delete", "remove", "purge", "truncate", "drop", "update", "rewrite", "clear",
    ] {
        assert!(
            !chain_trait.to_ascii_lowercase().contains(forbidden),
            "le dépôt de chaîne exprime un retrait : « {forbidden} » — l'ajout seul est \
             sa surface, pas sa discipline"
        );
    }

    // La liste de mots interdits ci-dessus est une liste NOIRE, donc en
    // retard d'un synonyme : `expunge`, `retract`, `forget` la
    // franchissaient sans effort. Ce qui suit la remplace par une liste
    // BLANCHE — le trait a EXACTEMENT ces deux méthodes, et toute
    // troisième, quel que soit son nom, fait rougir. On ne peut pas
    // énumérer les façons de dire « supprimer » ; on peut énumérer les
    // deux façons permises de toucher au journal.
    let mut methods: Vec<String> = Vec::new();
    let mut rest = chain_trait;
    while let Some(at) = rest.find("async fn ") {
        let after = &rest["async fn ".len() + at..];
        rest = after;
        methods.push(after.chars().take_while(|c| *c != '(').collect());
    }
    assert_eq!(
        methods,
        vec!["append".to_owned(), "read".to_owned()],
        "la surface du dépôt de chaîne a changé. Deux méthodes, `append` et \
         `read` : retirer une preuve ne doit pas être EXPRIMABLE, pas \
         seulement déconseillé."
    );
}

/// Le trait ne suffit pas si le pilote peut écrire du SQL à côté : la
/// chaîne d'audit ne doit subir AUCUN retrait ni AUCUNE réécriture, où que
/// ce soit dans la persistance. Les GRANT le refusent déjà côté base ; ce
/// test le refuse côté source, où l'intention se lit.
#[test]
fn no_sql_ever_removes_or_rewrites_a_chain_entry() {
    let store = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("kollega-store/src");
    let mut sources = Vec::new();
    collect_rust_sources(&store, &mut sources);
    assert!(!sources.is_empty(), "balayage suspect");

    let mut violations = Vec::new();
    for path in &sources {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("lecture de {} : {e}", path.display()));
        // Les COMMENTAIRES sont écartés : ce fichier-ci comme la
        // persistance PARLENT des `GRANT DELETE` absents, et une garde qui
        // se déclenche sur sa propre explication finit désactivée.
        let code: String = content
            .lines()
            .map(|line| line.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join(" ");
        // Blancs normalisés : sans cela, une requête coupée en deux lignes
        // — la façon habituelle d'écrire du SQL en Rust — échappait à des
        // motifs qui supposaient une espace unique.
        let flat = code
            .to_ascii_uppercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        // On cherche le VERBE puis la table à proximité, au lieu d'une
        // liste de phrases exactes. Quatre écritures passaient la liste :
        // requête sur deux lignes, table qualifiée `public.audit_chain`,
        // identifiant entre guillemets, et nom de table injecté par
        // `format!`. Une énumération de fautes est toujours en retard d'une
        // écriture.
        for keyword in ["DELETE", "UPDATE", "TRUNCATE"] {
            let mut rest = flat.as_str();
            while let Some(at) = rest.find(keyword) {
                let after = &rest[at..];
                rest = &after[keyword.len()..];
                let mut end = after.len().min(80);
                while !after.is_char_boundary(end) {
                    end -= 1;
                }
                let window = &after[..end];
                for table in ["AUDIT_CHAIN", "TOOL_CALL_EFFECTS"] {
                    if window.contains(table) {
                        violations.push(format!("{} : {keyword} … {table}", path.display()));
                    }
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "la persistance tente de retirer ou réécrire une preuve :\n{}",
        violations.join("\n")
    );
}

fn collect_rust_sources(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}
