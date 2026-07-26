//! Propriétés générales de la surface pure de `kollega-core` (BLOC 8).

use kollega_core::{
    prompt::compile, AssemblyLimits, Cents, Classification, Email, Segment, SourceRef,
};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(3000))]

    /// `checked_add` correspond exactement à l'addition mathématique et ne
    /// déborde jamais en silence : elle vaut `Some` ssi la somme vraie tient
    /// dans un `i64` (calculée en `i128` comme oracle).
    #[test]
    fn cents_checked_add_matches_exact_arithmetic(a in any::<i64>(), b in any::<i64>()) {
        let exact = a as i128 + b as i128;
        let in_range = exact >= i64::MIN as i128 && exact <= i64::MAX as i128;
        match Cents(a).checked_add(Cents(b)) {
            Some(sum) => {
                prop_assert!(in_range);
                prop_assert_eq!(sum.0 as i128, exact);
            }
            None => prop_assert!(!in_range),
        }
    }

    /// Associativité quand tout tient dans l'intervalle : `(a+b)+c ==
    /// a+(b+c)`. (Ne vaut pas pour `saturating_add`, non testée ici.)
    #[test]
    fn cents_addition_is_associative_when_in_range(
        a in -1_000_000_000i64..1_000_000_000,
        b in -1_000_000_000i64..1_000_000_000,
        c in -1_000_000_000i64..1_000_000_000,
    ) {
        let left = Cents(a).checked_add(Cents(b)).and_then(|s| s.checked_add(Cents(c)));
        let right = Cents(b).checked_add(Cents(c)).and_then(|s| Cents(a).checked_add(s));
        prop_assert_eq!(left, right);
    }

    /// `saturating_add` ne descend jamais sous une opérande positive ni ne
    /// monte au-dessus d'une opérande négative de façon aberrante : elle
    /// reste bornée par l'intervalle `i64` et égale l'exact quand il tient.
    #[test]
    fn cents_saturating_add_is_clamped_exact(a in any::<i64>(), b in any::<i64>()) {
        let exact = a as i128 + b as i128;
        let got = Cents(a).saturating_add(Cents(b)).0 as i128;
        let clamped = exact.clamp(i64::MIN as i128, i64::MAX as i128);
        prop_assert_eq!(got, clamped);
    }

    /// La normalisation d'une adresse est idempotente : re-parser le résultat
    /// de `parse` redonne exactement la même chaîne normalisée.
    #[test]
    fn email_normalization_is_idempotent(local in "[a-zA-Z0-9._-]{1,20}", dom in "[a-zA-Z0-9-]{1,15}") {
        let raw = format!("{local}@{dom}.com");
        if let Ok(email) = Email::parse(&raw) {
            let once = email.as_str().to_owned();
            let twice = Email::parse(&once).expect("une adresse normalisée reste valide");
            prop_assert_eq!(twice.as_str(), once.as_str());
            // Idempotence stricte de la casse : déjà en minuscules.
            prop_assert_eq!(email.as_str(), email.as_str().to_lowercase());
        }
    }

    /// Deux adresses qui ne diffèrent que par la casse se confondent après
    /// normalisation ; deux adresses distinctes en ASCII (hors casse) ne se
    /// confondent jamais.
    #[test]
    fn email_case_folds_but_distinct_stays_distinct(
        a in "[a-z0-9]{3,10}",
        b in "[a-z0-9]{3,10}",
    ) {
        let upper = Email::parse(&format!("{}@ex.com", a.to_uppercase())).unwrap();
        let lower = Email::parse(&format!("{a}@ex.com")).unwrap();
        prop_assert_eq!(upper.as_str(), lower.as_str());
        if a != b {
            let ea = Email::parse(&format!("{a}@ex.com")).unwrap();
            let eb = Email::parse(&format!("{b}@ex.com")).unwrap();
            prop_assert_ne!(ea.as_str(), eb.as_str());
        }
    }

    /// Invariant 7, confinement : pour TOUTE chaîne (contrôles, bidi,
    /// invisibles, multi-octets compris), l'assemblage transporte le contenu
    /// externe verbatim (préfixe verbatim si coupé à la borne, drapeau levé)
    /// et laisse l'instruction système et la demande humaine strictement
    /// inchangées.
    #[test]
    fn compile_transports_external_content_verbatim(
        content in proptest::string::string_regex(
            "(?s)[\\x00-\\x1f\\x7fa-z0-9 \u{202e}\u{2066}\u{200b}\u{feff}\u{ad}éΩ€𝄞中\"\\\\{}\\[\\]:,]{0,300}"
        ).expect("regex de génération"),
        max_chars in 1usize..200,
    ) {
        let limits = AssemblyLimits { max_document_chars: max_chars };
        let segments = vec![
            Segment::SystemInstruction("Instruction figée.".to_owned()),
            Segment::UserRequest("Demande figée.".to_owned()),
            Segment::ExternalContent {
                content: content.clone(),
                source: SourceRef::Mail { message_id: "<p@t>".to_owned() },
                classification: Classification::Internal,
            },
        ];
        let compiled = compile(&segments, &limits).expect("assemblage");
        prop_assert_eq!(compiled.system.as_str(), "Instruction figée.");
        prop_assert_eq!(compiled.user_request.as_str(), "Demande figée.");
        let doc = &compiled.documents[0];
        if content.chars().count() <= max_chars {
            prop_assert!(!doc.truncated);
            prop_assert_eq!(&doc.content, &content);
        } else {
            prop_assert!(doc.truncated);
            prop_assert_eq!(doc.content.chars().count(), max_chars);
            prop_assert!(content.starts_with(&doc.content));
        }
    }
}
