//! Word lists for the import wizard: Dutch and English words that name a motion, a direction,
//! a speed step or a role (E-stop, main, common...). The lists are data files
//! (`data/words-*.txt`), not code, and a user list in the same format can be added on top.

/// A direction of travel. Opposite directions form a pair that one rocker or joystick axis makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dir {
    Up,
    Down,
    Forward,
    Back,
    Left,
    Right,
}

impl Dir {
    pub fn opposite(self) -> Dir {
        match self {
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
            Dir::Forward => Dir::Back,
            Dir::Back => Dir::Forward,
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
        }
    }

    /// The first direction of the pair this one belongs to.
    pub fn pair(self) -> Dir {
        self.min(self.opposite())
    }

    /// Lower-case word used in position names (`up1`, `down2`).
    pub fn word(self) -> &'static str {
        match self {
            Dir::Up => "up",
            Dir::Down => "down",
            Dir::Forward => "forward",
            Dir::Back => "back",
            Dir::Left => "left",
            Dir::Right => "right",
        }
    }

    /// Upper-case part of a signal name.
    pub fn code(self) -> &'static str {
        match self {
            Dir::Up => "UP",
            Dir::Down => "DN",
            Dir::Forward => "FWD",
            Dir::Back => "BACK",
            Dir::Left => "LEFT",
            Dir::Right => "RIGHT",
        }
    }

    fn parse(s: &str) -> Option<Dir> {
        Some(match s {
            "up" => Dir::Up,
            "down" => Dir::Down,
            "forward" => Dir::Forward,
            "back" => Dir::Back,
            "left" => Dir::Left,
            "right" => Dir::Right,
            _ => return None,
        })
    }
}

/// A role that is not a motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Estop,
    Main,
    Stop,
    Common,
    Horn,
    Light,
    Selector,
    /// Dead-man bar or enabling switch: motion only while it is held.
    Deadman,
    /// Weighing tare button.
    Tare,
    /// Coupling switch (two hoists or cranes run as one).
    Coupling,
    /// Relay that releases the fast step for every motion.
    Release,
}

impl Role {
    fn parse(s: &str) -> Option<Role> {
        Some(match s {
            "estop" => Role::Estop,
            "main" => Role::Main,
            "stop" => Role::Stop,
            "common" => Role::Common,
            "horn" => Role::Horn,
            "light" => Role::Light,
            "selector" => Role::Selector,
            "deadman" => Role::Deadman,
            "tare" => Role::Tare,
            "coupling" => Role::Coupling,
            "release" => Role::Release,
            _ => return None,
        })
    }
}

/// What a word means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tag {
    Motion(String),
    Dir(Dir),
    /// A word that names motion and direction at once (hoisten: hoist up).
    Combo(String, Dir),
    /// An abbreviation that names both directions of one motion (H/Z: hoist up and down).
    Pair(String),
    /// 1 slow, 2 fast.
    Speed(u8),
    Role(Role),
    Kind(String),
}

#[derive(Debug, Clone)]
struct Entry {
    tokens: Vec<String>,
    /// The last token is a stem.
    prefix: bool,
    tag: Tag,
}

/// One recognised word or phrase of a text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub tag: Tag,
    /// Index of the first token and number of tokens it covers.
    pub start: usize,
    pub len: usize,
}

/// Dutch and English words, plus whatever the user added.
#[derive(Debug, Clone, Default)]
pub struct WordList {
    entries: Vec<Entry>,
}

/// Lower-case alphanumeric words of a text; every other character separates words.
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

impl WordList {
    /// The English and Dutch lists shipped with the program.
    pub fn builtin() -> WordList {
        let mut list = WordList::default();
        for text in [
            include_str!("../../data/words-en.txt"),
            include_str!("../../data/words-nl.txt"),
        ] {
            let problems = list.extend(text);
            debug_assert!(problems.is_empty(), "built-in word list: {problems:?}");
        }
        list
    }

    /// Adds the lines of `text`. Returns one message per line that could not be read; those
    /// lines are skipped.
    pub fn extend(&mut self, text: &str) -> Vec<String> {
        let mut problems = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((category, words)) = line.split_once(':') else {
                problems.push(format!("line {}: expected `category: words`", i + 1));
                continue;
            };
            let Some(tag) = parse_category(category.trim()) else {
                problems.push(format!(
                    "line {}: unknown category `{}`",
                    i + 1,
                    category.trim()
                ));
                continue;
            };
            for word in words.split(',') {
                let prefix = word.trim().ends_with('*');
                let tokens = tokenize(word);
                if tokens.is_empty() {
                    continue;
                }
                self.entries.push(Entry {
                    tokens,
                    prefix,
                    tag: tag.clone(),
                });
            }
        }
        // Longest phrase first, so "emergency stop" wins over "stop".
        self.entries
            .sort_by_key(|e| std::cmp::Reverse(e.tokens.len()));
        problems
    }

    /// Every word and phrase of `tokens` the list knows, left to right, longest match first.
    pub fn scan(&self, tokens: &[String]) -> Vec<Hit> {
        let mut hits = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            let found = self.entries.iter().find(|e| {
                let n = e.tokens.len();
                i + n <= tokens.len()
                    && e.tokens.iter().enumerate().all(|(k, w)| {
                        let t = &tokens[i + k];
                        if k + 1 == n && e.prefix {
                            t.starts_with(w.as_str())
                        } else {
                            t == w
                        }
                    })
            });
            match found {
                Some(e) => {
                    hits.push(Hit {
                        tag: e.tag.clone(),
                        start: i,
                        len: e.tokens.len(),
                    });
                    i += e.tokens.len();
                }
                None => i += 1,
            }
        }
        hits
    }
}

fn parse_category(s: &str) -> Option<Tag> {
    let mut parts = s.split('.');
    let head = parts.next()?;
    let a = parts.next()?;
    let b = parts.next();
    if parts.next().is_some() {
        return None;
    }
    match (head, b) {
        ("motion", None) => Some(Tag::Motion(a.to_owned())),
        ("dir", None) => Dir::parse(a).map(Tag::Dir),
        ("pair", None) => Some(Tag::Pair(a.to_owned())),
        ("combo", Some(d)) => Dir::parse(d).map(|d| Tag::Combo(a.to_owned(), d)),
        ("speed", None) => match a {
            "slow" => Some(Tag::Speed(1)),
            "fast" => Some(Tag::Speed(2)),
            _ => None,
        },
        ("role", None) => Role::parse(a).map(Tag::Role),
        ("kind", None) => Some(Tag::Kind(a.to_owned())),
        _ => None,
    }
}
