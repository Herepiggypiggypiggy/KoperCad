//! The mapping table (`table.toml`) and the text normalisation both sides use.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

const SOURCE: &str = include_str!("table.toml");

/// One row: a KoperCAD symbol and how an EPLAN user names and tags it.
#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    /// Pack-less symbol path, for example `contacts/no`.
    pub symbol: String,
    /// IEC 81346-2 class letters, the usual one first.
    pub letters: Vec<String>,
    /// Device words in English, Dutch and German; the first is the one an
    /// export writes.
    pub names: Vec<String>,
    /// Name of the pin alias set in the table, when the device is commonly
    /// pinned differently from KoperCAD's designations.
    #[serde(default)]
    pub pins: Option<String>,
    /// `names` after [`normalize`], filled in on load.
    #[serde(skip)]
    pub tokens: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct File {
    #[serde(default)]
    pinset: BTreeMap<String, BTreeMap<String, String>>,
    entry: Vec<Entry>,
}

/// The parsed table.
#[derive(Debug)]
pub struct Table {
    pub entries: Vec<Entry>,
    /// Alias sets with upper-cased keys.
    pub pinsets: BTreeMap<String, BTreeMap<String, String>>,
}

impl Table {
    /// The table compiled into the program.
    pub fn builtin() -> &'static Table {
        static TABLE: OnceLock<Table> = OnceLock::new();
        TABLE.get_or_init(|| Table::parse(SOURCE).expect("table.toml is valid"))
    }

    pub fn parse(text: &str) -> Result<Table, toml::de::Error> {
        let file: File = toml::from_str(text)?;
        let mut entries = file.entry;
        for e in &mut entries {
            e.tokens = e.names.iter().map(|n| tokens(n)).collect();
        }
        let pinsets = file
            .pinset
            .into_iter()
            .map(|(k, v)| {
                (
                    k,
                    v.into_iter()
                        .map(|(a, b)| (a.to_uppercase(), b))
                        .collect::<BTreeMap<_, _>>(),
                )
            })
            .collect();
        Ok(Table { entries, pinsets })
    }

    pub fn entry_for(&self, symbol_path: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.symbol == symbol_path)
    }
}

/// Lower-case ASCII words: umlauts and accents folded (`Schütz` is
/// `schuetz`), punctuation turned into spaces, `+` kept (`3p+n`).
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        match c {
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            'ß' => out.push_str("ss"),
            'à' | 'á' | 'â' | 'ã' | 'å' => out.push('a'),
            'è' | 'é' | 'ê' | 'ë' => out.push('e'),
            'ì' | 'í' | 'î' | 'ï' => out.push('i'),
            'ò' | 'ó' | 'ô' | 'õ' => out.push('o'),
            'ù' | 'ú' | 'û' => out.push('u'),
            'ç' => out.push('c'),
            c if c.is_ascii_alphanumeric() || c == '+' => out.push(c),
            _ => out.push(' '),
        }
    }
    out
}

pub fn tokens(text: &str) -> Vec<String> {
    normalize(text)
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// How well `input` names a device called `name` (both tokenised): 100 for
/// the same words, 70 plus the length when the input contains the name
/// (`Contactor coil, 3 pole` contains `contactor coil`), 50 plus the length
/// when the name contains the input, 0 otherwise.
pub fn name_score(input: &[String], name: &[String]) -> u32 {
    if input.is_empty() || name.is_empty() {
        return 0;
    }
    if input == name {
        return 100;
    }
    let contains =
        |long: &[String], short: &[String]| long.windows(short.len()).any(|w| w == short);
    // A one word name inside a longer text counts only as its head noun
    // (`red lamp`, not `weighing cell amplifier` for `cell`); trailing
    // numbers such as a pole count are ignored.
    let head = |name: &[String]| {
        let mut end = input.len();
        while end > 0 && input[end - 1].chars().all(|c| c.is_ascii_digit()) {
            end -= 1;
        }
        end > 0 && input[end - 1] == name[0]
    };
    if input.len() > name.len() && contains(input, name) && (name.len() > 1 || head(name)) {
        70 + name.len() as u32
    } else if name.len() > input.len() && contains(name, input) {
        50 + input.len() as u32
    } else {
        0
    }
}
