# Progress
- 2026-10-28: both fixes and tests done; novo-parts tests and clippy green.

## 2026-10-28 Windows hardware test findings PIR-01..03
- PIR-01 (price "9 EUR/100" read as 9.0): already fixed by #1562, merged after the tested build 64b31c2f; `tests/price_units.rs` covers per 100, per 1000, per metre, unreadable units.
- PIR-02 (a row wider than the header taken as the header): `guess_header_row` now compares against the most common row width. Tests in `tests/table_import.rs`.
- PIR-03 (Parts window 148 ms per idle frame at 5000 parts): rows used one `records.json` read per part per frame. Now one read per list (`LocalLibrary::stored_keys`) and the rows are kept until search, maker, catalog (`Catalog::revision`) or the records file (`LocalLibrary::stamp`) change. Not measured in the real window yet; the window needs a re-run of `09-p07-5000-rows.qa` on Windows.
