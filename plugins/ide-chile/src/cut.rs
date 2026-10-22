//! The CUT tools: Chile's data by commune, province and region, joined by the código único
//! territorial (CUT) however it is written.
//!
//! Most public data of Chile (INE, SINIM, CASEN, Servel, CEAD, DEIS, Mineduc…) comes as a table by
//! commune, keyed by the CUT or by the commune's name. Joining it to the communes' geometry fails
//! for three local reasons: the CSV is semicolon-separated, in Windows-1252 and with decimal
//! commas; the key is written differently on each side (`1101` and `01101`, `Aysén` and `Aisén`,
//! `Coyhaique` and `Coihaique`); and data from before 2018 carries Ñuble's communes under Biobío's
//! codes (and before 2007, Arica y Parinacota's and Los Ríos's under Tarapacá's and Los Lagos's).
//! These tools read such tables and recognise such keys against the division the INE took the
//! Census 2024 on (`communes.rs`, generated from the INE's layer).
//!
//! - **Join Table by CUT** joins a CSV to a layer of communes, provinces or regions.
//! - **Assign Commune to Points** gives points the commune, province and region they fall in.
//! - **Complete CUT** adds the current code and official names to a layer with a code or name field.
//!
//! The names and codes the tools write are the INE's (`CUT`, `COMUNA`, `COD_PROVINCIA`,
//! `PROVINCIA`, `COD_REGION`, `REGION`), so a result lines up with the Census 2024 layers.

use std::collections::HashMap;

use rustgis_plugin::{Map, Request, ToolOutput, Value, geojson, json};

use crate::MANIFEST;
use crate::communes::{COMMUNE_ALIASES, COMMUNES, DATE, OLD_CUTS, PROVINCES, REGION_ALIASES, REGIONS};

/// A commune of the Census 2024 division.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Commune {
    /// The código único territorial, 4 or 5 digits (region, province, commune).
    pub cut: u32,
    /// The name, as the INE spells it, in title case.
    pub name: &'static str,
    pub province: u32,
    pub region: u32,
}

/// A province.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Province {
    pub code: u32,
    pub name: &'static str,
    pub region: u32,
}

/// A region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub code: u32,
    /// The full name (`Libertador General Bernardo O'Higgins`).
    pub name: &'static str,
    /// The usual short name (`O'Higgins`).
    pub short: &'static str,
    /// The Roman numeral (`VI`).
    pub roman: &'static str,
}

/// The level a code or name is at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Level {
    Commune,
    Province,
    Region,
}

impl Level {
    pub const ALL: [Level; 3] = [Level::Commune, Level::Province, Level::Region];

    /// The level a `Level` parameter names; `None` for Auto (or nothing).
    pub fn from_param(s: Option<&str>) -> Option<Level> {
        match s.map(normalize).as_deref() {
            Some("commune" | "comuna" | "comunas" | "communes") => Some(Level::Commune),
            Some("province" | "provincia" | "provincias" | "provinces") => Some(Level::Province),
            Some("region" | "regiones" | "regions") => Some(Level::Region),
            _ => None,
        }
    }

    /// The English word, for messages (translated by the manifest).
    pub fn word(self) -> &'static str {
        match self {
            Level::Commune => "Commune",
            Level::Province => "Province",
            Level::Region => "Region",
        }
    }
}

/// A place in the division: a commune, a province or a region, by code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Place {
    pub level: Level,
    pub code: u32,
}

impl Place {
    pub fn commune(&self) -> Option<&'static Commune> {
        (self.level == Level::Commune).then(|| commune_by_cut(self.code)).flatten()
    }

    pub fn province(&self) -> Option<&'static Province> {
        let code = match self.level {
            Level::Commune => self.commune()?.province,
            Level::Province => self.code,
            Level::Region => return None,
        };
        PROVINCES.iter().find(|p| p.code == code)
    }

    pub fn region(&self) -> Option<&'static Region> {
        let code = match self.level {
            Level::Commune => self.commune()?.region,
            Level::Province => self.province()?.region,
            Level::Region => self.code,
        };
        REGIONS.iter().find(|r| r.code == code)
    }

    /// The INE's fields for the place: `CUT` and `COMUNA` for a commune, then `COD_PROVINCIA`
    /// and `PROVINCIA`, then `COD_REGION` and `REGION`, as far up as the level reaches.
    pub fn properties(&self) -> Vec<(&'static str, Value)> {
        let mut out = Vec::new();
        if let Some(c) = self.commune() {
            out.push(("CUT", json!(c.cut)));
            out.push(("COMUNA", json!(c.name)));
        }
        if let Some(p) = self.province() {
            out.push(("COD_PROVINCIA", json!(p.code)));
            out.push(("PROVINCIA", json!(p.name)));
        }
        if let Some(r) = self.region() {
            out.push(("COD_REGION", json!(r.code)));
            out.push(("REGION", json!(r.name)));
        }
        out
    }

    /// The fields [`Place::properties`] writes at a level, for the features that have no place.
    pub fn field_names(level: Level) -> &'static [&'static str] {
        match level {
            Level::Commune => &["CUT", "COMUNA", "COD_PROVINCIA", "PROVINCIA", "COD_REGION", "REGION"],
            Level::Province => &["COD_PROVINCIA", "PROVINCIA", "COD_REGION", "REGION"],
            Level::Region => &["COD_REGION", "REGION"],
        }
    }
}

fn commune_by_cut(cut: u32) -> Option<&'static Commune> {
    COMMUNES.binary_search_by_key(&cut, |c| c.cut).ok().map(|i| &COMMUNES[i])
}

/// The level a text names itself at: `"Región de Los Lagos"` is a region, not the commune of Los
/// Lagos; `"Comuna de Santiago"` a commune, not the province or the region.
pub fn marked_level(s: &str) -> Option<Level> {
    let mut level = None;
    let mut word = String::new();
    let mut check = |word: &mut String| {
        match word.as_str() {
            "region" | "reg" => level = Some(Level::Region),
            "provincia" | "prov" => level = Some(Level::Province),
            "comuna" | "com" => level = Some(Level::Commune),
            _ => {}
        }
        word.clear();
    };
    for c in s.chars().flat_map(|c| c.to_lowercase()) {
        match c {
            'ó' => word.push('o'),
            c if c.is_alphanumeric() => word.push(c),
            _ => check(&mut word),
        }
    }
    check(&mut word);
    level
}

/// Lower case, without accents, with only letters and digits: `"Río Ibáñez"` → `"rioibanez"`,
/// `"O'Higgins"` → `"ohiggins"`, `"LLAY-LLAY"` → `"llayllay"`. A leading or trailing "región",
/// "provincia" or "comuna", with the "de" or "del" after it, is dropped (`"Región del Biobío"` →
/// `"biobio"`), but not an article, since `"Región de Los Lagos"` needs its "los"; see
/// [`normalize_loose`] for that.
pub fn normalize(s: &str) -> String {
    normalize_with(s, false)
}

/// [`normalize`], also dropping an article after a dropped "región de": `"Región de La
/// Araucanía"` → `"araucania"`.
pub fn normalize_loose(s: &str) -> String {
    normalize_with(s, true)
}

fn normalize_with(s: &str, drop_article: bool) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    for c in s.chars().flat_map(|c| c.to_lowercase()) {
        let mapped = match c {
            'á' | 'à' | 'ä' | 'â' => Some('a'),
            'é' | 'è' | 'ë' | 'ê' => Some('e'),
            'í' | 'ì' | 'ï' | 'î' => Some('i'),
            'ó' | 'ò' | 'ö' | 'ô' => Some('o'),
            'ú' | 'ù' | 'ü' | 'û' => Some('u'),
            'ñ' => Some('n'),
            'ç' => Some('c'),
            c if c.is_ascii_alphanumeric() => Some(c),
            _ => None,
        };
        match mapped {
            Some(c) => word.push(c),
            None if !word.is_empty() => words.push(std::mem::take(&mut word)),
            None => {}
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    // "Región de …", "… Región", "Comuna de …", "Provincia del …", "Reg. Metropolitana".
    let marker = |w: &str| matches!(w, "region" | "reg" | "comuna" | "com" | "provincia" | "prov");
    if words.iter().any(|w| marker(w)) {
        let mut out: Vec<String> = Vec::new();
        let mut after_marker = false;
        for w in words {
            if marker(&w) {
                after_marker = true;
            } else if after_marker
                && (matches!(w.as_str(), "de" | "del") || (drop_article && matches!(w.as_str(), "la" | "las" | "los" | "el")))
            {
                // Dropped. "de" may be followed by an article: stay after the marker.
                after_marker = w == "de" || w == "del";
            } else {
                after_marker = false;
                out.push(w);
            }
        }
        words = out;
    }
    words.concat()
}

/// A CUT written as a number: `"1101"`, `"01101"`, `"1101.0"`, `1101` → `1101`. Nothing else.
pub fn parse_code(v: &Value) -> Option<u32> {
    let s = match v {
        Value::Number(n) => return n.as_f64().filter(|f| f.fract() == 0.0 && *f >= 0.0 && *f < 100_000.0).map(|f| f as u32),
        Value::String(s) => s.trim(),
        _ => return None,
    };
    let s = s.strip_suffix(".0").or_else(|| s.strip_suffix(",0")).unwrap_or(s);
    if s.is_empty() || s.len() > 5 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Whether a value is empty: null, blank, or one of the dashes and "s/i" tables use for no data.
pub fn is_empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => {
            let s = s.trim();
            s.is_empty()
                || matches!(
                    s.to_ascii_lowercase().as_str(),
                    "-" | "--" | "—" | "s/i" | "s.i." | "n/a" | "na" | "nd" | "n.d." | "null" | "nan"
                )
        }
        _ => false,
    }
}

/// How a value was recognised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    /// By its current code.
    Code,
    /// By a code of before 2007 or 2018 (or Antártica's).
    OldCode(u32),
    /// By its official name.
    Name,
    /// By another spelling of its name.
    Alias,
}

/// Why a value wasn't.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    Empty,
    UnknownCode,
    UnknownName,
}

/// The division indexed for lookups: names (normalised) and codes at each level.
pub struct Index {
    communes_by_name: HashMap<String, u32>,
    commune_aliases: HashMap<String, u32>,
    provinces_by_name: HashMap<String, u32>,
    regions_by_name: HashMap<String, u32>,
    region_aliases: HashMap<String, u32>,
    old_cuts: HashMap<u32, u32>,
}

impl Default for Index {
    fn default() -> Index {
        Index::new()
    }
}

impl Index {
    pub fn new() -> Index {
        let mut regions_by_name = HashMap::new();
        for r in REGIONS {
            regions_by_name.insert(normalize(r.name), r.code);
            regions_by_name.insert(normalize(r.short), r.code);
            regions_by_name.insert(normalize(r.roman), r.code);
        }
        Index {
            communes_by_name: COMMUNES.iter().map(|c| (normalize(c.name), c.cut)).collect(),
            commune_aliases: COMMUNE_ALIASES.iter().map(|(a, cut)| (normalize(a), *cut)).collect(),
            provinces_by_name: PROVINCES.iter().map(|p| (normalize(p.name), p.code)).collect(),
            regions_by_name,
            region_aliases: REGION_ALIASES.iter().map(|(a, code)| (normalize(a), *code)).collect(),
            old_cuts: OLD_CUTS.iter().copied().collect(),
        }
    }

    /// The place a value names at a level, and how it was recognised.
    pub fn resolve(&self, v: &Value, level: Level) -> Result<(Place, How), Why> {
        if is_empty(v) {
            return Err(Why::Empty);
        }
        if let Some(code) = parse_code(v) {
            let known = match level {
                Level::Commune => commune_by_cut(code).is_some(),
                Level::Province => PROVINCES.iter().any(|p| p.code == code),
                Level::Region => REGIONS.iter().any(|r| r.code == code),
            };
            if known {
                return Ok((Place { level, code }, How::Code));
            }
            if level == Level::Commune
                && let Some(&now) = self.old_cuts.get(&code)
            {
                return Ok((Place { level, code: now }, How::OldCode(code)));
            }
            return Err(Why::UnknownCode);
        }
        let Value::String(s) = v else { return Err(Why::UnknownName) };
        if marked_level(s).is_some_and(|l| l != level) {
            return Err(Why::UnknownName);
        }
        let strict = normalize(s);
        let loose = normalize_loose(s);
        let (names, aliases): (&HashMap<String, u32>, Option<&HashMap<String, u32>>) = match level {
            Level::Commune => (&self.communes_by_name, Some(&self.commune_aliases)),
            Level::Province => (&self.provinces_by_name, None),
            Level::Region => (&self.regions_by_name, Some(&self.region_aliases)),
        };
        for key in [&strict, &loose] {
            if let Some(&code) = names.get(key) {
                return Ok((Place { level, code }, How::Name));
            }
            if let Some(&code) = aliases.and_then(|a| a.get(key)) {
                return Ok((Place { level, code }, How::Alias));
            }
        }
        Err(Why::UnknownName)
    }

    /// How many of `values` a level recognises, and the level that recognises most (communes
    /// first on a tie: they're the usual case, and a region's "11" is also a province's).
    pub fn best_level(&self, values: &[&Value], only: Option<Level>) -> (Level, usize) {
        let levels: Vec<Level> = match only {
            Some(l) => vec![l],
            None => Level::ALL.to_vec(),
        };
        let mut best = (levels[0], 0);
        for &level in &levels {
            let n = values.iter().filter(|v| self.resolve(v, level).is_ok()).count();
            if n > best.1 {
                best = (level, n);
            }
        }
        best
    }
}

/// The key among `columns` (name → the values in it): the one a level recognises most of; on a
/// tie the lowest level (a layer with communes and their regions is a layer of communes), then
/// codes before names, then the one declared first. `None` when none is recognised at all, or
/// less than half of its non-empty values are.
pub fn detect_key<'a>(index: &Index, columns: &[(&'a str, Vec<&Value>)], only: Option<Level>) -> Option<(&'a str, Level)> {
    let mut best: Option<(&str, Level, usize, bool)> = None;
    for (name, values) in columns {
        let non_empty: Vec<&Value> = values.iter().copied().filter(|v| !is_empty(v)).collect();
        if non_empty.is_empty() {
            continue;
        }
        let (level, n) = index.best_level(&non_empty, only);
        if n == 0 || n * 2 < non_empty.len() {
            continue;
        }
        let codes = non_empty.iter().filter(|v| parse_code(v).is_some()).count() * 2 >= non_empty.len();
        let better = match best {
            None => true,
            Some((_, l, m, c)) => n > m || (n == m && (level < l || (level == l && codes && !c))),
        };
        if better {
            best = Some((name, level, n, codes));
        }
    }
    best.map(|(name, level, _, _)| (name, level))
}

// ---------------------------------------------------------------------------------------------
// Tables

/// A table read from a CSV: its column names and its rows, each cell typed (a number, a string,
/// or null when empty).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    /// Rows that had more cells than the header (a stray separator): their extra cells are
    /// dropped, and the tool says how many rows that happened to.
    pub overlong: usize,
}

impl Table {
    /// The values of a column, by name.
    pub fn column(&self, name: &str) -> Option<Vec<&Value>> {
        let i = self.columns.iter().position(|c| c == name)?;
        Some(self.rows.iter().map(|r| r.get(i).unwrap_or(&Value::Null)).collect())
    }
}

/// Reads a CSV as the public services of Chile export them: UTF-8 (with or without a byte order
/// mark) or Windows-1252; separated by semicolons, commas, tabs or pipes (whichever the header
/// has most of); quoted fields with doubled quotes and line breaks inside; and numbers written
/// `1.234,5` as well as `1234.5` (a column is a number when every value in it is one).
pub fn read_csv(bytes: &[u8]) -> Result<Table, String> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => windows_1252(bytes),
    };
    let sep = detect_separator(header_line(&text));
    let records = parse_records(&text, sep);
    let mut records = records.into_iter().filter(|r| !(r.len() == 1 && r[0].trim().is_empty()));
    let Some(header) = records.next() else { return Ok(Table::default()) };
    let mut columns: Vec<String> = Vec::new();
    for (i, h) in header.iter().enumerate() {
        let mut name = h.trim().to_string();
        if name.is_empty() {
            name = format!("col{}", i + 1);
        }
        let base = name.clone();
        let mut k = 2;
        while columns.contains(&name) {
            name = format!("{base}_{k}");
            k += 1;
        }
        columns.push(name);
    }
    let mut overlong = 0;
    let raw: Vec<Vec<String>> = records
        .map(|mut r| {
            if r.len() > columns.len() && r[columns.len()..].iter().any(|c| !c.trim().is_empty()) {
                overlong += 1;
            }
            r.resize(columns.len(), String::new());
            r
        })
        .collect();
    // Type each column: numbers when every non-empty value is one. With semicolons, the Chilean
    // notation (dots for thousands, a comma for decimals) is tried first.
    let chilean_first = sep == ';';
    let mut rows: Vec<Vec<Value>> = vec![Vec::with_capacity(columns.len()); raw.len()];
    for c in 0..columns.len() {
        let cells: Vec<&str> = raw.iter().map(|r| r[c].trim()).collect();
        let non_empty: Vec<&str> = cells.iter().copied().filter(|s| !is_empty(&Value::String((*s).to_string()))).collect();
        let parsers: [fn(&str) -> Option<f64>; 2] = if chilean_first { [parse_chilean, parse_plain] } else { [parse_plain, parse_chilean] };
        let parser = parsers.iter().find(|p| !non_empty.is_empty() && non_empty.iter().all(|s| p(s).is_some()));
        for (r, cell) in cells.iter().enumerate() {
            let v = if is_empty(&Value::String((*cell).to_string())) {
                Value::Null
            } else if let Some(p) = parser {
                number(p(cell).unwrap_or(0.0))
            } else {
                Value::String((*cell).to_string())
            };
            rows[r].push(v);
        }
    }
    Ok(Table { columns, rows, overlong })
}

/// A JSON number: an integer when it is one.
pub fn number(f: f64) -> Value {
    if f.fract() == 0.0 && f.abs() < 9.0e15 { json!(f as i64) } else { json!(f) }
}

/// `1234.5`, `-12`, `1e3`, `12.5%` → a number.
fn parse_plain(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches('%').trim().trim_start_matches('$').trim();
    if s.is_empty() || s.contains(',') || !s.bytes().any(|b| b.is_ascii_digit()) || leading_zero(s) {
        return None;
    }
    s.parse::<f64>().ok().filter(|f| f.is_finite())
}

/// Whether a number is written with a leading zero (`01101`, `007`): a code, not a quantity,
/// which keeps its column as text.
fn leading_zero(s: &str) -> bool {
    let s = s.trim_start_matches(['-', '+']);
    let int = s.split(['.', ',', 'e', 'E']).next().unwrap_or(s);
    int.len() > 1 && int.starts_with('0')
}

/// `1.234,5`, `1.234`, `12,5`, `-1.234.567`, `12,5%` → a number. A dot is a thousands separator
/// (groups of three digits), a comma the decimal mark.
fn parse_chilean(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches('%').trim().trim_start_matches('$').trim();
    if s.is_empty() || !s.bytes().any(|b| b.is_ascii_digit()) {
        return None;
    }
    let (sign, body) = match s.strip_prefix('-') {
        Some(b) => (-1.0, b),
        None => (1.0, s.strip_prefix('+').unwrap_or(s)),
    };
    let (int, frac) = match body.split_once(',') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    if (int.is_empty() && frac.is_none()) || leading_zero(int) {
        return None;
    }
    let groups: Vec<&str> = int.split('.').collect();
    if groups.len() > 1 && (groups[0].is_empty() || groups[0].len() > 3 || groups[1..].iter().any(|g| g.len() != 3)) {
        return None;
    }
    if !groups.iter().all(|g| g.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    if let Some(f) = frac
        && (f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let mut text = groups.concat();
    if text.is_empty() {
        text.push('0');
    }
    if let Some(f) = frac {
        text.push('.');
        text.push_str(f);
    }
    text.parse::<f64>().ok().map(|f| sign * f).filter(|f| f.is_finite())
}

/// The header line of a delimited text: up to the first line break outside quotes.
fn header_line(text: &str) -> &str {
    let mut quoted = false;
    for (i, c) in text.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '\n' | '\r' if !quoted => return &text[..i],
            _ => {}
        }
    }
    text
}

/// The separator a header line uses: the one of `;`, `,`, tab and `|` it has most of, outside
/// quotes (a comma when it has none).
fn detect_separator(header: &str) -> char {
    let mut counts = [(';', 0), (',', 0), ('\t', 0), ('|', 0)];
    let mut quoted = false;
    for c in header.chars() {
        if c == '"' {
            quoted = !quoted;
        } else if !quoted && let Some(e) = counts.iter_mut().find(|(s, _)| *s == c) {
            e.1 += 1;
        }
    }
    counts.iter().max_by_key(|(_, n)| *n).filter(|(_, n)| *n > 0).map(|(s, _)| *s).unwrap_or(',')
}

/// The records of a delimited text: quoted fields may hold the separator, doubled quotes and
/// line breaks. `\r\n` and `\n` end records.
fn parse_records(text: &str, sep: char) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    let mut record: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' && field.trim().is_empty() {
            field.clear();
            quoted = true;
        } else if c == sep {
            record.push(std::mem::take(&mut field));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            record.push(std::mem::take(&mut field));
            records.push(std::mem::take(&mut record));
        } else {
            field.push(c);
        }
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    records
}

/// Windows-1252 text (what Excel saves as "CSV" on a Chilean Windows) as UTF-8.
fn windows_1252(bytes: &[u8]) -> String {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž', '\u{8F}', '\u{90}', '‘', '’', '“', '”', '•',
        '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}', 'ž', 'Ÿ',
    ];
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9F => HIGH[(b - 0x80) as usize],
            _ => b as char,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Geometry

/// A polygon's rings, with its box, for point-in-polygon tests.
struct Rings {
    bbox: [f64; 4],
    /// Each polygon of a (Multi)Polygon: its outer ring first, then its holes.
    polygons: Vec<Vec<Vec<[f64; 2]>>>,
}

impl Rings {
    fn of(geometry: &Value) -> Option<Rings> {
        fn ring(v: &Value) -> Option<Vec<[f64; 2]>> {
            let pts: Vec<[f64; 2]> = v.as_array()?.iter().filter_map(|p| Some([p.get(0)?.as_f64()?, p.get(1)?.as_f64()?])).collect();
            (pts.len() >= 4).then_some(pts)
        }
        // The outer ring must be sound (a hole must not stand in for it); a bad hole is skipped.
        fn polygon(v: &Value) -> Option<Vec<Vec<[f64; 2]>>> {
            let mut parts = v.as_array()?.iter();
            let mut rings = vec![ring(parts.next()?)?];
            rings.extend(parts.filter_map(ring));
            Some(rings)
        }
        let polygons: Vec<Vec<Vec<[f64; 2]>>> = match geometry.get("type").and_then(Value::as_str)? {
            "Polygon" => vec![polygon(geometry.get("coordinates")?)?],
            "MultiPolygon" => geometry.get("coordinates")?.as_array()?.iter().filter_map(polygon).collect(),
            _ => return None,
        };
        if polygons.is_empty() {
            return None;
        }
        let bbox = geojson::bbox(&geojson::positions(geometry))?;
        Some(Rings { bbox, polygons })
    }

    fn contains(&self, p: [f64; 2]) -> bool {
        let [w, s, e, n] = self.bbox;
        if p[0] < w || p[0] > e || p[1] < s || p[1] > n {
            return false;
        }
        self.polygons.iter().any(|rings| in_ring(&rings[0], p) && !rings[1..].iter().any(|hole| in_ring(hole, p)))
    }
}

/// The polygons' boxes on a coarse grid over them all, so a point is tested against the few
/// polygons whose boxes share its cell rather than every one (a million points against the 345
/// communes would otherwise be a third of a billion box tests).
struct Grid {
    bbox: [f64; 4],
    side: usize,
    /// For each cell, the indexes of the polygons whose boxes reach it.
    cells: Vec<Vec<usize>>,
}

impl Grid {
    fn new(polygons: &[Rings]) -> Option<Grid> {
        let bbox = polygons.iter().map(|r| r.bbox).reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])])?;
        let side = ((polygons.len() as f64).sqrt() * 2.0).ceil().clamp(1.0, 128.0) as usize;
        let mut grid = Grid { bbox, side, cells: vec![Vec::new(); side * side] };
        for (i, r) in polygons.iter().enumerate() {
            let (x0, y0) = grid.cell([r.bbox[0], r.bbox[1]]);
            let (x1, y1) = grid.cell([r.bbox[2], r.bbox[3]]);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    grid.cells[y * side + x].push(i);
                }
            }
        }
        Some(grid)
    }

    /// The cell a position falls in (clamped to the grid).
    fn cell(&self, p: [f64; 2]) -> (usize, usize) {
        let [w, s, e, n] = self.bbox;
        let f = |v: f64, lo: f64, hi: f64| {
            if hi > lo { (((v - lo) / (hi - lo)) * self.side as f64).floor().clamp(0.0, (self.side - 1) as f64) as usize } else { 0 }
        };
        (f(p[0], w, e), f(p[1], s, n))
    }

    /// Whether a position is within the grid's box.
    fn covers(&self, p: [f64; 2]) -> bool {
        let [w, s, e, n] = self.bbox;
        p[0] >= w && p[0] <= e && p[1] >= s && p[1] <= n
    }

    /// The polygons whose boxes share the position's cell.
    fn candidates(&self, p: [f64; 2]) -> &[usize] {
        if !self.covers(p) {
            return &[];
        }
        let (x, y) = self.cell(p);
        &self.cells[y * self.side + x]
    }
}

/// Whether a point is inside a ring (even-odd rule; a point on the edge counts either way).
fn in_ring(ring: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut inside = false;
    let n = ring.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (ring[i], ring[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0];
            if p[0] < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// A point feature's position: a Point's, or a MultiPoint's first.
fn position(geometry: &Value) -> Option<[f64; 2]> {
    let c = geometry.get("coordinates")?;
    let p = match geometry.get("type").and_then(Value::as_str)? {
        "Point" => c,
        "MultiPoint" => c.get(0)?,
        _ => return None,
    };
    Some([p.get(0)?.as_f64()?, p.get(1)?.as_f64()?])
}

// ---------------------------------------------------------------------------------------------
// The tools

/// `s` in the interface language, with its placeholders filled.
fn tr(req: &Request, s: &str, args: &[(&str, &str)]) -> String {
    req.tr(MANIFEST, s, args)
}

/// Fills `{name}` placeholders of an already translated text.
fn fill(text: &str, args: &[(&str, &str)]) -> String {
    let mut out = text.to_string();
    for (k, v) in args {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// The feature's properties as an object (an empty one when it has none).
fn properties(f: &Value) -> Map<String, Value> {
    f.get("properties").and_then(Value::as_object).cloned().unwrap_or_default()
}

/// Writes one of the INE's fields. A value the feature already had there, when it differs, is
/// kept as `<field>_ORIG`: an empty one isn't worth keeping unless `keep_empty` (the key field
/// itself), and an `_ORIG` already there, from an earlier run, is the original and stays.
fn write_field(props: &mut Map<String, Value>, k: &str, v: Value, keep_empty: bool) {
    if let Some(old) = props.get(k).cloned()
        && old != v
        && (keep_empty || !is_empty(&old))
    {
        props.entry(format!("{k}_ORIG")).or_insert(old);
    }
    props.insert(k.to_string(), v);
}

/// The features of the layer a tool parameter names, taken out of the request rather than
/// copied: a tool reads each layer once, and the result is built by moving its features.
fn take_features(req: &mut Request, param: &str) -> Result<Vec<Value>, String> {
    let name = req.str(param).ok_or_else(|| format!("'{param}' is required"))?.to_string();
    let mut layer = req.layers.remove(&name).ok_or_else(|| format!("no layer named '{name}'"))?;
    Ok(match layer.get_mut("features").map(Value::take) {
        Some(Value::Array(features)) => features,
        _ => Vec::new(),
    })
}

/// The fields of a layer's features (the first 500), in order of appearance, with their values
/// as columns.
fn layer_columns(features: &[Value]) -> Vec<(String, Vec<&Value>)> {
    let mut names: Vec<String> = Vec::new();
    for f in features.iter().take(500) {
        if let Some(p) = f.get("properties").and_then(Value::as_object) {
            for k in p.keys() {
                if !names.contains(k) {
                    names.push(k.clone());
                }
            }
        }
    }
    names
        .into_iter()
        .map(|n| {
            let values = features.iter().take(5000).map(|f| f.get("properties").and_then(|p| p.get(&n)).unwrap_or(&Value::Null)).collect();
            (n, values)
        })
        .collect()
}

/// The key field of a layer and its level: the one the user named, or the one detected.
fn layer_key(
    req: &Request,
    index: &Index,
    features: &[Value],
    named: Option<&str>,
    only: Option<Level>,
) -> Result<Option<(String, Level)>, String> {
    let columns = layer_columns(features);
    if let Some(name) = named.map(str::trim).filter(|s| !s.is_empty()) {
        let Some((_, values)) = columns.iter().find(|(n, _)| n == name) else {
            return Err(tr(req, "The layer has no field '{name}'.", &[("name", name)]));
        };
        let non_empty: Vec<&Value> = values.iter().copied().filter(|v| !is_empty(v)).collect();
        let (level, n) = index.best_level(&non_empty, only);
        return Ok(Some((name.to_string(), if n == 0 { only.unwrap_or(Level::Commune) } else { level })));
    }
    let refs: Vec<(&str, Vec<&Value>)> = columns.iter().map(|(n, v)| (n.as_str(), v.clone())).collect();
    Ok(detect_key(index, &refs, only).map(|(n, l)| (n.to_string(), l)))
}

/// The credit line every run ends with.
fn credit(req: &Request) -> String {
    tr(req, "Division: INE, Census 2024 ({n} communes); table checked on {date}.", &[("n", &COMMUNES.len().to_string()), ("date", DATE)])
}

/// The reason a value wasn't recognised, in the interface language.
fn why(words: &Words, w: Why) -> String {
    match w {
        Why::Empty => words.empty.clone(),
        Why::UnknownCode => words.unknown_code.clone(),
        Why::UnknownName => words.unknown_name.clone(),
    }
}

/// How a value was recognised, in the interface language, when not plainly.
fn how(words: &Words, h: How, value: &Value) -> Option<String> {
    match h {
        How::Code | How::Name => None,
        How::OldCode(old) => Some(fill(&words.old_code, &[("old", &old.to_string())])),
        How::Alias => Some(fill(&words.spelt, &[("name", value.as_str().unwrap_or_default())])),
    }
}

/// The per-row texts, translated once.
struct Words {
    empty: String,
    unknown_code: String,
    unknown_name: String,
    duplicate: String,
    no_feature: String,
    old_code: String,
    spelt: String,
}

impl Words {
    fn new(req: &Request) -> Words {
        Words {
            empty: tr(req, "empty", &[]),
            unknown_code: tr(req, "not a known code", &[]),
            unknown_name: tr(req, "not a known name", &[]),
            duplicate: tr(req, "duplicate of row {n}", &[]),
            no_feature: tr(req, "no feature with this code", &[]),
            old_code: tr(req, "old code {old}", &[]),
            spelt: tr(req, "spelt {name}", &[]),
        }
    }
}

/// Runs the tool `id` with the request.
pub fn run(req: &mut Request) -> Result<Value, String> {
    match req.id.as_str() {
        "join-cut" => join(req),
        "assign-cut" => assign(req),
        "complete-cut" => complete(req),
        other => Err(format!("IDE Chile has no tool '{other}'")),
    }
}

/// Join Table by CUT.
fn join(req: &mut Request) -> Result<Value, String> {
    let layer_name = req.str("layer").unwrap_or("layer").to_string();
    let features = take_features(req, "layer")?;
    let all_features = features.len();
    if features.is_empty() {
        return Err(tr(req, "{name} has no features.", &[("name", &layer_name)]));
    }
    let file = req.take_file("table")?;
    let table = read_csv(&file.bytes)?;
    if table.rows.is_empty() || table.columns.is_empty() {
        return Err(tr(req, "The table is empty.", &[]));
    }
    let only = Level::from_param(req.str("level"));
    let keep_unmatched = req.bool("keep_unmatched").unwrap_or(true);
    let index = Index::new();
    let words = Words::new(req);

    // The layer's key and level.
    let Some((layer_field, level)) = layer_key(req, &index, &features, req.str("layer_key"), only)? else {
        return Err(tr(req, "The layer has no field of CUT codes or names: name it in the key field.", &[]));
    };
    // The table's key column, at that level.
    let table_column = match req.str("table_key").map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => {
            if !table.columns.iter().any(|c| c == name) {
                return Err(tr(req, "The table has no column '{name}'.", &[("name", name)]));
            }
            name.to_string()
        }
        None => {
            let columns: Vec<(&str, Vec<&Value>)> =
                table.columns.iter().map(|c| (c.as_str(), table.column(c).unwrap_or_default())).collect();
            match detect_key(&index, &columns, Some(level)) {
                Some((name, _)) => name.to_string(),
                None => return Err(tr(req, "The table has no column of CUT codes or names: name it in 'Key column in the table'.", &[])),
            }
        }
    };
    let key_i = table.columns.iter().position(|c| *c == table_column).unwrap_or_default();

    // Each row's place; the first row of a place wins.
    let mut by_place: HashMap<Place, usize> = HashMap::new();
    let mut row_notes: Vec<Option<String>> = vec![None; table.rows.len()];
    let mut old_codes = 0;
    let mut aliases = 0;
    for (i, row) in table.rows.iter().enumerate() {
        let key = row.get(key_i).unwrap_or(&Value::Null);
        match index.resolve(key, level) {
            Ok((place, h)) => {
                match h {
                    How::OldCode(_) => old_codes += 1,
                    How::Alias => aliases += 1,
                    _ => {}
                }
                if let Some(&first) = by_place.get(&place) {
                    row_notes[i] = Some(fill(&words.duplicate, &[("n", &(first + 1).to_string())]));
                } else {
                    by_place.insert(place, i);
                }
            }
            Err(w) => row_notes[i] = Some(why(&words, w)),
        }
    }

    // The join.
    let mut used = vec![false; table.rows.len()];
    let mut out_features = Vec::with_capacity(features.len());
    let mut joined = 0;
    let mut features_without = 0;
    // The field each table column lands in, decided once so every feature gets the same ones:
    // its own name, or <name>_2 (then _2_2…) when the layer has a field of that name, another
    // column is called that, or an earlier column already landed there.
    let layer_fields: std::collections::HashSet<&str> =
        features.iter().filter_map(|f| f.get("properties").and_then(Value::as_object)).flat_map(|p| p.keys().map(String::as_str)).collect();
    let mut produced: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut targets: Vec<Option<String>> = Vec::with_capacity(table.columns.len());
    let mut renamed: Vec<String> = Vec::new();
    for (c, name) in table.columns.iter().enumerate() {
        if c == key_i {
            targets.push(None);
            continue;
        }
        let mut field = name.clone();
        while layer_fields.contains(field.as_str())
            || produced.contains(&field)
            || (field != *name && table.columns.iter().enumerate().any(|(o, n)| o != c && o != key_i && *n == field))
        {
            field.push_str("_2");
        }
        if field != *name {
            renamed.push(format!("{name} → {field}"));
        }
        produced.insert(field.clone());
        targets.push(Some(field));
    }
    for mut f in features {
        let mut props = properties(&f);
        let key = props.get(&layer_field).cloned().unwrap_or(Value::Null);
        let row = index.resolve(&key, level).ok().and_then(|(place, _)| by_place.get(&place).copied());
        if row.is_none() {
            features_without += 1;
            if !keep_unmatched {
                continue;
            }
        }
        if let Some(r) = row {
            used[r] = true;
            joined += 1;
        }
        for (c, target) in targets.iter().enumerate() {
            let Some(field) = target else { continue };
            let value = match row {
                Some(r) => table.rows[r].get(c).cloned().unwrap_or(Value::Null),
                None => Value::Null,
            };
            props.insert(field.clone(), value);
        }
        f["properties"] = Value::Object(props);
        out_features.push(f);
    }

    // The rows no feature took.
    let mut unmatched_rows = Vec::new();
    for (i, row) in table.rows.iter().enumerate() {
        if used[i] {
            continue;
        }
        let note = row_notes[i].clone().unwrap_or_else(|| words.no_feature.clone());
        let mut props = Map::new();
        props.insert(tr(req, "Row", &[]), json!(i + 1));
        props.insert(tr(req, "Reason", &[]), json!(note));
        for (c, name) in table.columns.iter().enumerate() {
            // A column of the table called like those two keeps its own name with _2.
            let mut field = name.clone();
            while props.contains_key(&field) {
                field.push_str("_2");
            }
            props.insert(field, row.get(c).cloned().unwrap_or(Value::Null));
        }
        unmatched_rows.push(geojson::feature(Value::Null, Value::Object(props)));
    }

    let total = table.rows.len();
    let level_word = tr(req, level.word(), &[]);
    let mut out = ToolOutput::new()
        .layer(&format!("{layer_name} ({})", tr(req, "Joined", &[])), geojson::collection(out_features))
        .message(tr(
            req,
            "Level: {level}. Key field in the layer: {field}. Key column in the table: {column}.",
            &[("level", &level_word), ("field", &layer_field), ("column", &table_column)],
        ))
        .message(tr(
            req,
            "{n} of {total} rows joined; {features} of {all} features got a row.",
            &[
                ("n", &(total - unmatched_rows.len()).to_string()),
                ("total", &total.to_string()),
                ("features", &joined.to_string()),
                ("all", &all_features.to_string()),
            ],
        ));
    if old_codes > 0 {
        out = out.message(tr(req, "{n} old codes translated to today's.", &[("n", &old_codes.to_string())]));
    }
    if aliases > 0 {
        out = out.message(tr(req, "{n} names matched by another spelling.", &[("n", &aliases.to_string())]));
    }
    if !unmatched_rows.is_empty() {
        let n = unmatched_rows.len().to_string();
        out = out.table(&tr(req, "Rows without a feature", &[]), geojson::collection(unmatched_rows)).message(tr(
            req,
            "{n} rows without a feature (see the table 'Rows without a feature').",
            &[("n", &n)],
        ));
    }
    if features_without > 0 {
        let n = features_without.to_string();
        out = out.message(tr(
            req,
            if keep_unmatched { "{n} features without a row." } else { "{n} features without a row were left out." },
            &[("n", &n)],
        ));
    }
    if table.overlong > 0 {
        out = out.message(tr(
            req,
            "{n} rows have more cells than the header; the extra cells were left out.",
            &[("n", &table.overlong.to_string())],
        ));
    }
    if !renamed.is_empty() {
        let list = renamed.join(", ");
        out = out.message(tr(req, "Columns the layer already had a field of were renamed: {list}.", &[("list", &list)]));
    }
    Ok(out.message(credit(req)).into())
}

/// Assign Commune to Points.
fn assign(req: &mut Request) -> Result<Value, String> {
    let points_name = req.str("points").unwrap_or("points").to_string();
    let points = take_features(req, "points")?;
    let total = points.len();
    let polygons = take_features(req, "communes")?;
    let only = Level::from_param(req.str("level"));
    let index = Index::new();
    if polygons.is_empty() {
        return Ok(ToolOutput::new()
            .message(tr(req, "{name} has no features.", &[("name", req.str("communes").unwrap_or("communes"))]))
            .into());
    }
    let Some((field, level)) = layer_key(req, &index, &polygons, req.str("key"), only)? else {
        return Ok(ToolOutput::new()
            .message(tr(req, "The layer has no field of CUT codes or names: name it in the key field.", &[]))
            .into());
    };
    // Each polygon with its place.
    let mut places: Vec<(Rings, Place)> = Vec::with_capacity(polygons.len());
    for f in &polygons {
        let key = f.get("properties").and_then(|p| p.get(&field)).unwrap_or(&Value::Null);
        if let (Ok((place, _)), Some(rings)) = (index.resolve(key, level), f.get("geometry").and_then(Rings::of)) {
            places.push((rings, place));
        }
    }
    let fields = Place::field_names(level);
    let mut out_features = Vec::with_capacity(points.len());
    let mut inside = 0;
    let mut any_overlap = false;
    let (rings, codes): (Vec<Rings>, Vec<Place>) = places.into_iter().unzip();
    let grid = Grid::new(&rings);
    for mut f in points {
        let mut props = properties(&f);
        let p = f.get("geometry").and_then(position);
        let hit = match (p, &grid) {
            (Some(p), Some(grid)) => {
                any_overlap |= grid.covers(p);
                grid.candidates(p).iter().find(|&&i| rings[i].contains(p)).map(|&i| codes[i])
            }
            _ => None,
        };
        let new = match hit {
            Some(place) => {
                inside += 1;
                place.properties()
            }
            None => Vec::new(),
        };
        for k in fields {
            let v = new.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap_or(Value::Null);
            write_field(&mut props, k, v, false);
        }
        f["properties"] = Value::Object(props);
        out_features.push(f);
    }
    let mut out = ToolOutput::new()
        .layer(&format!("{points_name} ({})", tr(req, "With commune", &[])), geojson::collection(out_features))
        .message(tr(req, "Level: {level}. Key field: {field}.", &[("level", &tr(req, level.word(), &[])), ("field", &field)]))
        .message(tr(
            req,
            "{n} of {total} points fall in a polygon; {outside} fall in none.",
            &[("n", &inside.to_string()), ("total", &total.to_string()), ("outside", &(total - inside).to_string())],
        ));
    if inside == 0 && total > 0 && !any_overlap {
        out = out.message(tr(req, "The points and the polygons don't overlap: are they in the same coordinate system?", &[]));
    }
    Ok(out.message(credit(req)).into())
}

/// Complete CUT.
fn complete(req: &mut Request) -> Result<Value, String> {
    let layer_name = req.str("layer").unwrap_or("layer").to_string();
    let features = take_features(req, "layer")?;
    let total = features.len();
    let only = Level::from_param(req.str("level"));
    let index = Index::new();
    let words = Words::new(req);
    if features.is_empty() {
        return Ok(ToolOutput::new().message(tr(req, "{name} has no features.", &[("name", &layer_name)])).into());
    }
    let Some((field, level)) = layer_key(req, &index, &features, req.str("key"), only)? else {
        return Ok(ToolOutput::new()
            .message(tr(req, "The layer has no field of CUT codes or names: name it in the key field.", &[]))
            .into());
    };
    let fields = Place::field_names(level);
    let mut out_features = Vec::with_capacity(features.len());
    let (mut matched, mut old_codes, mut aliases) = (0, 0, 0);
    for mut f in features {
        let mut props = properties(&f);
        let key = props.get(&field).cloned().unwrap_or(Value::Null);
        let mut note = Value::Null;
        let mut new: Vec<(&str, Value)> = Vec::new();
        match index.resolve(&key, level) {
            Ok((place, h)) => {
                matched += 1;
                match h {
                    How::OldCode(_) => old_codes += 1,
                    How::Alias => aliases += 1,
                    _ => {}
                }
                if let Some(n) = how(&words, h, &key) {
                    note = json!(n);
                }
                new = place.properties();
            }
            Err(w) => note = json!(why(&words, w)),
        }
        for k in fields {
            let v = new.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap_or(Value::Null);
            write_field(&mut props, k, v, *k == field);
        }
        props.insert("CUT_NOTA".to_string(), note);
        f["properties"] = Value::Object(props);
        out_features.push(f);
    }
    let mut out = ToolOutput::new()
        .layer(&format!("{layer_name} ({})", tr(req, "Completed", &[])), geojson::collection(out_features))
        .message(tr(req, "Level: {level}. Key field: {field}.", &[("level", &tr(req, level.word(), &[])), ("field", &field)]))
        .message(tr(
            req,
            "{n} of {total} features recognised; {unmatched} not (see the field CUT_NOTA).",
            &[("n", &matched.to_string()), ("total", &total.to_string()), ("unmatched", &(total - matched).to_string())],
        ));
    if old_codes > 0 {
        out = out.message(tr(req, "{n} old codes translated to today's.", &[("n", &old_codes.to_string())]));
    }
    if aliases > 0 {
        out = out.message(tr(req, "{n} names matched by another spelling.", &[("n", &aliases.to_string())]));
    }
    Ok(out.message(credit(req)).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_division_is_sound() {
        assert_eq!(COMMUNES.len(), 345);
        assert_eq!(PROVINCES.len(), 56);
        assert_eq!(REGIONS.len(), 16);
        assert!(COMMUNES.windows(2).all(|w| w[0].cut < w[1].cut), "sorted by CUT");
        for c in COMMUNES {
            assert_eq!(c.cut / 100, c.province, "{}", c.name);
            assert_eq!(c.cut / 1000, c.region, "{}", c.name);
            assert!(PROVINCES.iter().any(|p| p.code == c.province && p.region == c.region), "{}", c.name);
        }
        // Names are unique once normalised, so a name resolves to one commune.
        let mut names: Vec<String> = COMMUNES.iter().map(|c| normalize(c.name)).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), COMMUNES.len());
        for (alias, cut) in COMMUNE_ALIASES {
            assert!(commune_by_cut(*cut).is_some(), "{alias}");
            assert!(!names.contains(&normalize(alias)), "{alias} is a commune's name");
        }
        for (old, now) in OLD_CUTS {
            assert!(commune_by_cut(*old).is_none() && commune_by_cut(*now).is_some(), "{old} → {now}");
        }
    }

    #[test]
    fn names_normalise() {
        assert_eq!(normalize("Río Ibáñez"), "rioibanez");
        assert_eq!(normalize("O'HIGGINS"), "ohiggins");
        assert_eq!(normalize("Llay-Llay"), "llayllay");
        assert_eq!(normalize("  Ñuble "), "nuble");
        assert_eq!(normalize("Región del Biobío"), "biobio");
        assert_eq!(normalize("Región de Los Lagos"), "loslagos");
        assert_eq!(normalize("XIII Región"), "xiii");
        assert_eq!(normalize("Comuna de La Florida"), "laflorida");
        assert_eq!(normalize("Provincia de Cautín"), "cautin");
        assert_eq!(normalize("Región de La Araucanía"), "laaraucania");
        assert_eq!(normalize_loose("Región de La Araucanía"), "araucania");
        assert_eq!(normalize("La Reina"), "lareina");
        assert_eq!(marked_level("Región de Los Lagos"), Some(Level::Region));
        assert_eq!(marked_level("REGIÓN METROPOLITANA"), Some(Level::Region));
        assert_eq!(marked_level("Prov. de Cautín"), Some(Level::Province));
        assert_eq!(marked_level("comuna de Santiago"), Some(Level::Commune));
        assert_eq!(marked_level("Los Lagos"), None);
        assert_eq!(marked_level("Comodoro"), None);
        assert_eq!(parse_code(&json!("01101")), Some(1101));
        assert_eq!(parse_code(&json!("1101.0")), Some(1101));
        assert_eq!(parse_code(&json!(13101.0)), Some(13101));
        assert_eq!(parse_code(&json!("13-101")), None);
        assert_eq!(parse_code(&json!("Santiago")), None);
        assert_eq!(parse_code(&json!("123456")), None);
    }

    #[test]
    fn values_resolve_at_each_level() {
        let ix = Index::new();
        let c = |v: Value| ix.resolve(&v, Level::Commune);
        assert_eq!(c(json!("01101")), Ok((Place { level: Level::Commune, code: 1101 }, How::Code)));
        assert_eq!(c(json!(13101)), Ok((Place { level: Level::Commune, code: 13101 }, How::Code)));
        assert_eq!(c(json!("Coyhaique")), Ok((Place { level: Level::Commune, code: 11101 }, How::Name)));
        assert_eq!(c(json!("COIHAIQUE")), Ok((Place { level: Level::Commune, code: 11101 }, How::Alias)));
        assert_eq!(c(json!("Aisén")), Ok((Place { level: Level::Commune, code: 11201 }, How::Alias)));
        assert_eq!(c(json!("Puerto Natales")), Ok((Place { level: Level::Commune, code: 12401 }, How::Alias)));
        assert_eq!(c(json!("La Calera")), Ok((Place { level: Level::Commune, code: 5502 }, How::Alias)));
        assert_eq!(c(json!("Treguaco")), Ok((Place { level: Level::Commune, code: 16207 }, How::Alias)));
        assert_eq!(c(json!("Alto Bío Bío")), Ok((Place { level: Level::Commune, code: 8314 }, How::Name)));
        assert_eq!(c(json!("Comuna de Til Til")), Ok((Place { level: Level::Commune, code: 13303 }, How::Name)));
        assert_eq!(c(json!("Los Lagos")).unwrap().0.code, 14104);
        assert_eq!(c(json!("Región de Los Lagos")), Err(Why::UnknownName), "a region, said so");
        // Florida (Biobío) and La Florida (Santiago) are two communes.
        assert_eq!(c(json!("Florida")).unwrap().0.code, 8104);
        assert_eq!(c(json!("La Florida")).unwrap().0.code, 13110);
        // Old codes: Chillán under Biobío, Valdivia under Los Lagos, Arica under Tarapacá.
        assert_eq!(c(json!("08401")), Ok((Place { level: Level::Commune, code: 16101 }, How::OldCode(8401))));
        assert_eq!(c(json!(10501)), Ok((Place { level: Level::Commune, code: 14101 }, How::OldCode(10501))));
        assert_eq!(c(json!("1201")), Ok((Place { level: Level::Commune, code: 15101 }, How::OldCode(1201))));
        assert_eq!(c(json!(12202)), Ok((Place { level: Level::Commune, code: 12201 }, How::OldCode(12202))));
        assert_eq!(c(json!("99999")), Err(Why::UnknownCode));
        assert_eq!(c(json!("Narnia")), Err(Why::UnknownName));
        assert_eq!(c(json!("")), Err(Why::Empty));
        assert_eq!(c(json!("s/i")), Err(Why::Empty));
        assert_eq!(c(Value::Null), Err(Why::Empty));

        let r = |v: Value| ix.resolve(&v, Level::Region).map(|(p, h)| (p.code, h));
        assert_eq!(r(json!("13")), Ok((13, How::Code)));
        assert_eq!(r(json!("RM")), Ok((13, How::Alias)));
        assert_eq!(r(json!("Región Metropolitana de Santiago")), Ok((13, How::Name)));
        assert_eq!(r(json!("XIII")), Ok((13, How::Name)));
        assert_eq!(r(json!("Región del Libertador General Bernardo O'Higgins")), Ok((6, How::Name)));
        assert_eq!(r(json!("O'Higgins")), Ok((6, How::Name)));
        assert_eq!(r(json!("Bío-Bío")), Ok((8, How::Name)));
        assert_eq!(r(json!("Región de La Araucanía")), Ok((9, How::Name)));
        assert_eq!(r(json!("Araucanía")), Ok((9, How::Alias)));
        assert_eq!(r(json!("Aisén")), Ok((11, How::Alias)));
        assert_eq!(r(json!("Magallanes")), Ok((12, How::Name)));
        assert_eq!(r(json!("Los Lagos")), Ok((10, How::Name)));
        assert_eq!(r(json!("17")), Err(Why::UnknownCode));

        let p = |v: Value| ix.resolve(&v, Level::Province).map(|(p, _)| p.code);
        assert_eq!(p(json!("Cautín")), Ok(91));
        assert_eq!(p(json!(131)), Ok(131));
        assert_eq!(p(json!("Provincia de Santiago")), Ok(131));
        assert_eq!(p(json!("Diguillín")), Ok(161));

        let place = Place { level: Level::Commune, code: 16101 };
        let props = place.properties();
        assert_eq!(props[0], ("CUT", json!(16101)));
        assert_eq!(props[1], ("COMUNA", json!("Chillán")));
        assert_eq!(props[2], ("COD_PROVINCIA", json!(161)));
        assert_eq!(props[3], ("PROVINCIA", json!("Diguillín")));
        assert_eq!(props[4], ("COD_REGION", json!(16)));
        assert_eq!(props[5], ("REGION", json!("Ñuble")));
        assert_eq!(Place { level: Level::Region, code: 6 }.properties().len(), 2);
    }

    #[test]
    fn the_level_and_key_are_detected() {
        let ix = Index::new();
        let codes: Vec<Value> = COMMUNES.iter().map(|c| json!(c.cut)).collect();
        let refs: Vec<&Value> = codes.iter().collect();
        assert_eq!(ix.best_level(&refs, None), (Level::Commune, 345));
        let regions: Vec<Value> = (1..=16).map(|c| json!(c.to_string())).collect();
        let refs: Vec<&Value> = regions.iter().collect();
        assert_eq!(ix.best_level(&refs, None), (Level::Region, 16));
        // "Santiago" and "Valdivia" are communes, provinces and (one) a region: communes win.
        let names = [json!("Santiago"), json!("Valdivia"), json!("Concepción")];
        let refs: Vec<&Value> = names.iter().collect();
        assert_eq!(ix.best_level(&refs, None), (Level::Commune, 3));
        assert_eq!(ix.best_level(&refs, Some(Level::Region)), (Level::Region, 1));

        // A table with a name column and a code column: the codes win the tie.
        let noms = [json!("Iquique"), json!("Alto Hospicio"), json!("Pozo Almonte")];
        let cuts = [json!("01101"), json!("01107"), json!("01401")];
        let pop = [json!(223463), json!(129999), json!(18000)];
        let columns = vec![("NOM_COMUNA", noms.iter().collect()), ("CUT", cuts.iter().collect()), ("POBLACION", pop.iter().collect())];
        assert_eq!(detect_key(&ix, &columns, None), Some(("CUT", Level::Commune)));
        let columns = vec![("NOM_COMUNA", noms.iter().collect()), ("POBLACION", pop.iter().collect())];
        assert_eq!(detect_key(&ix, &columns, None), Some(("NOM_COMUNA", Level::Commune)));
        // Nothing of Chile: no key.
        let world = [json!("France"), json!("Peru"), json!(42)];
        let columns = vec![("name", world.iter().collect()), ("pop_est", pop.iter().collect())];
        assert_eq!(detect_key(&ix, &columns, None), None);
    }

    #[test]
    fn chilean_csvs_are_read() {
        // Semicolons, Windows-1252, a BOM-less header, decimal commas and thousands dots.
        let text = "C\u{d3}DIGO;COMUNA;POBLACI\u{d3}N;TASA;NOTA\r\n01101;Iquique;223.463;12,5;\"con; punto y coma\"\r\n5502;\"La Calera\";\"50.554\";-3,25%;s/i\r\n";
        let bytes: Vec<u8> = text.chars().map(|c| if (c as u32) < 256 { c as u32 as u8 } else { b'?' }).collect();
        let t = read_csv(&bytes).unwrap();
        assert_eq!(t.columns, ["CÓDIGO", "COMUNA", "POBLACIÓN", "TASA", "NOTA"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0], vec![json!("01101"), json!("Iquique"), json!(223463), json!(12.5), json!("con; punto y coma")]);
        assert_eq!(t.rows[1], vec![json!("5502"), json!("La Calera"), json!(50554), json!(-3.25), Value::Null]);
        // Codes with a leading zero keep their column as text (parse_code reads both anyway).
        // UTF-8 with a BOM, commas, plain decimals, a quoted line break, an empty header.
        let text = "\u{feff}cut,nombre,valor,\n13101,Santiago,\"1,5\",x\n13110,\"La\nFlorida\",\"2,25\",y\n\n";
        let t = read_csv(text.as_bytes()).unwrap();
        assert_eq!(t.columns, ["cut", "nombre", "valor", "col4"]);
        assert_eq!(t.rows[0], vec![json!(13101), json!("Santiago"), json!(1.5), json!("x")]);
        assert_eq!(t.rows[1], vec![json!(13110), json!("La\nFlorida"), json!(2.25), json!("y")]);
        // A column that mixes notations stays text.
        let t = read_csv("a;v\n1;1,5\n2;2.25\n".as_bytes()).unwrap();
        assert_eq!(t.rows[0], vec![json!(1), json!("1,5")]);
        // Tabs, and duplicate headers.
        let t = read_csv("a\ta\tb\n1\t2\t3\n".as_bytes()).unwrap();
        assert_eq!(t.columns, ["a", "a_2", "b"]);
        assert_eq!(t.rows[0], vec![json!(1), json!(2), json!(3)]);
        assert_eq!(read_csv(b"").unwrap(), Table::default());
        // A row with more cells than the header is counted; the extra cells are dropped.
        let t = read_csv("a;b\n1;2;3\n4;5\n6;7;\n".as_bytes()).unwrap();
        assert_eq!(t.overlong, 1, "a trailing empty cell doesn't count");
        assert_eq!(t.rows[0], vec![json!(1), json!(2)]);
        // Numbers.
        assert_eq!(parse_chilean("1.234.567,89"), Some(1234567.89));
        assert_eq!(parse_chilean("1.234"), Some(1234.0));
        assert_eq!(parse_chilean("12,5"), Some(12.5));
        assert_eq!(parse_chilean("1.23"), None);
        assert_eq!(parse_chilean("12.345,6%"), Some(12345.6));
        assert_eq!(parse_plain("1.23"), Some(1.23));
        assert_eq!(parse_plain("1,5"), None);
        assert_eq!(parse_plain("1e3"), Some(1000.0));
        assert_eq!(parse_plain("abc"), None);
        assert_eq!(parse_plain("01101"), None, "a code, not a number");
        assert_eq!(parse_chilean("01.101"), None);
        assert_eq!(parse_plain("0"), Some(0.0));
        assert_eq!(parse_plain("0.5"), Some(0.5));
        assert_eq!(parse_chilean("0,5"), Some(0.5));
        assert_eq!(detect_separator("a;b;c"), ';');
        assert_eq!(detect_separator("\"a;b\",c"), ',');
        assert_eq!(detect_separator("abc"), ',');
        // A header cell with a line break inside quotes doesn't cut the header short.
        assert_eq!(header_line("\"a\nb\";c\nx;y\n"), "\"a\nb\";c");
        let t = read_csv("\"a\nb\";c\n1;2\n".as_bytes()).unwrap();
        assert_eq!(t.columns, ["a\nb", "c"]);
        assert_eq!(t.rows[0], vec![json!(1), json!(2)]);
    }

    #[test]
    fn points_fall_in_polygons() {
        // A square with a square hole.
        let g = json!({"type": "Polygon", "coordinates": [[[0, 0], [10, 0], [10, 10], [0, 10], [0, 0]], [[4, 4], [6, 4], [6, 6], [4, 6], [4, 4]]]});
        let r = Rings::of(&g).unwrap();
        assert!(r.contains([1.0, 1.0]));
        assert!(!r.contains([5.0, 5.0]), "in the hole");
        assert!(!r.contains([11.0, 5.0]));
        let m = json!({"type": "MultiPolygon", "coordinates": [[[[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]], [[[5, 5], [6, 5], [6, 6], [5, 6], [5, 5]]]]});
        let r = Rings::of(&m).unwrap();
        assert!(r.contains([0.5, 0.5]) && r.contains([5.5, 5.5]) && !r.contains([3.0, 3.0]));
        assert!(Rings::of(&json!({"type": "Point", "coordinates": [0, 0]})).is_none());
        // A degenerate outer ring doesn't let a hole stand in for it; a bad hole is skipped.
        let bad_outer = json!({"type": "Polygon", "coordinates": [[[0, 0], [1, 1]], [[4, 4], [6, 4], [6, 6], [4, 6], [4, 4]]]});
        assert!(Rings::of(&bad_outer).is_none());
        let bad_hole = json!({"type": "Polygon", "coordinates": [[[0, 0], [10, 0], [10, 10], [0, 10], [0, 0]], [[4, 4], [6, 6]]]});
        let r = Rings::of(&bad_hole).unwrap();
        assert_eq!(r.polygons[0].len(), 1);
        assert!(r.contains([5.0, 5.0]));
        // The grid: a point finds the polygon whose box shares its cell, and only that one.
        let squares: Vec<Rings> = (0..400)
            .map(|i| {
                let (x, y) = ((i % 20) as f64 * 2.0, (i / 20) as f64 * 2.0);
                Rings::of(&json!({"type": "Polygon", "coordinates": [[[x, y], [x + 1.0, y], [x + 1.0, y + 1.0], [x, y + 1.0], [x, y]]]}))
                    .unwrap()
            })
            .collect();
        let grid = Grid::new(&squares).unwrap();
        assert_eq!(grid.side, 40);
        let c = grid.candidates([6.5, 4.5]);
        assert!(c.contains(&43) && c.len() <= 4, "{c:?}");
        assert!(squares[43].contains([6.5, 4.5]));
        assert!(grid.candidates([100.0, 100.0]).is_empty() && !grid.covers([100.0, 100.0]));
        assert!(Grid::new(&[]).is_none());
        assert_eq!(position(&json!({"type": "Point", "coordinates": [-70.6, -33.4]})), Some([-70.6, -33.4]));
        assert_eq!(position(&json!({"type": "MultiPoint", "coordinates": [[1, 2], [3, 4]]})), Some([1.0, 2.0]));
    }

    fn request(id: &str, params: Value, layers: &[(&str, Value)], files: &[(&str, &str, &[u8])]) -> Request {
        let mut layers_map = Map::new();
        for (name, fc) in layers {
            layers_map.insert(name.to_string(), fc.clone());
        }
        let mut files_map = Map::new();
        for (param, name, bytes) in files {
            files_map.insert(param.to_string(), json!({"name": name, "data": base64(bytes)}));
        }
        Request {
            kind: rustgis_plugin::Kind::Tool,
            id: id.to_string(),
            params,
            layers: layers_map,
            files: files_map,
            lang: "es".to_string(),
            panel: Map::new(),
            panel_changed: Vec::new(),
        }
    }

    fn base64(bytes: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let n = chunk.iter().fold(0u32, |n, &b| (n << 8) | b as u32) << (8 * (3 - chunk.len()));
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    fn square(x: f64, y: f64, props: Value) -> Value {
        geojson::feature(geojson::polygon(&[[x, y], [x + 1.0, y], [x + 1.0, y + 1.0], [x, y + 1.0]]), props)
    }

    #[test]
    fn join_tool_joins_a_chilean_table() {
        let communes = geojson::collection(vec![
            square(0.0, 0.0, json!({"CUT": 1101, "COMUNA": "IQUIQUE"})),
            square(2.0, 0.0, json!({"CUT": 16101, "COMUNA": "CHILLÁN"})),
            square(4.0, 0.0, json!({"CUT": 13101, "COMUNA": "SANTIAGO"})),
        ]);
        // Semicolons and decimal commas; an old code, a spelling, an unknown and a duplicate.
        let csv = "Comuna;Ingreso;Tasa\nIquique;1.000.000;10,5\n08401;2.000.000;20,5\nIquique;3;3\nNarnia;4;4\n";
        let mut req = request(
            "join-cut",
            json!({"layer": "Comunas", "table": "datos.csv"}),
            &[("Comunas", communes)],
            &[("table", "datos.csv", csv.as_bytes())],
        );
        let out = run(&mut req).unwrap();
        let layer = &out["layers"][0];
        assert_eq!(layer["name"], "Comunas (Unida)");
        let fs = layer["geojson"]["features"].as_array().unwrap();
        assert_eq!(fs.len(), 3);
        assert_eq!(fs[0]["properties"]["Ingreso"], json!(1000000));
        assert_eq!(fs[0]["properties"]["Tasa"], json!(10.5));
        assert_eq!(fs[1]["properties"]["Ingreso"], json!(2000000), "Chillán by its old code");
        assert_eq!(fs[2]["properties"]["Ingreso"], Value::Null, "Santiago has no row");
        let table = &out["tables"][0];
        assert_eq!(table["name"], "Filas sin entidad");
        let rows = table["geojson"]["features"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["properties"]["Fila"], json!(3));
        assert_eq!(rows[0]["properties"]["Motivo"], json!("duplicado de la fila 1"));
        assert_eq!(rows[1]["properties"]["Motivo"], json!("no es un nombre conocido"));
        let messages: Vec<&str> = out["messages"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert_eq!(messages[0], "Nivel: Comuna. Campo clave en la capa: CUT. Columna clave en la tabla: Comuna.");
        assert_eq!(messages[1], "2 de 4 filas unidas; 2 de 3 entidades recibieron una fila.");
        assert_eq!(messages[2], "1 códigos antiguos traducidos a los vigentes.");
        assert!(messages.iter().any(|m| m.starts_with("2 filas sin entidad")), "{messages:?}");
        assert!(messages.contains(&"1 entidades sin fila."), "{messages:?}");
        assert!(messages.last().unwrap().starts_with("División: INE, Censo 2024 (345 comunas)"));

        // Colliding columns land in the same fields for matched and unmatched features: a column
        // called like a layer field gets _2, and never takes another column's name.
        let communes = geojson::collection(vec![
            square(0.0, 0.0, json!({"codigo": "01101", "n": 1})),
            square(2.0, 0.0, json!({"codigo": "13101", "n": 2})),
        ]);
        let csv = "cut,n,n_2\n1101,7,8\n";
        let mut req =
            request("join-cut", json!({"layer": "L", "table": "t.csv"}), &[("L", communes.clone())], &[("table", "t.csv", csv.as_bytes())]);
        req.lang = "en".to_string();
        let out = run(&mut req).unwrap();
        let fs = out["layers"][0]["geojson"]["features"].as_array().unwrap();
        assert_eq!(fs[0]["properties"], json!({"codigo": "01101", "n": 1, "n_2_2": 7, "n_2": 8}));
        assert_eq!(fs[1]["properties"], json!({"codigo": "13101", "n": 2, "n_2_2": null, "n_2": null}), "the same fields, empty");
        assert_eq!(out["messages"][3], "Columns the layer already had a field of were renamed: n → n_2_2.");

        // Without the unmatched features, with the key named, in English.
        let csv = "cut,n\n1101,7\n";
        let mut req = request(
            "join-cut",
            json!({"layer": "L", "table": "t.csv", "keep_unmatched": false, "layer_key": "codigo", "table_key": "cut", "level": "Commune"}),
            &[("L", communes)],
            &[("table", "t.csv", csv.as_bytes())],
        );
        req.lang = "en".to_string();
        let out = run(&mut req).unwrap();
        let fs = out["layers"][0]["geojson"]["features"].as_array().unwrap();
        assert_eq!(fs.len(), 1);
        assert_eq!(fs[0]["properties"]["n"], json!(1));
        assert_eq!(fs[0]["properties"]["n_2"], json!(7), "a colliding column gets _2");
        assert_eq!(out["messages"][3], "Columns the layer already had a field of were renamed: n → n_2.");
        assert_eq!(out["messages"][2], "1 features without a row were left out.");
        assert_eq!(out["tables"].as_array().unwrap().len(), 0);
        assert_eq!(out["messages"][0], "Level: Commune. Key field in the layer: codigo. Key column in the table: cut.");

        // A table with columns called like the unmatched table's own: those keep their name
        // with _2, the row number and the reason stay.
        let cl = geojson::collection(vec![square(0.0, 0.0, json!({"CUT": 1101}))]);
        let csv = "cut,Fila,Motivo\n1101,x,y\n99999,a,b\n";
        let mut req = request("join-cut", json!({"layer": "C", "table": "t.csv"}), &[("C", cl)], &[("table", "t.csv", csv.as_bytes())]);
        let out = run(&mut req).unwrap();
        let row = &out["tables"][0]["geojson"]["features"][0]["properties"];
        assert_eq!(row["Fila"], json!(2));
        assert_eq!(row["Motivo"], "no es un código conocido");
        assert_eq!(row["Fila_2"], "a");
        assert_eq!(row["Motivo_2"], "b");

        // Errors: no key in the layer, no key in the table, a named column that isn't there.
        let world = geojson::collection(vec![square(0.0, 0.0, json!({"name": "France"}))]);
        let mut req = request(
            "join-cut",
            json!({"layer": "W", "table": "t.csv"}),
            &[("W", world.clone())],
            &[("table", "t.csv", b"cut,n\n1101,7\n")],
        );
        assert_eq!(run(&mut req).unwrap_err(), "La capa no tiene un campo de códigos CUT ni de nombres: indíquelo en el campo clave.");
        let cl = geojson::collection(vec![square(0.0, 0.0, json!({"CUT": 1101}))]);
        let mut req =
            request("join-cut", json!({"layer": "C", "table": "t.csv"}), &[("C", cl.clone())], &[("table", "t.csv", b"a,b\nx,y\n")]);
        assert!(run(&mut req).unwrap_err().starts_with("La tabla no tiene una columna de códigos CUT"));
        let mut req = request(
            "join-cut",
            json!({"layer": "C", "table": "t.csv", "table_key": "zzz"}),
            &[("C", cl)],
            &[("table", "t.csv", b"cut,b\n1101,y\n")],
        );
        assert_eq!(run(&mut req).unwrap_err(), "La tabla no tiene una columna 'zzz'.");
    }

    #[test]
    fn assign_tool_places_points() {
        let communes = geojson::collection(vec![
            square(0.0, 0.0, json!({"CUT": 1101})),
            square(2.0, 0.0, json!({"CUT": "08401"})),
            // A region, which the commune level doesn't know: skipped.
            square(6.0, 0.0, json!({"CUT": "RM"})),
        ]);
        let points = geojson::collection(vec![
            // A point that already says a (wrong) commune keeps it as COMUNA_ORIG.
            geojson::feature(geojson::point(0.5, 0.5), json!({"id": 1, "COMUNA": "Arica"})),
            geojson::feature(geojson::point(2.5, 0.5), json!({"id": 2})),
            geojson::feature(geojson::point(9.0, 9.0), json!({"id": 3})),
        ]);
        let mut req = request("assign-cut", json!({"points": "P", "communes": "C"}), &[("P", points.clone()), ("C", communes)], &[]);
        let out = run(&mut req).unwrap();
        let fs = out["layers"][0]["geojson"]["features"].as_array().unwrap();
        assert_eq!(out["layers"][0]["name"], "P (Con comuna)");
        assert_eq!(fs[0]["properties"]["COMUNA"], "Iquique");
        assert_eq!(fs[0]["properties"]["COMUNA_ORIG"], "Arica");
        assert_eq!(fs[0]["properties"]["REGION"], "Tarapacá");
        assert_eq!(fs[1]["properties"]["CUT"], json!(16101), "by the old code");
        assert_eq!(fs[1]["properties"]["PROVINCIA"], "Diguillín");
        assert_eq!(fs[2]["properties"]["CUT"], Value::Null);
        assert_eq!(fs[2]["properties"]["REGION"], Value::Null);
        assert_eq!(out["messages"][0], "Nivel: Comuna. Campo clave: CUT.");
        assert_eq!(out["messages"][1], "2 de 3 puntos caen en un polígono; 1 no caen en ninguno.");

        // Regions, by name, and points elsewhere (another coordinate system).
        let regions = geojson::collection(vec![square(0.0, 0.0, json!({"Nombre": "Región de Los Lagos"}))]);
        let far = geojson::collection(vec![geojson::feature(geojson::point(-7_000_000.0, -4_000_000.0), json!({}))]);
        let mut req = request("assign-cut", json!({"points": "P", "communes": "R"}), &[("P", far), ("R", regions)], &[]);
        let out = run(&mut req).unwrap();
        assert_eq!(out["messages"][0], "Nivel: Región. Campo clave: Nombre.");
        assert_eq!(out["messages"][2], "Los puntos y los polígonos no se superponen: ¿están en el mismo sistema de coordenadas?");
        let fs = out["layers"][0]["geojson"]["features"].as_array().unwrap();
        assert_eq!(fs[0]["properties"].as_object().unwrap().keys().collect::<Vec<_>>(), ["COD_REGION", "REGION"]);

        // The World Atlas: no key, a message and no layer, not an error.
        let world = geojson::collection(vec![square(0.0, 0.0, json!({"name": "France", "pop_est": 67000000}))]);
        let mut req = request("assign-cut", json!({"points": "P", "communes": "W"}), &[("P", points), ("W", world)], &[]);
        let out = run(&mut req).unwrap();
        assert_eq!(out["layers"].as_array().unwrap().len(), 0);
        assert_eq!(out["messages"][0], "La capa no tiene un campo de códigos CUT ni de nombres: indíquelo en el campo clave.");
    }

    #[test]
    fn complete_tool_adds_codes_and_names() {
        let layer = geojson::collection(vec![
            geojson::feature(geojson::point(0.0, 0.0), json!({"comuna": "Coihaique", "REGION": 11})),
            geojson::feature(geojson::point(0.0, 0.0), json!({"comuna": "8401", "REGION": 8})),
            geojson::feature(geojson::point(0.0, 0.0), json!({"comuna": "Narnia", "REGION": null})),
            geojson::feature(geojson::point(0.0, 0.0), json!({"comuna": "", "REGION": null})),
        ]);
        let mut req = request("complete-cut", json!({"layer": "L"}), &[("L", layer)], &[]);
        let out = run(&mut req).unwrap();
        let fs = out["layers"][0]["geojson"]["features"].as_array().unwrap();
        assert_eq!(out["layers"][0]["name"], "L (Completada)");
        let p = &fs[0]["properties"];
        assert_eq!(p["CUT"], json!(11101));
        assert_eq!(p["COMUNA"], "Coyhaique");
        assert_eq!(p["REGION"], "Aysén del General Carlos Ibáñez del Campo");
        assert_eq!(p["REGION_ORIG"], json!(11), "the numeric REGION is kept aside");
        assert_eq!(p["CUT_NOTA"], "escrito Coihaique");
        assert_eq!(fs[1]["properties"]["CUT"], json!(16101));
        assert_eq!(fs[1]["properties"]["CUT_NOTA"], "código antiguo 8401");
        assert_eq!(fs[2]["properties"]["CUT"], Value::Null);
        assert_eq!(fs[2]["properties"]["CUT_NOTA"], "no es un nombre conocido");
        assert_eq!(fs[3]["properties"]["CUT_NOTA"], "vacío");
        assert_eq!(out["messages"][0], "Nivel: Comuna. Campo clave: comuna.");
        assert_eq!(out["messages"][1], "2 de 4 entidades reconocidas; 2 no (vea el campo CUT_NOTA).");
        assert_eq!(out["messages"][2], "1 códigos antiguos traducidos a los vigentes.");
        assert_eq!(out["messages"][3], "1 nombres reconocidos por otra grafía.");

        // The Census 2017 layer: COMUNA is the code and NOM_COMUNA the name; the code wins, and
        // COMUNA becomes the name, its code kept as COMUNA_ORIG.
        let layer = geojson::collection(vec![geojson::feature(
            geojson::point(0.0, 0.0),
            json!({"COMUNA": 13101, "NOM_COMUNA": "SANTIAGO", "TOTAL_PERS": 404495}),
        )]);
        let mut req = request("complete-cut", json!({"layer": "L", "level": "Comuna"}), &[("L", layer)], &[]);
        let out = run(&mut req).unwrap();
        let p = &out["layers"][0]["geojson"]["features"][0]["properties"];
        assert_eq!(p["CUT"], json!(13101));
        assert_eq!(p["COMUNA"], "Santiago");
        assert_eq!(p["COMUNA_ORIG"], json!(13101));
        assert_eq!(p["TOTAL_PERS"], json!(404495));

        // Run again on the result: the original stays in COMUNA_ORIG, nothing else changes.
        let again = out["layers"][0]["geojson"].clone();
        let mut req = request("complete-cut", json!({"layer": "L"}), &[("L", again)], &[]);
        let out = run(&mut req).unwrap();
        let q = &out["layers"][0]["geojson"]["features"][0]["properties"];
        assert_eq!(q["COMUNA_ORIG"], json!(13101), "the first run's original, not 'Santiago'");
        assert_eq!(q["COMUNA"], "Santiago");
        assert_eq!(q["CUT"], json!(13101));
        assert!(q.get("CUT_ORIG").is_none(), "{q}");

        // The World Atlas: a message, no error.
        let world = geojson::collection(vec![square(0.0, 0.0, json!({"name": "France"}))]);
        let mut req = request("complete-cut", json!({"layer": "W"}), &[("W", world)], &[]);
        let out = run(&mut req).unwrap();
        assert_eq!(out["layers"].as_array().unwrap().len(), 0);
        assert!(out["messages"][0].as_str().unwrap().starts_with("La capa no tiene un campo"));
    }
}
