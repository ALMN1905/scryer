//! LOCAL DIAGNOSTIC, not for upstream: every `(release, title)` pair the Sonarr
//! and Radarr parser fixtures assert, run through the same identity matcher as
//! `arr_parity_corpus`. The rows are generated from the arrs' `[TestCase]`
//! attributes (both GPL-3.0) by `gen_parity.py`; they are noisy by nature, because
//! the arrs' "expected title" is whatever their *parser* extracts (a title with
//! `S3` or a year baked in, a site prefix, a language tag), not a library
//! name. Rows that look like that are tagged `suspect` and reported apart.
//!
//! ```text
//! cargo nextest run -p scryer-application arr_parity_fixture_report --run-ignored all --no-capture
//! ```

use super::arr_parity_corpus::{CaseFacet, Cat, scryer_accepts};

#[path = "arr_parity_generated.rs"]
mod generated;

/// One generated row: the arr's own test says `release` parses to `title`.
pub(super) struct FixtureRow {
    pub src: &'static str,
    pub facet: CaseFacet,
    pub release: &'static str,
    pub title: &'static str,
    pub year: Option<i32>,
    /// Why this row may say nothing about Scryer.
    pub suspect: Option<&'static str>,
}

#[tokio::test]
#[ignore = "parity report: prints, does not assert"]
async fn arr_parity_fixture_report() {
    let mut groups: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
    let mut misses = Vec::new();
    for (index, row) in generated::FIXTURE_ROWS.iter().enumerate() {
        let catalog = [Cat {
            name: row.title,
            year: row.year,
            facet: row.facet,
            aliases: &[],
            lang: "eng",
        }];
        let got = scryer_accepts(&catalog, row.release).await;
        let fixture = row.src.split('.').next().unwrap_or(row.src);
        let group = if row.suspect.is_some() {
            format!("{fixture} (suspect rows)")
        } else {
            fixture.to_string()
        };
        let entry = groups.entry(group).or_default();
        entry.0 += 1;
        if got {
            entry.1 += 1;
        } else {
            misses.push((index, row));
        }
    }
    println!("\n=== arr fixture rows: summary (accepted / total) ===");
    for (group, (total, accepted)) in &groups {
        println!("{group:<56} {accepted:>4} / {total:<4}");
    }
    for (heading, suspect_rows) in [("clean rows", false), ("SUSPECT rows", true)] {
        println!("\n=== arr fixture rows: not accepted, {heading} ===");
        for (index, row) in misses
            .iter()
            .filter(|(_, r)| r.suspect.is_some() == suspect_rows)
        {
            println!(
                "fixture-{index:04} [{}] {}\n    catalog: {}\n    release: {}{}",
                row.facet.label(),
                row.src,
                row.title,
                row.release,
                row.suspect
                    .map_or_else(String::new, |why| format!("\n    suspect: {why}")),
            );
        }
    }
}

#[test]
fn arr_parity_fixture_rows_are_well_formed() {
    assert!(
        generated::FIXTURE_ROWS.len() > 500,
        "the generated fixture rows went missing: {}",
        generated::FIXTURE_ROWS.len()
    );
    for row in generated::FIXTURE_ROWS {
        assert!(!row.title.trim().is_empty(), "{}", row.src);
        assert!(!row.release.trim().is_empty(), "{}", row.src);
    }
}
