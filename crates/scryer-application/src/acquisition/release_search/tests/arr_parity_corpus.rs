//! Sonarr/Radarr parity corpus for release-to-title identity matching.
//!
//! One table of `(library titles, release name, does the title match?)` rows,
//! run through the entry point the grab lane uses ([`candidate_title_match`]
//! over [`canonical_title_evidence`]). The expectations are the behaviours
//! Sonarr and Radarr settled on for the same names, written here as small
//! synthetic cases: key normalization (`CleanSeriesTitle` /
//! `CleanMovieTitle`), their alias and year rules, and the open questions
//! from the title-matching comparison.
//!
//! `want` is what the cited source does; a row where Scryer disagrees is a
//! *candidate* gap to triage, not necessarily a defect. Scryer is stricter
//! than the arrs on purpose in places (year veto, same-name collisions,
//! single-word aliases), and several rows below pin that strictness.
//!
//! The report prints, it does not assert:
//!
//! ```text
//! cargo nextest run -p scryer-application arr_parity_report --run-ignored all --no-capture
//! ```

use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) enum CaseFacet {
    Movie,
    Series,
    Anime,
}

impl CaseFacet {
    fn media_facet(self) -> MediaFacet {
        match self {
            Self::Movie => MediaFacet::Movie,
            Self::Series => MediaFacet::Series,
            Self::Anime => MediaFacet::Anime,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Movie => "movie",
            Self::Series => "series",
            Self::Anime => "anime",
        }
    }
}

/// A library title in a hand-written case.
pub(super) struct Cat {
    pub(super) name: &'static str,
    pub(super) year: Option<i32>,
    pub(super) facet: CaseFacet,
    pub(super) aliases: &'static [&'static str],
    /// ISO 639 language the catalog tags the name with.
    pub(super) lang: &'static str,
}

pub(super) const fn cat(name: &'static str, year: Option<i32>, facet: CaseFacet) -> Cat {
    Cat {
        name,
        year,
        facet,
        aliases: &[],
        lang: "eng",
    }
}

/// One hand-written case. The first catalog entry is the subject the release
/// is matched against; the rest are the rest of the library (rivals), which
/// the collision machinery sees.
struct Case {
    id: &'static str,
    /// Where the expectation comes from.
    src: &'static str,
    catalog: Vec<Cat>,
    release: &'static str,
    /// What the arr (or, where the source says so, Scryer's own documented
    /// intent) does with this release: `true` = accepted as catalog[0].
    want: bool,
    note: &'static str,
}

use CaseFacet::{Anime, Movie, Series};

fn build_title(index: usize, entry: &Cat) -> Title {
    let mut title = make_title();
    title.id = format!("parity-title-{index}");
    title.name = entry.name.to_string();
    title.facet = entry.facet.media_facet();
    title.library_id = scryer_domain::default_library_id_for_facet(&title.facet);
    title.year = entry.year;
    title.metadata_language = Some(entry.lang.to_string());
    title.aliases = entry
        .aliases
        .iter()
        .map(|alias| alias.to_string())
        .collect();
    title.tagged_aliases.clear();
    title.external_ids.clear();
    title.imdb_id = None;
    title
}

/// Whether Scryer's identity matcher accepts `release` as `catalog[0]`.
pub(super) async fn scryer_accepts(catalog: &[Cat], release: &str) -> bool {
    let library: Vec<Title> = catalog
        .iter()
        .enumerate()
        .map(|(index, entry)| build_title(index, entry))
        .collect();
    let evidence = spelling_evidence(&library[0], &library).await;
    candidate_title_match(&make_candidate(release, None), &evidence).is_some()
}

fn manual_cases() -> Vec<Case> {
    vec![
        // ---- Key normalization: what Sonarr's NormalizeSeriesTitleFixture and
        // ---- Radarr's NormalizeTitleFixture say is the same title. -----------
        Case {
            id: "norm-punctuation-apostrophes",
            src: "radarr:NormalizeTitleFixture \"I'm a cyborg, but that's OK\" == \"Im a cyborg, but thats ok\"",
            catalog: vec![cat("I'm a Cyborg, But That's OK", Some(2006), Movie)],
            release: "Im.a.Cyborg.But.Thats.OK.2006.1080p.BluRay.x264-GRP",
            want: true,
            note: "apostrophes dropped, not separators, in the arr key",
        },
        Case {
            id: "norm-punctuation-colon-article",
            src: "radarr:NormalizeTitleFixture \"Test: Something à Deux\" -> testsomethingdeux",
            catalog: vec![cat("Test: Something à Deux", Some(2010), Movie)],
            release: "Test.Something.Deux.2010.1080p.BluRay.x264-GRP",
            want: true,
            note: "arr regex treats a lone `à` as an article and drops it mid-title; quirky but pinned by their fixture",
        },
        Case {
            id: "norm-accents-series",
            src: "sonarr:NormalizeSeriesTitleFixture \"Ríkið\" -> rikid",
            catalog: vec![cat("Ríkið", None, Series)],
            release: "Rikid.S01E01.1080p.WEB-DL.H264-GRP",
            want: true,
            note: "accent-stripped release spelling",
        },
        Case {
            id: "norm-accents-trailing-a",
            src: "sonarr:NormalizeSeriesTitleFixture \"Parler à\" -> parlera",
            catalog: vec![cat("Parler à", None, Series)],
            release: "Parler.a.S01E01.1080p.WEB-DL.H264-GRP",
            want: true,
            note: "trailing `à` is kept by the arr (articles are never removed at the end)",
        },
        Case {
            id: "norm-numeric-series-24",
            src: "sonarr:NormalizeSeriesTitleFixture \"24\" -> 24",
            catalog: vec![cat("24", Some(2001), Series)],
            release: "24.S01E01.720p.HDTV.x264-GRP",
            want: true,
            note: "numeric-only title",
        },
        Case {
            id: "norm-numeric-series-90210",
            src: "sonarr:NormalizeSeriesTitleFixture \"90210\" -> 90210",
            catalog: vec![cat("90210", Some(2008), Series)],
            release: "90210.S01E01.720p.HDTV.x264-GRP",
            want: true,
            note: "numeric-only title",
        },
        Case {
            id: "norm-stopword-and-mid",
            src: "sonarr:NormalizeSeriesTitleFixture should_remove_common_words_from_middle_of_title (and)",
            catalog: vec![cat("Word and Word", None, Series)],
            release: "Word.Word.S01E01.1080p.WEB-DL.H264-GRP",
            want: true,
            note: "arr drops and/or/of/the/an/a mid-title on both sides",
        },
        Case {
            id: "norm-stopword-of-mid",
            src: "sonarr:NormalizeSeriesTitleFixture should_remove_common_words_from_middle_of_title (of)",
            catalog: vec![cat("Word of Word", None, Series)],
            release: "Word.Word.S01E01.1080p.WEB-DL.H264-GRP",
            want: true,
            note: "",
        },
        Case {
            id: "norm-stopword-the-mid",
            src: "sonarr:NormalizeSeriesTitleFixture should_remove_common_words_from_middle_of_title (the)",
            catalog: vec![cat("Word the Word", None, Series)],
            release: "Word.Word.S01E01.1080p.WEB-DL.H264-GRP",
            want: true,
            note: "",
        },
        Case {
            id: "norm-stopword-release-adds-and",
            src: "sonarr:NormalizeSeriesTitleFixture (reverse of the above)",
            catalog: vec![cat("Pride Prejudice", None, Series)],
            release: "Pride.and.Prejudice.S01E01.1080p.WEB-DL.H264-GRP",
            want: true,
            note: "release spells a stopword the catalog omits",
        },
        Case {
            id: "norm-stopword-omitted-by-release",
            src: "sonarr:NormalizeSeriesTitleFixture (mid-title stopword dropped on both sides)",
            catalog: vec![cat("Pride and Prejudice", Some(2005), Movie)],
            release: "Pride.Prejudice.2005.1080p.BluRay.x264-GRP",
            want: true,
            note: "",
        },
        Case {
            id: "norm-stopword-kept-at-end",
            src: "sonarr:NormalizeSeriesTitleFixture should_not_remove_common_words_from_end_of_title",
            catalog: vec![cat("Word Word The", None, Series)],
            release: "Word.Word.S01E01.1080p.WEB-DL.H264-GRP",
            want: false,
            note: "arr keeps a trailing stopword in the key, so these differ",
        },
        Case {
            id: "norm-leading-article-significant",
            src: "sonarr:NormalizeSeriesTitleFixture should_not_remove_from_the_beginning_of_the_title",
            catalog: vec![cat("The Series", None, Series)],
            release: "Series.S01E01.1080p.WEB-DL.H264-GRP",
            want: false,
            note: "leading `The` is significant in the arr key (theseries != series)",
        },
        Case {
            id: "norm-percent",
            src: "sonarr:Parser.PercentRegex `3%` -> 3percent",
            catalog: vec![cat("3%", Some(2016), Series)],
            release: "3%.S01E01.720p.NF.WEB-DL.DDP5.1.x264-GRP",
            want: true,
            note: "Sonarr fix for the show 3%",
        },
        Case {
            id: "norm-percent-not-plain-3",
            src: "sonarr:Parser.PercentRegex `3%` -> 3percent (so `3%` must not equal `3`)",
            catalog: vec![cat("3", Some(2010), Series), cat("3%", Some(2016), Series)],
            release: "3%.S01E01.720p.NF.WEB-DL.DDP5.1.x264-GRP",
            want: false,
            note: "the release for `3%` must not be claimed by the show `3`",
        },
        // ---- Punctuation and symbols that the arrs reduce to nothing. --------
        Case {
            id: "punct-dots-initialism",
            src: "sonarr:key `Chicago P.D.` -> chicagopd",
            catalog: vec![cat("Chicago P.D.", Some(2014), Series)],
            release: "Chicago.PD.S01E01.1080p.WEB-DL.H264-GRP",
            want: true,
            note: "also the shape of the newznab-common `Chicago P.D.` -> `Chicago PD` query test",
        },
        Case {
            id: "punct-hyphen-number",
            src: "sonarr:key `Hawaii Five-0` -> hawaiifive0",
            catalog: vec![cat("Hawaii Five-0", Some(2010), Series)],
            release: "Hawaii.Five.0.S01E01.1080p.WEB-DL.H264-GRP",
            want: true,
            note: "",
        },
        Case {
            id: "punct-possessive",
            src: "radarr:key `Schindler's List` -> schindlerslist",
            catalog: vec![cat("Schindler's List", Some(1993), Movie)],
            release: "Schindlers.List.1993.1080p.BluRay.x264-GRP",
            want: true,
            note: "",
        },
        Case {
            id: "punct-colon-dash",
            src: "radarr:key drops `:` and `-`",
            catalog: vec![cat("Mission: Impossible - Fallout", Some(2018), Movie)],
            release: "Mission.Impossible.Fallout.2018.1080p.BluRay.x264-GRP",
            want: true,
            note: "",
        },
        // ---- Ampersand vs `and` (diagnostic probe H2). -----------------------
        Case {
            id: "h2-amp-catalog-and-release",
            src: "probe H2: arr key drops both `&` and `and`",
            catalog: vec![cat("Tom & Jerry", Some(2021), Movie)],
            release: "Tom.and.Jerry.2021.1080p.WEB-DL.DDP5.1.x264-GRP",
            want: true,
            note: "catalog `&`, release `and`",
        },
        Case {
            id: "h2-amp-catalog-amp-release",
            src: "probe H2",
            catalog: vec![cat("Tom & Jerry", Some(2021), Movie)],
            release: "Tom.&.Jerry.2021.1080p.WEB-DL.DDP5.1.x264-GRP",
            want: true,
            note: "catalog `&`, release `&`",
        },
        Case {
            id: "h2-amp-catalog-bare-release",
            src: "probe H2",
            catalog: vec![cat("Tom & Jerry", Some(2021), Movie)],
            release: "Tom.Jerry.2021.1080p.WEB-DL.DDP5.1.x264-GRP",
            want: true,
            note: "catalog `&`, release with neither",
        },
        Case {
            id: "h2-and-catalog-amp-release",
            src: "probe H2",
            catalog: vec![cat("Tom and Jerry", Some(2021), Movie)],
            release: "Tom.&.Jerry.2021.1080p.WEB-DL.DDP5.1.x264-GRP",
            want: true,
            note: "catalog `and`, release `&` (the direction Scryer's own test covers)",
        },
        Case {
            id: "h2-and-catalog-bare-release",
            src: "probe H2",
            catalog: vec![cat("Tom and Jerry", Some(2021), Movie)],
            release: "Tom.Jerry.2021.1080p.WEB-DL.DDP5.1.x264-GRP",
            want: true,
            note: "",
        },
        Case {
            id: "h2-series-law-order-and",
            src: "probe H2 (series)",
            catalog: vec![cat("Law & Order", Some(1990), Series)],
            release: "Law.and.Order.S01E01.1080p.WEB-DL.H264-GRP",
            want: true,
            note: "",
        },
        Case {
            id: "h2-series-law-order-bare",
            src: "probe H2 (series)",
            catalog: vec![cat("Law & Order", Some(1990), Series)],
            release: "Law.Order.S01E01.1080p.WEB-DL.H264-GRP",
            want: true,
            note: "",
        },
        // ---- Roman <-> Arabic numerals (probe H3). ---------------------------
        Case {
            id: "h3-roman-catalog-arabic-release",
            src: "radarr:ParsingService.TryGetMovieBySearchCriteria / MovieService.FindByTitleCandidates",
            catalog: vec![
                cat("Rocky II", Some(1979), Movie),
                cat("Rocky", Some(1976), Movie),
            ],
            release: "Rocky.2.1979.1080p.BluRay.x264-GRP",
            want: true,
            note: "catalog Roman, release Arabic",
        },
        Case {
            id: "h3-arabic-catalog-roman-release",
            src: "radarr:ParsingService.TryGetMovieBySearchCriteria / MovieService.FindByTitleCandidates",
            catalog: vec![
                cat("Rocky 2", Some(1979), Movie),
                cat("Rocky", Some(1976), Movie),
            ],
            release: "Rocky.II.1979.1080p.BluRay.x264-GRP",
            want: true,
            note: "catalog Arabic, release Roman",
        },
        Case {
            id: "h3-roman-exact-control",
            src: "control: identical spelling",
            catalog: vec![
                cat("Rocky II", Some(1979), Movie),
                cat("Rocky", Some(1976), Movie),
            ],
            release: "Rocky.II.1979.1080p.BluRay.x264-GRP",
            want: true,
            note: "",
        },
        Case {
            id: "h3-roman-distinct-numerals",
            src: "radarr:Roman mapping keeps II and III distinct",
            catalog: vec![
                cat("Rocky II", Some(1979), Movie),
                cat("Rocky III", Some(1982), Movie),
            ],
            release: "Rocky.III.1982.1080p.BluRay.x264-GRP",
            want: false,
            note: "must not match Rocky II",
        },
        Case {
            id: "h3-roman-i-vs-ii",
            src: "scryer:title_spelling `Nymphomaniac Volume I` vs `II` guard",
            catalog: vec![cat("Nymphomaniac Volume I", Some(2013), Movie)],
            release: "Nymphomaniac.Volume.II.2013.1080p.BluRay.x264-GRP",
            want: false,
            note: "the guard Scryer must keep while adding Roman/Arabic equivalence",
        },
        // ---- German umlauts (probe H6). -------------------------------------
        Case {
            id: "h6-umlaut-catalog-de",
            src: "radarr:Parser.ReplaceGermanUmlauts",
            catalog: vec![Cat {
                name: "Götter über der Straße",
                year: Some(2016),
                facet: Movie,
                aliases: &[],
                lang: "de",
            }],
            release: "Goetter.ueber.der.Strasse.2016.1080p.BluRay.x264-GRP",
            want: true,
            note: "catalog tagged German",
        },
        Case {
            id: "h6-umlaut-catalog-en",
            src: "radarr:Parser.ReplaceGermanUmlauts",
            catalog: vec![cat("Götter über der Straße", Some(2016), Movie)],
            release: "Goetter.ueber.der.Strasse.2016.1080p.BluRay.x264-GRP",
            want: true,
            note: "same title, catalog language tagged English (arrs do not look at language)",
        },
        Case {
            id: "h6-umlaut-release-ue",
            src: "radarr:Parser.ReplaceGermanUmlauts",
            catalog: vec![cat("Müller", Some(2015), Movie)],
            release: "Mueller.2015.1080p.BluRay.x264-GRP",
            want: true,
            note: "",
        },
        Case {
            id: "h6-umlaut-release-accented",
            src: "radarr:Parser.ReplaceGermanUmlauts",
            catalog: vec![cat("Mueller", Some(2015), Movie)],
            release: "Müller.2015.1080p.BluRay.x264-GRP",
            want: true,
            note: "reverse direction",
        },
        Case {
            id: "accent-amelie-plain-release",
            src: "radarr/sonarr:RemoveAccent / RemoveDiacritics",
            catalog: vec![cat("Amélie", Some(2001), Movie)],
            release: "Amelie.2001.1080p.BluRay.x264-GRP",
            want: true,
            note: "",
        },
        Case {
            id: "accent-amelie-accented-release",
            src: "radarr/sonarr:RemoveAccent / RemoveDiacritics",
            catalog: vec![cat("Amelie", Some(2001), Movie)],
            release: "Amélie.2001.1080p.BluRay.x264-GRP",
            want: true,
            note: "",
        },
        // ---- Alternate titles / translations as catalog aliases. -------------
        Case {
            id: "alias-alt-title-match",
            src: "radarr:MovieService.FindByTitle alt-title lane",
            catalog: vec![Cat {
                name: "Amélie",
                year: Some(2001),
                facet: Movie,
                aliases: &["Le Fabuleux Destin d'Amélie Poulain"],
                lang: "fra",
            }],
            release: "Le.Fabuleux.Destin.d.Amelie.Poulain.2001.1080p.BluRay.x264-GRP",
            want: true,
            note: "alias present in the catalog",
        },
        Case {
            id: "alias-alt-title-absent-control",
            src: "control for alias-alt-title-match",
            catalog: vec![cat("Amélie", Some(2001), Movie)],
            release: "Le.Fabuleux.Destin.d.Amelie.Poulain.2001.1080p.BluRay.x264-GRP",
            want: false,
            note: "without the alias the release is not this title",
        },
        // ---- Years. ----------------------------------------------------------
        Case {
            id: "year-exact-control",
            src: "control",
            catalog: vec![cat("Quillon", Some(2028), Movie)],
            release: "Quillon.2028.1080p.WEB-DL-SYNTH",
            want: true,
            note: "",
        },
        Case {
            id: "year-wrong-far",
            src: "radarr:AllWithYear / scryer:search_cutoff another_years_film",
            catalog: vec![cat("Quillon", Some(2028), Movie)],
            release: "Quillon.2000.1080p.WEB-DL-SYNTH",
            want: false,
            note: "",
        },
        Case {
            id: "h1-year-plus-one",
            src: "probe H1: scryer release_search.rs movie_year_contradicts_subject doc comment (one year either side is the same film)",
            catalog: vec![cat("Quillon", Some(2028), Movie)],
            release: "Quillon.2029.1080p.WEB-DL-SYNTH",
            want: true,
            note: "Radarr only accepts this through SecondaryYear; Scryer's comment says it should match",
        },
        Case {
            id: "h1-year-minus-one",
            src: "probe H1",
            catalog: vec![cat("Quillon", Some(2028), Movie)],
            release: "Quillon.2027.1080p.WEB-DL-SYNTH",
            want: true,
            note: "",
        },
        Case {
            id: "year-release-without-year",
            src: "radarr:ParsingService (parsedMovieInfo.Year < 1800 accepts)",
            catalog: vec![cat("Quillon", Some(2028), Movie)],
            release: "Quillon.1080p.WEB-DL-SYNTH",
            want: true,
            note: "release carries no year",
        },
        Case {
            id: "year-in-title-1917",
            src: "radarr:ParserFixture year-as-title",
            catalog: vec![cat("1917", Some(2019), Movie)],
            release: "1917.2019.1080p.BluRay.x264-GRP",
            want: true,
            note: "",
        },
        Case {
            id: "year-in-title-1917-wrong-year",
            src: "radarr:ParserFixture year-as-title",
            catalog: vec![cat("1917", Some(2019), Movie)],
            release: "1917.2005.1080p.BluRay.x264-GRP",
            want: false,
            note: "",
        },
        Case {
            id: "year-series-year-qualified",
            src: "sonarr:NormalizeSeriesTitleFixture \"Series.2010\" -> series2010; ParsingService title+year",
            catalog: vec![cat("Parenthood", Some(2010), Series)],
            release: "Parenthood.2010.S01E01.720p.HDTV.x264-GRP",
            want: true,
            note: "",
        },
        Case {
            id: "year-series-wrong-year",
            src: "sonarr:ParsingService.GetSeries FindByTitle(TitleWithoutYear, Year)",
            catalog: vec![cat("Castle", Some(2009), Series)],
            release: "Castle.2005.S01E01.720p.HDTV.x264-GRP",
            want: false,
            note: "",
        },
        Case {
            id: "year-series-twin-stays-separate",
            src: "sonarr:SeriesRepository.FindByTitle(title, year)",
            catalog: vec![
                cat("The Office", Some(2005), Series),
                cat("The Office", Some(2001), Series),
            ],
            release: "The.Office.2001.S01E01.1080p.WEB-DL.H264-GRP",
            want: false,
            note: "release belongs to the 2001 twin, not catalog[0]",
        },
        Case {
            id: "alias-anime-romanized-alias",
            src: "control: catalog alias answers a fansub release under its romanized name",
            catalog: vec![Cat {
                name: "Aurora Falls",
                year: Some(2020),
                facet: Anime,
                aliases: &["Aurora no Taki"],
                lang: "eng",
            }],
            release: "[GRP] Aurora no Taki - 03 [1080p]",
            want: true,
            note: "",
        },
        // ---- Multi-title releases (probe H5). --------------------------------
        Case {
            id: "h5-aka-two-different-library-titles-a",
            src: "sonarr:ParsingService.GetSeriesByAllTitles (parts resolving to different series -> no series selected)",
            catalog: vec![
                cat("Aurora Falls", Some(2015), Series),
                cat("Birchwood Lane", Some(2018), Series),
            ],
            release: "Aurora.Falls.AKA.Birchwood.Lane.S01E01.1080p.WEB-DL.H264-GRP",
            want: false,
            note: "two distinct library series in one release: Sonarr selects none",
        },
        Case {
            id: "h5-aka-two-different-library-titles-b",
            src: "sonarr:ParsingService.GetSeriesByAllTitles",
            catalog: vec![
                cat("Birchwood Lane", Some(2018), Series),
                cat("Aurora Falls", Some(2015), Series),
            ],
            release: "Aurora.Falls.AKA.Birchwood.Lane.S01E01.1080p.WEB-DL.H264-GRP",
            want: false,
            note: "same, seen from the other title",
        },
        Case {
            id: "aka-one-library-title",
            src: "radarr:ParserFixture should_parse_movie_alternative_titles (Sydney AKA Hard Eight)",
            catalog: vec![cat("Hard Eight", Some(1996), Movie)],
            release: "Sydney.AKA.Hard.Eight.1996.1080p.BluRay.x264-GRP",
            want: true,
            note: "AKA release, library holds one of the halves",
        },
        Case {
            id: "slash-one-library-title",
            src: "sonarr:Parser.TitleComponentsRegex (a / b)",
            catalog: vec![cat("Aurora Falls", Some(2015), Series)],
            release: "Aurora.Falls.-.Other.Name.S01E01.1080p.WEB-DL.H264-GRP",
            want: false,
            note: "control: an unrelated second title is not an alias unless it is one",
        },
    ]
}

struct Outcome {
    id: String,
    src: String,
    facet: &'static str,
    catalog: String,
    release: String,
    want: bool,
    got: bool,
    note: String,
}

impl Outcome {
    fn agrees(&self) -> bool {
        self.want == self.got
    }
}

async fn run_all() -> Vec<Outcome> {
    let mut outcomes = Vec::new();

    for case in manual_cases() {
        let got = scryer_accepts(&case.catalog, case.release).await;
        outcomes.push(Outcome {
            id: case.id.to_string(),
            src: case.src.to_string(),
            facet: case.catalog[0].facet.label(),
            catalog: case
                .catalog
                .iter()
                .map(|entry| match entry.year {
                    Some(year) => format!("{} ({year})", entry.name),
                    None => entry.name.to_string(),
                })
                .collect::<Vec<_>>()
                .join(" | "),
            release: case.release.to_string(),
            want: case.want,
            got,
            note: case.note.to_string(),
        });
    }

    outcomes
}

/// Group key for the summary: the case id's prefix.
fn group_of(outcome: &Outcome) -> String {
    outcome
        .id
        .split('-')
        .next()
        .unwrap_or(&outcome.id)
        .to_string()
}

#[tokio::test]
#[ignore = "parity report: prints, does not assert"]
async fn arr_parity_report() {
    let outcomes = run_all().await;

    let mut groups: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    for outcome in &outcomes {
        let entry = groups.entry(group_of(outcome)).or_default();
        entry.0 += 1;
        if outcome.agrees() {
            entry.1 += 1;
        }
    }

    println!("\n=== arr parity: summary (agree / total) ===");
    for (group, (total, agree)) in &groups {
        println!("{group:<48} {agree:>4} / {total:<4}");
    }
    let total = outcomes.len();
    let agree = outcomes.iter().filter(|o| o.agrees()).count();
    println!("{:<48} {agree:>4} / {total:<4}", "TOTAL");

    println!(
        "legend: WANT = what the arr (or the cited source) does; GOT = Scryer's identity matcher"
    );
    println!("\n=== arr parity: disagreements ===");
    for outcome in outcomes.iter().filter(|o| !o.agrees()) {
        println!(
            "[{}] {} want={} got={}\n    src:     {}\n    catalog: {}\n    release: {}{}",
            outcome.facet,
            outcome.id,
            if outcome.want { "match" } else { "no-match" },
            if outcome.got { "match" } else { "no-match" },
            outcome.src,
            outcome.catalog,
            outcome.release,
            if outcome.note.is_empty() {
                String::new()
            } else {
                format!("\n    note:    {}", outcome.note)
            },
        );
    }
}

#[test]
fn arr_parity_corpus_is_well_formed() {
    let cases = manual_cases();
    let mut ids = std::collections::HashSet::new();
    for case in &cases {
        assert!(ids.insert(case.id), "duplicate case id {}", case.id);
        assert!(!case.catalog.is_empty(), "{} has no catalog", case.id);
        assert!(
            !case.release.trim().is_empty(),
            "{} has no release",
            case.id
        );
    }
}
