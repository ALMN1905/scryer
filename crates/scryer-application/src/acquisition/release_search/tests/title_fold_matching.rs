//! Regression tests for the spellings a scene release and a catalog name
//! disagree on without disagreeing about the title (see
//! `scryer_domain::title_spelling::title_equivalence_fold`).
//!
//! Each positive row is a release that the matcher refused before the fold
//! existed. The negative rows pin the other half of the contract: the fold
//! must never make two different titles equal, and a same-name collision in
//! the library must still stop a spelling-only match.

use super::arr_parity_corpus::{
    CaseFacet::{Movie, Series},
    Cat, cat, scryer_accepts,
};

fn one(
    name: &'static str,
    year: Option<i32>,
    facet: super::arr_parity_corpus::CaseFacet,
) -> Vec<Cat> {
    vec![cat(name, year, facet)]
}

#[tokio::test]
async fn apostrophes_and_dotted_initialisms_do_not_split_a_title() {
    for (catalog, year, facet, release) in [
        (
            "Schindler's List",
            Some(1993),
            Movie,
            "Schindlers.List.1993.1080p.BluRay.x264-GRP",
        ),
        (
            "I'm a Cyborg, But That's OK",
            Some(2006),
            Movie,
            "Im.a.Cyborg.But.Thats.OK.2006.1080p.BluRay.x264-GRP",
        ),
        (
            "Chicago P.D.",
            Some(2014),
            Series,
            "Chicago.PD.S01E01.1080p.WEB-DL.H264-GRP",
        ),
        (
            "Marvel's Agents of S.H.I.E.L.D.",
            Some(2013),
            Series,
            "Marvels.Agents.of.SHIELD.S01E01.1080p.WEB-DL.H264-GRP",
        ),
    ] {
        assert!(
            scryer_accepts(&one(catalog, year, facet), release).await,
            "{catalog} <= {release}"
        );
    }
}

#[tokio::test]
async fn ampersand_and_and_read_as_the_same_word_in_both_directions() {
    for (catalog, year, facet, release) in [
        (
            "Tom & Jerry",
            Some(2021),
            Movie,
            "Tom.and.Jerry.2021.1080p.WEB-DL.DDP5.1.x264-GRP",
        ),
        (
            "Tom & Jerry",
            Some(2021),
            Movie,
            "Tom.&.Jerry.2021.1080p.WEB-DL.DDP5.1.x264-GRP",
        ),
        (
            "Tom & Jerry",
            Some(2021),
            Movie,
            "Tom.Jerry.2021.1080p.WEB-DL.DDP5.1.x264-GRP",
        ),
        (
            "Tom and Jerry",
            Some(2021),
            Movie,
            "Tom.Jerry.2021.1080p.WEB-DL.DDP5.1.x264-GRP",
        ),
        (
            "Law & Order",
            Some(1990),
            Series,
            "Law.and.Order.S01E01.1080p.WEB-DL.H264-GRP",
        ),
        (
            "Pride and Prejudice",
            Some(2005),
            Movie,
            "Pride.Prejudice.2005.1080p.BluRay.x264-GRP",
        ),
    ] {
        assert!(
            scryer_accepts(&one(catalog, year, facet), release).await,
            "{catalog} <= {release}"
        );
    }
}

#[tokio::test]
async fn roman_numerals_and_digits_name_the_same_sequel() {
    let library = |target: &'static str| {
        vec![
            cat(target, Some(1979), Movie),
            cat("Rocky", Some(1976), Movie),
            cat("Rocky III", Some(1982), Movie),
        ]
    };
    assert!(scryer_accepts(&library("Rocky II"), "Rocky.2.1979.1080p.BluRay.x264-GRP").await);
    assert!(scryer_accepts(&library("Rocky 2"), "Rocky.II.1979.1080p.BluRay.x264-GRP").await);
    assert!(scryer_accepts(&library("Rocky II"), "Rocky.II.1979.1080p.BluRay.x264-GRP").await);
}

#[tokio::test]
async fn the_fold_never_makes_different_titles_equal() {
    for (catalog, year, facet, release) in [
        // Numerals stay distinct from each other.
        (
            "Rocky II",
            Some(1979),
            Movie,
            "Rocky.III.1982.1080p.BluRay.x264-GRP",
        ),
        (
            "Rocky II",
            Some(1979),
            Movie,
            "Rocky.3.1982.1080p.BluRay.x264-GRP",
        ),
        (
            "Nymphomaniac Volume I",
            Some(2013),
            Movie,
            "Nymphomaniac.Volume.II.2013.1080p.BluRay.x264-GRP",
        ),
        // The first and last word are never dropped.
        (
            "The Series",
            None,
            Series,
            "Series.S01E01.1080p.WEB-DL.H264-GRP",
        ),
        (
            "Word Word The",
            None,
            Series,
            "Word.Word.S01E01.1080p.WEB-DL.H264-GRP",
        ),
        // An extra word is an extra word, not a stopword.
        (
            "Tom and Jerry Kids",
            Some(1990),
            Series,
            "Tom.and.Jerry.S01E01.1080p.WEB-DL.H264-GRP",
        ),
        // A wrong year still vetoes a spelling match.
        (
            "Tom & Jerry",
            Some(2021),
            Movie,
            "Tom.and.Jerry.1940.1080p.WEB-DL.DDP5.1.x264-GRP",
        ),
    ] {
        assert!(
            !scryer_accepts(&one(catalog, year, facet), release).await,
            "{catalog} must not accept {release}"
        );
    }
}

#[tokio::test]
async fn a_same_name_title_in_the_library_still_blocks_a_spelling_only_match() {
    let library = |first: &'static str, first_year, second: &'static str, second_year| {
        vec![
            cat(first, Some(first_year), Movie),
            cat(second, Some(second_year), Movie),
        ]
    };
    // Year picks the right one of the two when the release carries it.
    assert!(
        scryer_accepts(
            &library("Tom & Jerry", 2021, "Tom and Jerry", 1940),
            "Tom.and.Jerry.2021.1080p.WEB-DL.DDP5.1.x264-GRP"
        )
        .await
    );
    assert!(
        !scryer_accepts(
            &library("Tom & Jerry", 2021, "Tom and Jerry", 1940),
            "Tom.and.Jerry.1940.1080p.WEB-DL.DDP5.1.x264-GRP"
        )
        .await
    );
    // Without a year the release literally spells the other title, so the
    // `&` title must not claim it on spelling alone, and the literal one does.
    let yearless = "Tom.and.Jerry.1080p.BluRay.x264-GRP";
    assert!(
        !scryer_accepts(
            &library("Tom & Jerry", 2021, "Tom and Jerry", 1940),
            yearless
        )
        .await
    );
    assert!(
        scryer_accepts(
            &library("Tom and Jerry", 1940, "Tom & Jerry", 2021),
            yearless
        )
        .await
    );
}
