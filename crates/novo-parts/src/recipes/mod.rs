//! Import recipes: what KoperCAD knows about getting a maker's data and
//! reading it, one recipe per maker.
//!
//! A recipe is data plus a few small functions:
//! - **route**: where the user gets the file (page, login, terms), and the
//!   steps shown while the app waits for the download. KoperCAD never signs
//!   in for the user and never fetches by itself (design doc 3a to 3d);
//! - **recognition**: how the maker's order numbers look (a typo is caught
//!   before a wrong part is imported) and how its downloads are named, so
//!   the Downloads watch picks up the right file;
//! - **reading**: [`MakerRecipe::apply`] turns what a BMEcat or CSV file
//!   holds (feature names in English, German or Dutch, with units in the
//!   text) into the structured part data of catalog schema v2: rated
//!   values in SI units tagged "from the maker's data", connection kind and
//!   wire range, classification codes, dimensions, price, lifecycle,
//!   function and provenance. Whatever it cannot map is listed in the
//!   [`RecipeReport`], never guessed.
//!
//! **What is verified.** Routes marked [`Verified::OwnerCapture`] were
//! captured by the owner in a browser. The rest are written from public
//! descriptions and marked [`Verified::Unverified`]; the app says so. The
//! mapping was written from public datasheet wording and is tested on
//! synthetic sample files in each maker's published format
//! (`tests/golden/`), not on a maker's own export files: add a real file
//! with `tests/golden/README.md` and the mapping can be corrected from it.

mod apply;
mod makers;
mod quantity;

pub use apply::{ApplyContext, Mapped, RecipeReport, Skipped, local_keys};
pub use makers::{GENERIC, RECIPES, recipe_for};
pub use quantity::{Dim, Quantity, parse_quantity};

use crate::library::FileKind;
use crate::search::normalize_code;

/// Whether the maker's route has been confirmed by a real download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verified {
    /// The owner captured it in a browser (design doc 3c).
    OwnerCapture,
    /// Described on the maker's or a portal's page; no download made.
    Survey,
    /// Written from public descriptions only.
    Unverified,
}

/// Whether getting the file needs an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Login {
    None,
    Free,
    /// An account that must accept the portal's terms.
    Required,
    Unknown,
}

/// Where the user gets the maker's data.
#[derive(Debug, Clone, Copy)]
pub struct Route {
    /// Name of the page or portal, shown to the user.
    pub portal: &'static str,
    /// Page for a part, `{order}` stands for the order number. `None` when
    /// no address has been confirmed: nothing is guessed.
    pub page: Option<&'static str>,
    /// The maker's home page, opened when there is no part page.
    pub home: &'static str,
    pub login: Login,
    pub verified: Verified,
    /// Terms of the maker's data, when someone has found them.
    pub terms: Option<&'static str>,
}

/// What a part's description says about its function, when the file has no
/// better source. The first matching entry wins.
#[derive(Debug, Clone, Copy)]
pub struct FunctionHint {
    /// Lower-case phrases, any of which in the description or keywords.
    pub phrases: &'static [&'static str],
    pub function: &'static str,
    /// IEC 81346-2 letter codes the part fits.
    pub letters: &'static [&'static str],
}

/// One maker's recipe.
#[derive(Debug, Clone, Copy)]
pub struct MakerRecipe {
    pub manufacturer: &'static str,
    /// Other spellings of the maker's name in files.
    pub aliases: &'static [&'static str],
    pub route: Route,
    /// Steps shown while waiting for the user's download.
    pub steps: &'static [&'static str],
    /// File types this maker's downloads come as, most useful first.
    pub accepts: &'static [FileKind],
    /// Whether the maker is mainly cables and wires (a cross-section is the
    /// cable's own section, not a terminal's wire range).
    pub cable_maker: bool,
    /// Whether `text` looks like one of the maker's order numbers.
    pub order_ok: fn(&str) -> bool,
    /// What the order numbers look like, for the warning text.
    pub order_hint: &'static str,
    /// The part of an order number makers' files may leave out (ABB writes
    /// `1SDA066721R1`, files often carry `1SDA066721`): variants to look for
    /// in a file name besides the number itself.
    pub order_stems: fn(&str) -> Vec<String>,
    /// Lower-case name fragments of catalog packages (`bmecat`, `etim`) the
    /// Downloads watch accepts even when the name has no order number.
    pub package_names: &'static [&'static str],
    pub functions: &'static [FunctionHint],
}

impl MakerRecipe {
    /// A warning when `order` does not look like this maker's numbers. Not an
    /// error: new number schemes appear, so this only asks the user to check.
    pub fn check_order(&self, order: &str) -> Option<String> {
        let t = order.trim();
        if t.is_empty() {
            return Some("The order number is empty.".into());
        }
        if (self.order_ok)(t) {
            None
        } else {
            Some(format!(
                "\u{201c}{t}\u{201d} does not look like a {} order number ({}). \
Check that it is the order number and not a type name.",
                self.manufacturer, self.order_hint
            ))
        }
    }

    /// The page to open for a part: its page, else the maker's home page.
    pub fn page_for(&self, order: &str) -> String {
        match self.route.page {
            Some(p) => p.replace("{order}", order),
            None => self.route.home.to_owned(),
        }
    }

    /// The steps with `{order}` filled in.
    pub fn steps_for(&self, order: &str) -> Vec<String> {
        self.steps
            .iter()
            .map(|s| s.replace("{order}", order))
            .collect()
    }

    /// Codes (see [`normalize_code`]) that, found in a file name, tie the file
    /// to the part: the order number and the stems the maker leaves out.
    pub fn file_codes(&self, order: &str) -> Vec<String> {
        let mut codes = vec![normalize_code(order)];
        for stem in (self.order_stems)(order) {
            let c = normalize_code(&stem);
            if c.len() >= 5 && !codes.contains(&c) {
                codes.push(c);
            }
        }
        codes.retain(|c| !c.is_empty());
        codes
    }

    /// Whether a file name is one of this maker's catalog packages.
    pub fn recognises_package(&self, file_name: &str) -> bool {
        let lower = file_name.to_lowercase();
        self.package_names.iter().any(|p| lower.contains(p))
    }

    /// Whether `kind` is something this maker's downloads come as.
    pub fn accepts_kind(&self, kind: FileKind) -> bool {
        self.accepts.contains(&kind)
    }
}
