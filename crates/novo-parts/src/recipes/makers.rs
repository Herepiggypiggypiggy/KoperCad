//! The nine maker recipes and the generic one.

use super::{FunctionHint, Login, MakerRecipe, Route, Verified};
use crate::library::FileKind;
use crate::search::normalize_code;

/// The recipe for a maker, by name or alias (case, spaces and punctuation
/// ignored). `None` for a maker without one: use [`GENERIC`].
pub fn recipe_for(manufacturer: &str) -> Option<&'static MakerRecipe> {
    let m = normalize_code(manufacturer);
    if m.is_empty() {
        return None;
    }
    RECIPES.iter().find(|r| {
        normalize_code(r.manufacturer) == m || r.aliases.iter().any(|a| normalize_code(a) == m)
    })
}

fn no_stems(_: &str) -> Vec<String> {
    Vec::new()
}

fn any_order(t: &str) -> bool {
    !t.is_empty()
}

const COMMON_PACKAGES: &[&str] = &[
    "bmecat",
    "etim",
    "eclass",
    "katalog",
    "catalog",
    "masterdata",
    "price",
    "preis",
    "prijs",
    "artikel",
    "article",
    "product",
    "produkt",
];

const FUNCTIONS: &[FunctionHint] = &[
    FunctionHint {
        phrases: &[
            "terminal block",
            "feed-through",
            "feed through",
            "reihenklemme",
            "durchgangsklemme",
            "rijklem",
            "installation terminal",
        ],
        function: "terminal",
        letters: &["X"],
    },
    FunctionHint {
        phrases: &[
            "motor protection circuit breaker",
            "motor protection circuit-breaker",
            "motorschutzschalter",
            "motorbeveiligingsschakelaar",
        ],
        function: "motor_protection_breaker",
        letters: &["Q", "F"],
    },
    FunctionHint {
        phrases: &[
            "miniature circuit breaker",
            "miniature circuit-breaker",
            "leitungsschutzschalter",
            "installatieautomaat",
            "mcb",
        ],
        function: "miniature_circuit_breaker",
        letters: &["F", "Q"],
    },
    FunctionHint {
        phrases: &["circuit breaker", "leistungsschalter", "moulded case"],
        function: "circuit_breaker",
        letters: &["Q", "F"],
    },
    FunctionHint {
        phrases: &["overload relay", "thermal overload", "motorschutzrelais"],
        function: "overload_relay",
        letters: &["F"],
    },
    FunctionHint {
        phrases: &["contactor", "schütz", "schuetz", "contactor relay"],
        function: "contactor",
        letters: &["K", "Q"],
    },
    FunctionHint {
        phrases: &["power supply", "netzteil", "stromversorgung", "voeding"],
        function: "power_supply",
        letters: &["G", "T"],
    },
    FunctionHint {
        phrases: &["relay", "relais", "koppelrelais"],
        function: "relay",
        letters: &["K"],
    },
    FunctionHint {
        phrases: &["fuse", "sicherung", "zekering"],
        function: "fuse",
        letters: &["F"],
    },
    FunctionHint {
        phrases: &[
            "enclosure",
            "schaltschrank",
            "gehäuse",
            "gehaeuse",
            "cabinet",
            "kast",
        ],
        function: "enclosure",
        letters: &[],
    },
    FunctionHint {
        phrases: &[
            "cordset",
            "connection cable",
            "sensor cable",
            "anschlussleitung",
            "connector",
            "steckverbinder",
        ],
        function: "cable_assembly",
        letters: &["W"],
    },
    FunctionHint {
        phrases: &["cable", "leitung", "ölflex", "oelflex", "kabel", "wire"],
        function: "cable",
        letters: &["W"],
    },
];

/// Used for a maker without its own recipe: the same reading, no route.
pub static GENERIC: MakerRecipe = MakerRecipe {
    manufacturer: "",
    aliases: &[],
    route: Route {
        portal: "the maker's website",
        page: None,
        home: "",
        login: Login::Unknown,
        verified: Verified::Unverified,
        terms: None,
    },
    steps: &[],
    accepts: &[
        FileKind::Bmecat,
        FileKind::Csv,
        FileKind::Step,
        FileKind::Dxf,
        FileKind::Zip,
        FileKind::Edz,
    ],
    cable_maker: false,
    order_ok: any_order,
    order_hint: "",
    order_stems: no_stems,
    package_names: COMMON_PACKAGES,
    functions: FUNCTIONS,
};

fn digits(t: &str, n: std::ops::RangeInclusive<usize>) -> bool {
    n.contains(&t.len()) && t.chars().all(|c| c.is_ascii_digit())
}

// WAGO: `2002-1201`, `750-8212`, `221-413`, `2002-1201/1000-836`.
fn wago_ok(t: &str) -> bool {
    t.split('/').all(|p| {
        p.split_once('-')
            .is_some_and(|(a, b)| digits(a, 3..=4) && digits(b, 3..=4))
    })
}

// Phoenix Contact: seven digits (`3044076`).
fn phoenix_ok(t: &str) -> bool {
    digits(t, 7..=7)
}

// Weidmüller: ten digits (`1020000000`).
fn weidmueller_ok(t: &str) -> bool {
    digits(t, 10..=10)
}

/// Siemens MLFB: digit, two letters, digit, then letters, digits and dashes
/// (`6ES7511-1AK02-0AB0`, `3RT2016-1BB41`, `5SL6106-7`).
fn siemens_ok(t: &str) -> bool {
    let c: Vec<char> = t.chars().filter(|c| !matches!(c, ' ' | '-')).collect();
    c.len() >= 8
        && c.len() <= 20
        && c[0].is_ascii_digit()
        && c[1].is_ascii_uppercase()
        && c[2].is_ascii_uppercase()
        && c[3].is_ascii_digit()
        && c.iter()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// Schneider references: letters and digits, no spaces (`LC1D09BD`, `GV2ME10`,
/// `A9F74210`, `NSX100F`). A type name with spaces fails.
fn schneider_ok(t: &str) -> bool {
    let c: Vec<char> = t.chars().filter(|c| *c != '-').collect();
    (5..=16).contains(&c.len())
        && c.iter()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        && c.iter().any(char::is_ascii_digit)
        && c.iter().any(char::is_ascii_uppercase)
}

/// ABB: digit, three letters, six digits, `R` and the variant
/// (`1SDA066721R1`, `2CDS253001R0164`, `1SAM350000R1001`).
fn abb_ok(t: &str) -> bool {
    let c: Vec<char> = t.chars().collect();
    c.len() >= 12
        && c[0].is_ascii_digit()
        && c[1..4].iter().all(char::is_ascii_uppercase)
        && c[4..10].iter().all(char::is_ascii_digit)
        && c[10] == 'R'
        && c[11..].iter().all(char::is_ascii_digit)
}

fn abb_stems(t: &str) -> Vec<String> {
    // `1SDA066721R1` -> `1SDA066721`
    t.find('R')
        .filter(|&i| i >= 10)
        .map(|i| vec![t[..i].to_owned()])
        .unwrap_or_default()
}

/// Rittal: `3209500`, `3209.500`, optionally with the series prefix
/// (`SK 3209.500`, `TS 8606.000`).
fn rittal_ok(t: &str) -> bool {
    let core = t.rsplit(' ').next().unwrap_or(t);
    let compact: String = core.chars().filter(|c| *c != '.').collect();
    digits(&compact, 7..=7)
}

fn rittal_stems(t: &str) -> Vec<String> {
    let core = t.rsplit(' ').next().unwrap_or(t);
    vec![core.chars().filter(|c| *c != '.').collect()]
}

/// Murrelektronik: digit groups joined by dashes (`7000-12341-6140500`,
/// `85000`).
fn murr_ok(t: &str) -> bool {
    let groups: Vec<&str> = t.split('-').collect();
    groups.len() <= 3
        && groups.iter().all(|g| digits(g, 4..=7))
        && (groups.len() > 1 || digits(t, 5..=5))
}

/// Lapp article numbers: seven or eight digits (`1119303`, `10018019`).
fn lapp_ok(t: &str) -> bool {
    digits(t, 7..=8)
}

pub static RECIPES: &[MakerRecipe] = &[
    MakerRecipe {
        manufacturer: "WAGO",
        aliases: &["WAGO Kontakttechnik", "WAGO GmbH"],
        route: Route {
            portal: "WAGO CAD models (CADENAS PARTcommunity)",
            page: Some("https://wago.partcommunity.com/3d-cad-models/?catalog=wago&part={order}"),
            home: "https://www.wago.com",
            login: Login::Required,
            verified: Verified::OwnerCapture,
            terms: Some("https://www.cadenas.de/terms-of-use-3d-cad-models"),
        },
        steps: &[
            "Sign in to your WAGO account on the page that opens. KoperCAD never signs in for you.",
            "Pick the format STEP (AP214) for {order} and download it.",
            "The ZIP arrives in Downloads and is imported by itself.",
        ],
        accepts: &[
            FileKind::Zip,
            FileKind::Step,
            FileKind::Bmecat,
            FileKind::Csv,
        ],
        cable_maker: false,
        order_ok: wago_ok,
        order_hint: "two groups of three or four digits, such as 2002-1201",
        order_stems: no_stems,
        package_names: COMMON_PACKAGES,
        functions: FUNCTIONS,
    },
    MakerRecipe {
        manufacturer: "Phoenix Contact",
        aliases: &["Phoenix", "Phoenix Contact GmbH"],
        route: Route {
            portal: "Eplan Data Portal (needs an Eplan ID)",
            page: Some("https://dataportal.eplan.com/part-details/PXC.{order}"),
            home: "https://www.phoenixcontact.com",
            login: Login::Required,
            verified: Verified::OwnerCapture,
            terms: None,
        },
        steps: &[
            "Sign in with your Eplan ID on the page that opens. KoperCAD never signs in for you.",
            "Download the DXF data (2D drawing) or add the part to your download list for the EDZ.",
            "The file arrives in Downloads and is imported by itself. An EDZ is asked about first, since its name does not mention the part.",
        ],
        accepts: &[
            FileKind::Edz,
            FileKind::Dxf,
            FileKind::Bmecat,
            FileKind::Csv,
            FileKind::Zip,
            FileKind::Step,
        ],
        cable_maker: false,
        order_ok: phoenix_ok,
        order_hint: "seven digits, such as 3044076",
        order_stems: no_stems,
        package_names: COMMON_PACKAGES,
        functions: FUNCTIONS,
    },
    MakerRecipe {
        manufacturer: "Weidmüller",
        aliases: &["Weidmuller", "Weidmueller", "Weidmüller Interface"],
        route: Route {
            portal: "weidmueller.com product page (Downloads)",
            page: None,
            home: "https://www.weidmueller.com",
            login: Login::Unknown,
            verified: Verified::Unverified,
            terms: None,
        },
        steps: &[
            "Search for {order} on the page that opens and open the product page.",
            "Under Downloads pick the CAD data (STEP or DXF) or the product data (BMEcat/ETIM).",
            "The file arrives in Downloads and is imported by itself.",
        ],
        accepts: &[
            FileKind::Bmecat,
            FileKind::Step,
            FileKind::Dxf,
            FileKind::Zip,
            FileKind::Edz,
            FileKind::Csv,
        ],
        cable_maker: false,
        order_ok: weidmueller_ok,
        order_hint: "ten digits, such as 1020000000",
        order_stems: no_stems,
        package_names: COMMON_PACKAGES,
        functions: FUNCTIONS,
    },
    MakerRecipe {
        manufacturer: "Siemens",
        aliases: &["Siemens AG", "Siemens Industry"],
        route: Route {
            portal: "Siemens Industry Online Support (CAx download)",
            page: None,
            home: "https://support.industry.siemens.com",
            login: Login::Free,
            verified: Verified::Unverified,
            terms: None,
        },
        steps: &[
            "Search for {order} on the page that opens and open its CAx data.",
            "Sign in with your Siemens account if asked. KoperCAD never signs in for you.",
            "Download the STEP, DXF or EDZ file. It is imported when it arrives in Downloads.",
        ],
        accepts: &[
            FileKind::Edz,
            FileKind::Step,
            FileKind::Dxf,
            FileKind::Zip,
            FileKind::Bmecat,
            FileKind::Csv,
        ],
        cable_maker: false,
        order_ok: siemens_ok,
        order_hint: "starts with a digit and two letters, such as 3RT2016-1BB41",
        order_stems: no_stems,
        package_names: COMMON_PACKAGES,
        functions: FUNCTIONS,
    },
    MakerRecipe {
        manufacturer: "Schneider Electric",
        aliases: &["Schneider", "Schneider Electric Industries"],
        route: Route {
            portal: "se.com product page (CAD button)",
            page: Some("https://www.se.com/ww/en/product/{order}/"),
            home: "https://www.se.com",
            login: Login::None,
            verified: Verified::Unverified,
            terms: Some("https://www.se.com/my/en/about-us/legal/terms-of-use/"),
        },
        steps: &[
            "On the product page for {order}, use the CAD button and pick STEP or DXF. The page address is not confirmed; search for the number if it does not open the part.",
            "The file arrives in Downloads and is imported by itself.",
            "Schneider limits copying of its site content to personal, non-commercial use; check its CAD terms for commercial projects.",
        ],
        accepts: &[
            FileKind::Step,
            FileKind::Dxf,
            FileKind::Zip,
            FileKind::Bmecat,
            FileKind::Csv,
            FileKind::Edz,
        ],
        cable_maker: false,
        order_ok: schneider_ok,
        order_hint: "letters and digits without spaces, such as LC1D09BD",
        order_stems: no_stems,
        package_names: COMMON_PACKAGES,
        functions: FUNCTIONS,
    },
    MakerRecipe {
        manufacturer: "ABB",
        aliases: &["ABB Stotz-Kontakt", "ABB AG", "ABB Ltd"],
        route: Route {
            portal: "ABB product page",
            page: None,
            home: "https://new.abb.com",
            login: Login::Unknown,
            verified: Verified::Unverified,
            terms: None,
        },
        steps: &[
            "Search for {order} on the page that opens and open the product page.",
            "Use the CAD or Documents tab to download the STEP, DXF or EDZ file.",
            "The file arrives in Downloads and is imported by itself. ABB files often leave off the R-suffix of the number; that is allowed for.",
        ],
        accepts: &[
            FileKind::Step,
            FileKind::Dxf,
            FileKind::Edz,
            FileKind::Zip,
            FileKind::Bmecat,
            FileKind::Csv,
        ],
        cable_maker: false,
        order_ok: abb_ok,
        order_hint: "such as 1SDA066721R1",
        order_stems: abb_stems,
        package_names: COMMON_PACKAGES,
        functions: FUNCTIONS,
    },
    MakerRecipe {
        manufacturer: "Rittal",
        aliases: &["Rittal GmbH", "Rittal GmbH & Co. KG"],
        route: Route {
            portal: "rittal.com product page",
            page: None,
            home: "https://www.rittal.com",
            login: Login::Unknown,
            verified: Verified::Unverified,
            terms: None,
        },
        steps: &[
            "Search for {order} on the page that opens and open the product page.",
            "Download the CAD data (STEP or DXF) or the product data (ETIM/BMEcat).",
            "The file arrives in Downloads and is imported by itself.",
        ],
        accepts: &[
            FileKind::Step,
            FileKind::Dxf,
            FileKind::Edz,
            FileKind::Bmecat,
            FileKind::Zip,
            FileKind::Csv,
        ],
        cable_maker: false,
        order_ok: rittal_ok,
        order_hint: "seven digits, such as 3209500 or 3209.500",
        order_stems: rittal_stems,
        package_names: COMMON_PACKAGES,
        functions: FUNCTIONS,
    },
    MakerRecipe {
        manufacturer: "Murrelektronik",
        aliases: &["Murr Elektronik", "Murr"],
        route: Route {
            portal: "murrelektronik.com product page",
            page: None,
            home: "https://www.murrelektronik.com",
            login: Login::Unknown,
            verified: Verified::Unverified,
            terms: None,
        },
        steps: &[
            "Search for {order} on the page that opens and open the product page.",
            "Download the CAD data or the product data table. A spreadsheet must be saved as CSV (Excel: Save as, CSV) before it can be read.",
            "The file arrives in Downloads and is imported by itself.",
        ],
        accepts: &[
            FileKind::Csv,
            FileKind::Bmecat,
            FileKind::Step,
            FileKind::Dxf,
            FileKind::Zip,
            FileKind::Edz,
        ],
        cable_maker: false,
        order_ok: murr_ok,
        order_hint: "digit groups joined by dashes, such as 7000-12341-6140500, or five digits",
        order_stems: no_stems,
        package_names: COMMON_PACKAGES,
        functions: FUNCTIONS,
    },
    MakerRecipe {
        manufacturer: "Lapp",
        aliases: &["LAPP", "Lapp Kabel", "U.I. Lapp", "Lapp Group"],
        route: Route {
            portal: "lapp.com product page",
            page: None,
            home: "https://lapp.com",
            login: Login::Unknown,
            verified: Verified::Unverified,
            terms: None,
        },
        steps: &[
            "Search for {order} on the page that opens and open the product page.",
            "Download the product data (ETIM/BMEcat or a table) for the cable. A spreadsheet must be saved as CSV first.",
            "The file arrives in Downloads and is imported by itself.",
        ],
        accepts: &[
            FileKind::Bmecat,
            FileKind::Csv,
            FileKind::Zip,
            FileKind::Step,
            FileKind::Dxf,
            FileKind::Edz,
        ],
        cable_maker: true,
        order_ok: lapp_ok,
        order_hint: "seven or eight digits, such as 1119303",
        order_stems: no_stems,
        package_names: COMMON_PACKAGES,
        functions: FUNCTIONS,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn r(name: &str) -> &'static MakerRecipe {
        recipe_for(name).unwrap()
    }

    #[test]
    fn finds_a_recipe_by_name_alias_and_spelling() {
        assert_eq!(r("wago").manufacturer, "WAGO");
        assert_eq!(r("Weidmuller").manufacturer, "Weidmüller");
        assert_eq!(r("weidmueller").manufacturer, "Weidmüller");
        assert_eq!(r("Schneider").manufacturer, "Schneider Electric");
        assert_eq!(r("Murr Elektronik").manufacturer, "Murrelektronik");
        assert!(recipe_for("Unknown Co").is_none());
        assert!(recipe_for("").is_none());
    }

    #[test]
    fn every_recipe_accepts_its_own_known_numbers_and_rejects_type_names() {
        let good = [
            ("WAGO", "2002-1201"),
            ("WAGO", "750-8212"),
            ("WAGO", "2002-1201/1000-836"),
            ("Phoenix Contact", "3044076"),
            ("Weidmüller", "1020000000"),
            ("Siemens", "6ES7511-1AK02-0AB0"),
            ("Siemens", "3RT2016-1BB41"),
            ("Siemens", "6ES7 511-1AK02-0AB0"),
            ("Schneider Electric", "LC1D09BD"),
            ("Schneider Electric", "A9F74210"),
            ("ABB", "1SDA066721R1"),
            ("ABB", "2CDS253001R0164"),
            ("Rittal", "3209500"),
            ("Rittal", "SK 3209.500"),
            ("Murrelektronik", "7000-12341-6140500"),
            ("Murrelektronik", "85000"),
            ("Lapp", "1119303"),
        ];
        for (maker, n) in good {
            assert_eq!(r(maker).check_order(n), None, "{maker} {n}");
        }
        let bad = [
            ("WAGO", "2002"),
            ("Phoenix Contact", "UT 2,5"),
            ("Weidmüller", "WDU 2.5"),
            ("Siemens", "S7-1500"),
            ("Schneider Electric", "Contactor D09"),
            ("ABB", "S201-C16"),
            ("Rittal", "TS 8"),
            ("Murrelektronik", "MICO"),
            ("Lapp", "OLFLEX"),
        ];
        for (maker, n) in bad {
            let w = r(maker).check_order(n);
            assert!(w.is_some_and(|w| w.contains("order number")), "{maker} {n}");
        }
        assert!(r("WAGO").check_order(" ").unwrap().contains("empty"));
    }

    #[test]
    fn file_codes_cover_the_numbers_makers_shorten() {
        assert_eq!(
            r("ABB").file_codes("1SDA066721R1"),
            ["1sda066721r1", "1sda066721"]
        );
        assert_eq!(
            r("Rittal").file_codes("SK 3209.500"),
            ["sk3209500", "3209500"]
        );
        assert_eq!(r("WAGO").file_codes("2002-1201"), ["20021201"]);
    }

    #[test]
    fn page_falls_back_to_the_home_page_and_nothing_is_guessed() {
        assert_eq!(
            r("WAGO").page_for("2002-1201"),
            "https://wago.partcommunity.com/3d-cad-models/?catalog=wago&part=2002-1201"
        );
        assert_eq!(r("Lapp").page_for("1119303"), "https://lapp.com");
        // Only routes captured by the owner are marked verified.
        let verified: Vec<_> = RECIPES
            .iter()
            .filter(|r| r.route.verified == super::super::Verified::OwnerCapture)
            .map(|r| r.manufacturer)
            .collect();
        assert_eq!(verified, ["WAGO", "Phoenix Contact"]);
    }

    #[test]
    fn steps_name_the_part_and_never_ask_the_app_to_sign_in() {
        for recipe in RECIPES {
            let steps = recipe.steps_for("X-1");
            assert!(!steps.is_empty(), "{}", recipe.manufacturer);
            assert!(steps.iter().all(|s| !s.contains("{order}")));
            if recipe.route.login == Login::Required {
                assert!(
                    steps.iter().any(|s| s.contains("never signs in")),
                    "{}",
                    recipe.manufacturer
                );
            }
        }
    }
}
