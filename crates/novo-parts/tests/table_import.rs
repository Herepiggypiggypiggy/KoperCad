//! The table import with a mapping step: CSV and Excel files with any column
//! layout, units converted, every row checked, nothing guessed.
//!
//! All data here is made up (maker "Example Maker").

use novo_parts::*;

fn csv_sheet(text: &str) -> Sheet {
    TableFile::read("parts.csv", text.as_bytes())
        .unwrap()
        .sheets
        .remove(0)
}

fn preview(sheet: &Sheet, mapping: &Mapping) -> ImportPreview {
    build_preview(sheet, mapping, "parts.csv", "2026-10-21", &Catalog::new())
}

fn target_of<'a>(mapping: &'a Mapping, header: &str) -> &'a Target {
    &mapping
        .columns
        .iter()
        .find(|c| c.header == header)
        .unwrap_or_else(|| panic!("no column {header}"))
        .target
}

const GERMAN: &str = "Hersteller;Artikelnummer;Bezeichnung;Nennstrom [A];Verlustleistung (W);Breite mm;Höhe mm;Tiefe mm;ETIM;Preis\n\
Example Maker;EX-100;Schütz 9 A;9;1,2;45;85;70;EC000066;12,50 €\n\
Example Maker;EX-101;Schütz 12 A;12;1,8;45;85;70;EC000066;1.234,56 €\n";

#[test]
fn german_headers_units_and_decimal_commas_map_and_convert() {
    let sheet = csv_sheet(GERMAN);
    let mapping = Mapping::suggest(&sheet, 0);
    assert_eq!(*target_of(&mapping, "Hersteller"), Target::Manufacturer);
    assert_eq!(*target_of(&mapping, "Artikelnummer"), Target::OrderNumber);
    assert_eq!(
        *target_of(&mapping, "Nennstrom [A]"),
        Target::Measure("rated_current_a")
    );
    assert_eq!(
        *target_of(&mapping, "Verlustleistung (W)"),
        Target::Measure("power_loss_w")
    );
    assert_eq!(*target_of(&mapping, "ETIM"), Target::Etim);
    assert_eq!(*target_of(&mapping, "Preis"), Target::Price);
    let p = preview(&sheet, &mapping);
    assert!(p.problems.is_empty(), "{:?}", p.problems);
    assert_eq!(p.counts().new, 2);
    let part = &p.parts[0];
    assert_eq!(part.order_number, "EX-100");
    assert_eq!(part.data["power_loss_w"], novo_core::Value::Float(1.2));
    assert_eq!(part.data["rated_current_a"], novo_core::Value::Float(9.0));
    let d = part.dimensions.unwrap();
    assert_eq!((d.width_mm, d.height_mm, d.depth_mm), (45.0, 85.0, 70.0));
    assert_eq!(part.class.etim.as_deref(), Some("EC000066"));
    let price = part.price.as_ref().unwrap();
    assert_eq!((price.amount, price.currency.as_str()), (12.5, "EUR"));
    // European thousands and decimals in one cell.
    assert_eq!(p.parts[1].price.as_ref().unwrap().amount, 1234.56);
    // Header units are not "assumed".
    assert!(p.assumptions.is_empty(), "{:?}", p.assumptions);
}

#[test]
fn a_cell_unit_beats_the_column_unit_and_milliamps_convert() {
    let sheet = csv_sheet(
        "Maker,Order no,Rated current,Width\nExample Maker,A-1,500 mA,6 cm\nExample Maker,A-2,2 A,45\n",
    );
    let mapping = Mapping::suggest(&sheet, 0);
    let p = preview(&sheet, &mapping);
    assert_eq!(
        p.parts[0].data["rated_current_a"],
        novo_core::Value::Float(0.5)
    );
    assert_eq!(p.parts[0].data["width_mm"], novo_core::Value::Float(60.0));
    // A cell without any unit in a column without one: taken as the stored
    // unit, and the preview says so.
    assert_eq!(p.parts[1].data["width_mm"], novo_core::Value::Float(45.0));
    assert!(
        p.assumptions
            .iter()
            .any(|a| a.contains("Width") && a.contains("mm")),
        "{:?}",
        p.assumptions
    );
}

#[test]
fn rows_that_cannot_be_trusted_are_skipped_or_stripped_and_said_so() {
    let sheet = csv_sheet(
        "Maker;Order number;Width [mm];Height [mm];Depth [mm];Power loss [W];ETIM;Rated current [A]\n\
         Example Maker;B-1;45;85;70;2;EC000066;10\n\
         Example Maker;;45;85;70;2;EC000066;10\n\
         ;B-3;45;85;70;2;EC000066;10\n\
         Example Maker;B-4;45000;85;70;2;12345;10 kV\n\
         Example Maker;B-5;45;85;70;-3;EC000066;1,500\n\
         Example Maker;B-1;50;85;70;2;EC000066;10\n",
    );
    let mapping = Mapping::suggest(&sheet, 0);
    let p = preview(&sheet, &mapping);
    let status: Vec<RowStatus> = p.rows.iter().map(|r| r.status).collect();
    assert_eq!(
        status,
        [
            RowStatus::Skipped, // B-1 replaced by the later duplicate
            RowStatus::Skipped, // no order number
            RowStatus::Skipped, // no manufacturer
            RowStatus::New,
            RowStatus::New,
            RowStatus::New, // the later B-1
        ]
    );
    let issue =
        |row: usize, text: &str| p.rows[row].issues.iter().any(|i| i.message.contains(text));
    assert!(issue(0, "same part is in row 7"));
    assert!(issue(1, "no order number"));
    assert!(issue(2, "no manufacturer"));
    // B-4: absurd width dropped, bad ETIM dropped, a kV unit on an ampere column refused.
    assert!(issue(3, "not a possible size"));
    assert!(issue(3, "not an ETIM class"));
    assert!(issue(3, "does not fit"));
    let b4 = p.parts.iter().find(|x| x.order_number == "B-4").unwrap();
    assert!(b4.dimensions.is_none());
    assert!(b4.class.etim.is_none());
    assert!(!b4.data.contains_key("rated_current_a"));
    // B-5: negative loss dropped; 1,500 is flagged as ambiguous.
    assert!(issue(4, "not a possible power loss"));
    assert!(issue(4, "decimal separator"));
    // Rows are numbered as the spreadsheet shows them: header is row 1.
    assert_eq!(p.rows[0].line, 2);
    assert_eq!(p.parts.len(), 3);
    assert_eq!(p.counts().skipped, 3);
}

#[test]
fn what_the_file_does_not_say_stays_unknown_and_is_counted() {
    let sheet = csv_sheet(
        "Maker,Order number,Description,Power loss [W]\nExample Maker,C-1,Relay,1.5\nExample Maker,C-2,Relay big,\n",
    );
    let p = preview(&sheet, &Mapping::suggest(&sheet, 0));
    let cov = |label: &str| p.coverage.iter().find(|c| c.label == label).unwrap();
    assert_eq!((cov("Power loss").have, cov("Power loss").total), (1, 2));
    assert_eq!(cov("Dimensions (width, height, depth)").have, 0);
    assert_eq!(cov("Price").have, 0);
    assert!(p.parts[1].price.is_none() && p.parts[1].dimensions.is_none());
    assert!(!p.parts[1].data.contains_key("power_loss_w"));
}

#[test]
fn the_source_of_values_is_recorded_only_when_the_person_says_what_the_file_is() {
    let sheet = csv_sheet("Maker,Order number,Power loss [W]\nExample Maker,D-1,2\n");
    let mut mapping = Mapping::suggest(&sheet, 0);
    let p = preview(&sheet, &mapping);
    assert_eq!(p.parts[0].source_of("power_loss_w"), DataSource::Unknown);
    mapping.origin = DataSource::MakerDatasheet;
    let p = preview(&sheet, &mapping);
    assert_eq!(
        p.parts[0].source_of("power_loss_w"),
        DataSource::MakerDatasheet
    );
    let prov = p.parts[0].provenance.as_ref().unwrap();
    assert_eq!(
        (
            prov.source.as_str(),
            prov.file.as_str(),
            prov.imported.as_str()
        ),
        ("import:table", "parts.csv", "2026-10-21")
    );
    assert!(!prov.redistributable());
}

#[test]
fn a_default_maker_fills_rows_without_one() {
    let sheet = csv_sheet("Order number,Description\nF-1,Relay\n");
    let mut mapping = Mapping::suggest(&sheet, 0);
    assert!(!mapping.maker_known());
    assert_eq!(preview(&sheet, &mapping).counts().skipped, 1);
    mapping.default_manufacturer = "Example Maker".into();
    let p = preview(&sheet, &mapping);
    assert_eq!(p.parts[0].manufacturer, "Example Maker");
}

#[test]
fn mapping_problems_block_the_import_in_words() {
    let sheet = csv_sheet("Maker,Order number,Article\nExample Maker,E-1,E-1b\n");
    let mut mapping = Mapping::suggest(&sheet, 0);
    // "Article" also means order number: the suggestion keeps it as data.
    assert!(matches!(target_of(&mapping, "Article"), Target::Custom(_)));
    mapping.columns[2].target = Target::OrderNumber;
    let p = preview(&sheet, &mapping);
    assert!(
        p.problems
            .iter()
            .any(|t| t.contains("both mapped to Order number"))
    );
    assert!(p.parts.is_empty() && !p.can_import());
    mapping.columns[1].target = Target::Ignore;
    mapping.columns[2].target = Target::Ignore;
    assert!(
        mapping
            .problems()
            .iter()
            .any(|t| t.contains("No column is the order number"))
    );
    // A unit that does not fit the field.
    mapping.columns[1].target = Target::OrderNumber;
    mapping.columns[2].target = Target::Measure("rated_current_a");
    mapping.columns[2].unit = "kg".into();
    assert!(
        mapping
            .problems()
            .iter()
            .any(|t| t.contains("does not fit"))
    );
}

#[test]
fn importing_twice_changes_nothing_and_a_new_value_shows_as_changed() {
    let sheet = csv_sheet("Maker,Order number,Power loss [W]\nExample Maker,G-1,2\n");
    let mapping = Mapping::suggest(&sheet, 0);
    let mut catalog = Catalog::new();
    let first = build_preview(&sheet, &mapping, "a.csv", "d", &catalog);
    assert!(first.can_import());
    assert_eq!(first.apply(&mut catalog), 1);
    let again = build_preview(&sheet, &mapping, "a.csv", "d", &catalog);
    assert_eq!(again.counts().unchanged, 1);
    assert!(!again.can_import());
    let newer = csv_sheet("Maker,Order number,Power loss [W]\nExample Maker,G-1,3\n");
    let changed = build_preview(&newer, &mapping, "a.csv", "d", &catalog);
    assert_eq!(changed.counts().changed, 1);
    assert_eq!(changed.merge.changed.len(), 1);
    assert!(changed.summary().contains("1 changed"));
}

#[test]
fn a_terminal_block_with_a_count_gets_its_terminals_with_the_conductor_range() {
    let sheet = csv_sheet(
        "Maker;Order number;Function;Connections;Connection type;Conductor range\nExample Maker;T-1;terminal;2;push-in;0,14 ... 4 mm²\nExample Maker;T-2;contactor;2;screw;1,5 mm²\n",
    );
    let p = preview(&sheet, &Mapping::suggest(&sheet, 0));
    let t = &p.parts[0];
    assert_eq!(t.terminals.len(), 2);
    assert_eq!(t.terminals[0].kind, ConnectionKind::PushIn);
    assert_eq!(t.terminals[0].wire_mm2, Some((0.14, 4.0)));
    // A contactor is not a terminal block: only the data keys, and a single
    // value is the largest conductor with the smallest unknown.
    let c = &p.parts[1];
    assert!(c.terminals.is_empty());
    assert!(c.data.contains_key("wire_max_mm2") && !c.data.contains_key("wire_min_mm2"));
    assert!(
        p.rows[1]
            .issues
            .iter()
            .any(|i| i.message.contains("largest conductor"))
    );
}

#[test]
fn an_excel_workbook_with_a_title_row_and_several_sheets_reads_through_the_same_path() {
    use rust_xlsxwriter::Workbook;
    let mut book = Workbook::new();
    let cover = book.add_worksheet();
    cover.set_name("Cover").unwrap();
    cover
        .write_string(0, 0, "Price list, example only")
        .unwrap();
    let sheet = book.add_worksheet();
    sheet.set_name("Contactors").unwrap();
    sheet
        .write_string(0, 1, "Example Maker price list 2026")
        .unwrap();
    for (c, h) in [
        "Manufacturer",
        "Article number",
        "Description",
        "Rated current [A]",
        "Power loss [W]",
        "Width [mm]",
        "Height [mm]",
        "Depth [mm]",
    ]
    .iter()
    .enumerate()
    {
        sheet.write_string(2, c as u16, *h).unwrap();
    }
    sheet.write_string(3, 0, "Example Maker").unwrap();
    // A long number stored as a number: Excel shows 1.23457E+11.
    sheet.write_number(3, 1, 123_456_789_012.0).unwrap();
    sheet.write_string(3, 2, "Contactor").unwrap();
    sheet.write_number(3, 3, 9.0).unwrap();
    sheet.write_number(3, 4, 1.4).unwrap();
    for (c, v) in [45.0, 85.0, 70.0].iter().enumerate() {
        sheet.write_number(3, 5 + c as u16, *v).unwrap();
    }
    sheet.write_string(4, 0, "Example Maker").unwrap();
    sheet.write_string(4, 1, "XL-2").unwrap();
    sheet.write_number(4, 3, 12.0).unwrap();
    let bytes = book.save_to_buffer().unwrap();
    let file = TableFile::read("list.xlsx", &bytes).unwrap();
    assert_eq!(file.sheets.len(), 2);
    assert_eq!(file.sheets[0].name, "Cover");
    let sheet = &file.sheets[1];
    // The blank row 2 is not kept; the header is the second kept row.
    assert_eq!(guess_header_row(sheet), 1);
    let mapping = Mapping::suggest(sheet, 1);
    assert_eq!(*target_of(&mapping, "Article number"), Target::OrderNumber);
    let p = build_preview(sheet, &mapping, "list.xlsx", "d", &Catalog::new());
    assert_eq!(p.parts.len(), 2, "{:#?}", p.rows);
    assert_eq!(p.parts[0].order_number, "123456789012");
    assert_eq!(
        p.parts[0].data["power_loss_w"],
        novo_core::Value::Float(1.4)
    );
    assert_eq!(
        p.parts[1].data["rated_current_a"],
        novo_core::Value::Float(12.0)
    );
    // Row numbers are the sheet's own: the first part is on row 4.
    assert_eq!(p.rows[0].line, 4);
}

#[test]
fn saved_mappings_come_back_for_the_same_columns() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("maps.json");
    let sheet = csv_sheet("Maker,Order number,Weird col\nExample Maker,H-1,x\n");
    let mut mapping = Mapping::suggest(&sheet, 0);
    mapping.columns[2].target = Target::Ignore;
    mapping.default_manufacturer = "Example Maker".into();
    mapping.origin = DataSource::MakerDatasheet;
    let headers = sheet.rows[0].clone();
    let mut saved = SavedMappings::load(&path);
    assert!(saved.find(&headers).is_none());
    saved.remember(&headers, &mapping);
    saved.save(&path).unwrap();
    let back = SavedMappings::load(&path).find(&headers).unwrap();
    assert_eq!(back.columns[2].target, Target::Ignore);
    assert_eq!(back.origin, DataSource::MakerDatasheet);
    assert_eq!(back.default_manufacturer, "Example Maker");
    // Other columns: nothing saved.
    assert!(
        SavedMappings::load(&path)
            .find(&["A".into(), "B".into()])
            .is_none()
    );
}

#[test]
fn the_text_report_names_every_finding_with_its_row_and_column() {
    let sheet = csv_sheet("Maker,Order number,Width [mm]\nExample Maker,R-1,9999\n");
    let p = preview(&sheet, &Mapping::suggest(&sheet, 0));
    let text = p.render("parts.csv");
    assert!(text.contains("WARNING row 2 R-1 [Width [mm]]"), "{text}");
    assert!(text.contains("FIELD    Dimensions"), "{text}");
}

#[test]
fn a_title_line_above_the_table_is_not_the_header_even_when_it_has_the_delimiter() {
    let sheet = csv_sheet(
        "Price list 2026 (example; not real)\n\nMaker;Order number;Description;Width [mm]\nExample Maker;K-1;Relay;22\n",
    );
    assert_eq!(guess_header_row(&sheet), 1);
    let mapping = Mapping::suggest(&sheet, 1);
    let p = preview(&sheet, &mapping);
    assert_eq!(p.parts.len(), 1);
    // Row numbers are the file's own lines: the part is on line 4.
    assert_eq!(p.rows[0].line, 4);
}

#[test]
fn oversized_tables_are_refused_in_words() {
    let big = vec![b'a'; MAX_FILE_BYTES as usize + 1];
    assert!(matches!(
        TableFile::read("x.csv", &big),
        Err(TableError::TooLarge(_))
    ));
    let rows = "a;b\n".repeat(MAX_ROWS + 1);
    let err = TableFile::read("x.csv", rows.as_bytes()).unwrap_err();
    assert!(err.to_string().contains("rows"), "{err}");
}

#[test]
fn a_workbook_starts_on_the_sheet_that_has_an_order_number_column() {
    let sheet = |name: &str, rows: &[&[&str]]| Sheet {
        name: name.into(),
        rows: rows
            .iter()
            .map(|r| r.iter().map(|c| c.to_string()).collect())
            .collect(),
        lines: Vec::new(),
    };
    let file = TableFile {
        name: "list.xlsx".into(),
        sheets: vec![
            sheet("Cover", &[&["Contents only"]]),
            sheet(
                "Contactors",
                &[&["Maker", "Order number"], &["Example Maker", "EX-1"]],
            ),
        ],
    };
    assert_eq!(file.first_sheet(), 0);
    assert_eq!(file.best_sheet(), 1);
    // No sheet has one: the first with rows, as before.
    let none = TableFile {
        name: "n.xlsx".into(),
        sheets: vec![sheet("A", &[&["x"]]), sheet("B", &[&["y"]])],
    };
    assert_eq!(none.best_sheet(), 0);
}

#[test]
fn excel_unicode_text_is_read_and_a_binary_file_is_refused() {
    let text = "Maker\tOrder number\nExample Maker\tEX-1\n";
    let mut bytes = vec![0xFF, 0xFE];
    for u in text.encode_utf16() {
        bytes.extend(u.to_le_bytes());
    }
    let file = TableFile::read("unicode.txt", &bytes).unwrap();
    assert_eq!(file.sheets[0].rows[1], vec!["Example Maker", "EX-1"]);
    let junk: Vec<u8> = (0..2000u32).map(|i| (i * 37 % 251) as u8).collect();
    assert_eq!(TableFile::read("p.csv", &junk), Err(TableError::NotText));
}

#[test]
fn a_price_header_with_a_currency_after_it_is_the_price() {
    let sheet = csv_sheet("Maker;Order number;Price EUR;Preis USD\nExample Maker;EX-1;12,5;13\n");
    let mapping = Mapping::suggest(&sheet, 0);
    assert_eq!(target_of(&mapping, "Price EUR"), &Target::Price);
    // The second price column is kept as data, nothing is overwritten.
    assert_ne!(target_of(&mapping, "Preis USD"), &Target::Price);
}

#[test]
fn an_order_number_that_excel_turned_into_a_power_of_ten_is_not_imported() {
    let sheet = csv_sheet("Maker;Order number\nExample Maker;1,23E+11\nExample Maker;EX-2\n");
    let mapping = Mapping::suggest(&sheet, 0);
    let p = preview(&sheet, &mapping);
    assert_eq!(p.parts.len(), 1, "{:#?}", p.rows);
    assert_eq!(p.parts[0].order_number, "EX-2");
    assert!(
        p.rows[0]
            .issues
            .iter()
            .any(|i| i.message.contains("power of ten") && i.message.contains("not imported"))
    );
}
