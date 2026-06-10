//! Column-aware detection for pasted tabular data (CSV / TSV / semicolon /
//! pipe). A bare value like `486-12-7795` without dashes or an arbitrary
//! full name doesn't match any pattern — but when it sits under a header
//! named `ssn` or `full_name`, the header tells us its semantics. This stage
//! turns header knowledge into spans for every cell in a mapped column.
//!
//! Deliberate v1 scope:
//! - Naive splitting, no quoted-field handling. Rows whose cell count
//!   differs from the header row are skipped entirely — a misparsed row
//!   would put spans at wrong offsets, which is worse than no span.
//! - Pure function of the text (no store/settings): pack toggles apply via
//!   `filter_disabled_packs` at the call sites, like regex spans.
//! - Blocking kinds (cards, IBANs…) only keep their blocking kind when the
//!   cell passes the checksum validator; otherwise the cell is aliased as
//!   CUSTOM (which never blocks). A column header is strong evidence the
//!   data is sensitive, but not strong enough to hard-block invalid values.
//! - Overlap policy at merge time: regex wins over structured (the pattern
//!   engine's typed span is at least as specific), structured wins over
//!   watchlist and NER.

use crate::vendetta::{self, Kind, Span};

const DELIMS: [char; 4] = [',', '\t', ';', '|'];
const MAX_CELLS: usize = 5_000;
/// Tabular shape requires the header row plus at least one data row.
const MIN_ROWS: usize = 2;

/// Map a header cell to the Kind its column holds. Contains-based and
/// case-insensitive: "Customer Email", "email_address", "work-email" all
/// land on EMAIL. Order matters — first hit wins, so more specific names
/// come before generic substrings.
fn kind_for_header(raw: &str) -> Option<Kind> {
    let h: String = raw
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect();
    let has = |needle: &str| h.split_whitespace().any(|w| w == needle);
    let contains = |needle: &str| h.contains(needle);

    // Specific multi-word / unambiguous names first.
    if contains("social security") || has("ssn") { return Some(Kind::SSN); }
    if contains("credit card") || contains("card number") || has("pan") || has("cc") { return Some(Kind::CREDITCARD); }
    if has("iban") { return Some(Kind::IBAN); }
    if has("routing") || has("aba") { return Some(Kind::US_BANK); }
    if contains("account number") || has("acct") { return Some(Kind::US_BANK); }
    if has("swift") || has("bic") { return Some(Kind::SWIFT_BIC); }
    if has("ein") { return Some(Kind::EIN); }
    if has("itin") { return Some(Kind::US_ITIN); }
    if has("sin") { return Some(Kind::CA_SIN); }
    if has("nhs") { return Some(Kind::UK_NHS); }
    if has("nino") || contains("national insurance") { return Some(Kind::UK_NINO); }
    if has("tfn") { return Some(Kind::AU_TFN); }
    if has("aadhaar") || has("aadhar") { return Some(Kind::AADHAAR); }
    if has("mbi") || contains("medicare") { return Some(Kind::MEDICARE_MBI); }
    if has("npi") { return Some(Kind::NPI); }
    if has("dea") { return Some(Kind::DEA); }
    if has("mrn") || contains("medical record") { return Some(Kind::MRN); }
    if contains("member id") || contains("subscriber") || contains("policy number") { return Some(Kind::HEALTH_ID); }
    if has("vin") { return Some(Kind::VIN); }
    if has("passport") { return Some(Kind::PASSPORT); }
    if contains("driver") || has("dl") || contains("licen") { return Some(Kind::DRIVERS_LICENSE); }
    if has("dob") || contains("birth") { return Some(Kind::DOB); }
    if has("email") || has("mail") { return Some(Kind::EMAIL); }
    if has("phone") || has("mobile") || has("cell") || has("tel") || has("fax") { return Some(Kind::PHONE); }
    if has("ip") || contains("ip address") { return Some(Kind::IP); }
    if has("mac") { return Some(Kind::MAC_ADDRESS); }
    if has("wallet") || has("btc") || has("eth") { return Some(Kind::CRYPTO_WALLET); }
    if contains("address") || has("street") { return Some(Kind::ADDRESS); }
    if has("salary") || has("income") || contains("compensation") || has("wage") { return Some(Kind::MONEY); }
    if has("name") || has("firstname") || has("lastname") || has("surname") || has("fullname") { return Some(Kind::NAME); }
    if contains("case number") || has("docket") { return Some(Kind::CASE_NO); }
    if has("password") || has("secret") || has("token") || contains("api key") || has("apikey") { return Some(Kind::CREDENTIAL); }
    None
}

/// Values that mean "no data" — never worth a span.
fn cell_is_empty(v: &str) -> bool {
    let t = v.trim();
    t.is_empty()
        || matches!(
            t.to_ascii_lowercase().as_str(),
            "null" | "none" | "n/a" | "na" | "nil" | "-" | "--" | "unknown" | "tbd"
        )
}

/// Detect tabular blocks in `text` and return spans for cells in columns
/// whose header maps to a sensitive kind. Spans carry empty aliases —
/// `apply_alias_map` assigns them, exactly like regex spans.
pub fn structured_spans(text: &str) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let mut cells_seen = 0usize;

    // Collect (line, byte_offset_of_line_start).
    let mut lines: Vec<(&str, usize)> = Vec::new();
    let mut off = 0usize;
    for line in text.split('\n') {
        lines.push((line, off));
        off += line.len() + 1;
    }

    let mut i = 0usize;
    while i < lines.len() {
        let Some((delim, ncols)) = header_shape(lines[i].0) else {
            i += 1;
            continue;
        };
        // A block is the run of subsequent lines with the same delimiter
        // count. Require at least one data row.
        let mut block_end = i + 1;
        while block_end < lines.len()
            && lines[block_end].0.matches(delim).count() + 1 == ncols
            && !lines[block_end].0.trim().is_empty()
        {
            block_end += 1;
        }
        if block_end - i < MIN_ROWS {
            i += 1;
            continue;
        }

        let header_cells: Vec<&str> = lines[i].0.split(delim).collect();
        let mapped: Vec<Option<Kind>> = header_cells.iter().map(|h| kind_for_header(h)).collect();
        if mapped.iter().all(|k| k.is_none()) {
            i = block_end;
            continue;
        }

        for (line, line_off) in &lines[i + 1..block_end] {
            let mut cell_off = 0usize;
            for (col, cell) in line.split(delim).enumerate() {
                let Some(Some(kind)) = mapped.get(col) else {
                    cell_off += cell.len() + delim.len_utf8();
                    continue;
                };
                cells_seen += 1;
                if cells_seen > MAX_CELLS {
                    return out;
                }
                let trimmed = cell.trim();
                if !cell_is_empty(trimmed) {
                    // Header semantics say sensitive; the validator decides
                    // whether a blocking kind keeps its teeth. An invalid
                    // value under a "card_number" header still gets aliased
                    // (as CUSTOM) but must not hard-block egress.
                    let kind = if vendetta::validate(kind, trimmed) {
                        kind.clone()
                    } else if vendetta::is_critical(kind) {
                        Kind::CUSTOM
                    } else {
                        kind.clone()
                    };
                    let lead = cell.len() - cell.trim_start().len();
                    let start = line_off + cell_off + lead;
                    out.push(Span {
                        start,
                        end: start + trimmed.len(),
                        kind: kind.clone(),
                        raw: trimmed.to_string(),
                        alias: String::new(),
                        confidence: 0.9, // header-anchored: strong, not checksum-strong
                    });
                }
                cell_off += cell.len() + delim.len_utf8();
            }
        }
        i = block_end;
    }
    out
}

/// Does this line look like a header row? Returns (delimiter, column count).
/// Requires ≥2 columns and at least one header that maps to a kind — plus
/// every cell being short and value-free (headers are labels, not data).
fn header_shape(line: &str) -> Option<(char, usize)> {
    for &d in DELIMS.iter() {
        let n = line.matches(d).count();
        if n == 0 {
            continue;
        }
        let cells: Vec<&str> = line.split(d).collect();
        let plausible = cells.len() >= 2
            && cells.iter().all(|c| {
                let t = c.trim();
                !t.is_empty() && t.len() <= 40 && !t.contains('@')
            })
            && cells.iter().any(|c| kind_for_header(c).is_some());
        if plausible {
            return Some((d, cells.len()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_columns_map_to_kinds_with_exact_offsets() {
        let text = "patient roster:\nname,ssn,email\nJordan Vance,486121234,jordan@example.com\nMia Torres,523887654,mia@example.com\n";
        let spans = structured_spans(text);
        // 3 mapped columns × 2 data rows = 6 spans.
        assert_eq!(spans.len(), 6, "{spans:?}");
        for s in &spans {
            assert_eq!(&text[s.start..s.end], s.raw, "offset drift for {s:?}");
        }
        assert!(spans.iter().any(|s| matches!(s.kind, Kind::NAME) && s.raw == "Jordan Vance"));
        // Bare 9-digit SSNs (no dashes) never match the regex pattern — the
        // header is what catches them.
        assert!(spans.iter().any(|s| matches!(s.kind, Kind::SSN) && s.raw == "486121234"));
    }

    #[test]
    fn tsv_and_pipe_delimiters() {
        let text = "email\tphone\na@b.co\t415 555 0142\n";
        assert_eq!(structured_spans(text).len(), 2);
        let text = "vin|owner name\n1HGCM82633A004352|Dana Reyes\n";
        let spans = structured_spans(text);
        assert!(spans.iter().any(|s| matches!(s.kind, Kind::VIN)));
        assert!(spans.iter().any(|s| matches!(s.kind, Kind::NAME)));
    }

    #[test]
    fn invalid_blocking_values_downgrade_to_custom() {
        // Luhn-invalid card under a card header: aliased, but never blockable.
        let text = "card_number,amount\n4111111111111112,$50\n4111111111111111,$90\n";
        let spans = structured_spans(text);
        let invalid = spans.iter().find(|s| s.raw == "4111111111111112").unwrap();
        assert!(matches!(invalid.kind, Kind::CUSTOM), "{invalid:?}");
        let valid = spans.iter().find(|s| s.raw == "4111111111111111").unwrap();
        assert!(matches!(valid.kind, Kind::CREDITCARD), "{valid:?}");
    }

    #[test]
    fn prose_and_ragged_rows_are_ignored() {
        assert!(structured_spans("just a sentence, with a comma, and another").is_empty());
        assert!(structured_spans("no table here at all").is_empty());
        // Ragged data row (cell count mismatch) is skipped, clean row kept.
        let text = "email,phone\nbroken,row,with,extra,cells\nok@example.com,415 555 0142\n";
        let spans = structured_spans(text);
        assert!(spans.iter().all(|s| s.raw != "broken"));
        // The ragged row ends the block scan — conservative, no misaligned spans.
    }

    #[test]
    fn empty_markers_skipped_and_cap_respected() {
        let text = "email,ssn\nn/a,486121234\nnull,-\n";
        let spans = structured_spans(text);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].raw, "486121234");
    }

    #[test]
    fn header_without_data_rows_is_not_a_table() {
        assert!(structured_spans("email,phone\n").is_empty());
    }
}
