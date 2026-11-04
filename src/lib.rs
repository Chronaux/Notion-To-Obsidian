use percent_encoding::percent_decode_str;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Cursor, Read},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock,
    },
};
use unicode_normalization::UnicodeNormalization;

pub type Result<T> = std::result::Result<T, String>;
static ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[a-fA-F0-9]{32}").unwrap());
static URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"https?://[^\s<>"\]]+"#).unwrap());
static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[ \t]*(`{3,}|~{3,})(.*)$").unwrap());
static MD_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([a-fA-F0-9]{32})\.md$").unwrap());
pub fn hash(data: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(data.as_ref()))
}
fn rx(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap()
}
fn pretty<T: Serialize>(v: &T) -> String {
    serde_json::to_string_pretty(v).unwrap()
}
fn decode(s: &str) -> String {
    percent_decode_str(s).decode_utf8_lossy().to_string()
}
fn basename(s: &str) -> &str {
    s.rsplit('/').next().unwrap_or(s)
}
fn parent(s: &str) -> &str {
    s.rsplit_once('/').map(|x| x.0).unwrap_or("")
}
fn join(a: &str, b: &str) -> String {
    if a.is_empty() {
        b.into()
    } else {
        format!("{a}/{b}")
    }
}
fn normalize_path(s: &str) -> String {
    let mut parts = vec![];
    for p in s.split('/') {
        if p == ".." {
            parts.pop();
        } else if !p.is_empty() && p != "." {
            parts.push(p);
        }
    }
    parts.join("/")
}
fn is_tag_column(name: &str) -> bool {
    matches!(
        name.trim().to_lowercase().as_str(),
        "тег" | "тэг" | "теги" | "тэги" | "tag" | "tags"
    )
}
fn obsidian_tags(value: &str) -> Vec<String> {
    let mut tags = BTreeSet::new();
    for value in value.split([',', ';', '\n']) {
        let value = value.trim().trim_start_matches('#');
        let mut tag = String::new();
        for ch in value.chars() {
            tag.push(if ch.is_alphanumeric() || "_-/".contains(ch) {
                ch
            } else {
                '_'
            });
        }
        let tag = rx(r"_+")
            .replace_all(&tag, "_")
            .trim_matches(['_', '/'])
            .to_string();
        let tag = tag
            .split('/')
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("/");
        if tag.is_empty() {
            continue;
        }
        let tag = if tag.chars().all(|c| c.is_numeric()) {
            format!("тег_{tag}")
        } else {
            tag
        };
        tags.insert(tag);
    }
    tags.into_iter().collect()
}
fn metadata_tags(text: &str) -> Vec<String> {
    let block = text
        .split_once("\n\n")
        .map(|(_, s)| s.split("\n\n").next().unwrap_or(""))
        .unwrap_or("");
    block
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(key, _)| is_tag_column(key))
        .flat_map(|(_, v)| obsidian_tags(v))
        .collect()
}
fn remove_tag_lines(text: &str, values: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    for (code, chunk) in code_parts(text) {
        if code {
            out.push_str(&chunk);
            continue;
        }
        for line in chunk.split_inclusive('\n') {
            let remove = line
                .trim_end()
                .split_once(':')
                .map(|(k, v)| {
                    is_tag_column(k) && values.get(k).map(|s| norm(s) == norm(v)).unwrap_or(false)
                })
                .unwrap_or(false);
            if !remove {
                out.push_str(line);
            }
        }
    }
    out
}
fn markdown_cell(value: &str) -> String {
    let value = value.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::new();
    let mut cursor = 0;
    let wiki = rx(r"!?\[\[[^\]\n]+\]\]");
    for m in wiki.find_iter(&value) {
        out.push_str(
            &html_escape::encode_text(&value[cursor..m.start()])
                .replace('|', "\\|")
                .replace('`', "\\`"),
        );
        out.push_str(
            &m.as_str()
                .replace("\\|", "|")
                .replace('|', "\\|")
                .replace('`', "\\`"),
        );
        cursor = m.end();
    }
    out.push_str(
        &html_escape::encode_text(&value[cursor..])
            .replace('|', "\\|")
            .replace('`', "\\`"),
    );
    out.split('\n').collect::<Vec<_>>().join("<br>")
}
#[derive(Clone, Debug)]
struct SectionHeading {
    depth: usize,
    title: String,
    start: usize,
    anchor: String,
}
fn section_headings(text: &str) -> Vec<SectionHeading> {
    let mut out = vec![];
    let mut quotes = 0;
    let mut active: Option<(usize, usize, String)> = None;
    let mut stack = Vec::<(usize, String)>::new();
    for (event, range) in
        pulldown_cmark::Parser::new_ext(text, pulldown_cmark::Options::ENABLE_TABLES)
            .into_offset_iter()
    {
        use pulldown_cmark::{Event, Tag, TagEnd};
        match event {
            Event::Start(Tag::BlockQuote(_)) => quotes += 1,
            Event::End(TagEnd::BlockQuote(_)) => quotes -= 1,
            Event::Start(Tag::Heading { level, .. }) if quotes == 0 && level as usize >= 2 => {
                active = Some((level as usize, range.start, String::new()))
            }
            Event::Text(value) | Event::Code(value) if active.is_some() => {
                active.as_mut().unwrap().2.push_str(&value)
            }
            Event::SoftBreak | Event::HardBreak if active.is_some() => {
                active.as_mut().unwrap().2.push(' ')
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, start, title)) = active.take() {
                    let title = html_escape::decode_html_entities(title.trim()).to_string();
                    if title.is_empty() {
                        continue;
                    }
                    while stack.last().map(|h| h.0 >= level).unwrap_or(false) {
                        stack.pop();
                    }
                    stack.push((level, title.clone()));
                    let anchor = stack
                        .iter()
                        .map(|h| h.1.as_str())
                        .collect::<Vec<_>>()
                        .join("#");
                    out.push(SectionHeading {
                        depth: stack.len() - 1,
                        title,
                        start,
                        anchor,
                    });
                }
            }
            _ => {}
        }
    }
    out
}
fn emoji_prefix(line: &str) -> Option<(String, String)> {
    fn icon(c: char) -> bool {
        matches!(c as u32,0x1f000..=0x1faff|0x2600..=0x27bf|0x2300..=0x23ff|0x2b00..=0x2bff|0xfe0f|0x200d|0x2139|0x203c|0x2049)
    }
    let line = line.trim();
    let (bold, line) = if let Some(s) = line.strip_prefix("**") {
        (true, s)
    } else {
        (false, line)
    };
    let end = line
        .char_indices()
        .take_while(|(_, c)| icon(*c))
        .map(|(i, c)| i + c.len_utf8())
        .last()?;
    let symbol = line[..end].to_string();
    let rest = line[end..].trim_start();
    let rest = if bold && rest != "**" && !rest.is_empty() {
        format!("**{rest}")
    } else if bold && rest == "**" {
        String::new()
    } else {
        rest.to_string()
    };
    Some((symbol, rest))
}
fn callout_kind(symbol: &str) -> &'static str {
    if symbol.contains('💡') {
        "tip"
    } else if symbol.chars().any(|c| "❗⚠‼🚨🟠🔴".contains(c)) {
        "warning"
    } else if symbol.chars().any(|c| "⛔🚫🛑".contains(c)) {
        "danger"
    } else if symbol.chars().any(|c| "❓❔".contains(c)) {
        "question"
    } else if symbol.chars().any(|c| "✅☑✔".contains(c)) {
        "success"
    } else if symbol.chars().any(|c| "⭐🌟📌".contains(c)) {
        "important"
    } else if symbol.chars().any(|c| "ℹ📚📖📑🔗".contains(c)) {
        "info"
    } else if symbol.contains('💬') {
        "quote"
    } else {
        "note"
    }
}
fn clean_callout(body: &str) -> (String, String, String) {
    let mut lines = body.trim().lines().map(String::from).collect::<Vec<_>>();
    let mut kind = "note".to_string();
    if let Some(first) = lines.first() {
        if let Some((symbol, rest)) = emoji_prefix(first) {
            kind = callout_kind(&symbol).into();
            lines[0] = rest;
        }
    }
    while lines.first().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.remove(0);
    }
    let mut title = String::new();
    if let Some(first) = lines.first() {
        let line = first.trim();
        let bold = line.starts_with("**") && line.ends_with("**") && line.len() > 4;
        if line.chars().count() <= 120
            && !line.contains("](")
            && !line.contains("[[")
            && (bold || line.ends_with(':'))
        {
            title = if bold {
                line[2..line.len() - 2].trim().into()
            } else {
                line.into()
            };
            lines.remove(0);
            while lines.first().map(|l| l.trim().is_empty()).unwrap_or(false) {
                lines.remove(0);
            }
        }
    }
    (kind, title, lines.join("\n"))
}
fn title_of(s: &str) -> String {
    s.split("\n\n")
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches("# ")
        .to_string()
}
fn norm(s: &str) -> String {
    let s = decode(s);
    let s = html_escape::decode_html_entities(&s);
    let s = rx(r"\[([^\]]+)\]\([^\n]*?\)").replace_all(&s, "$1");
    let s = s.replace("**", "").replace('`', "");
    let s = s
        .split_whitespace()
        .map(|s| s.trim_end_matches('/'))
        .collect::<Vec<_>>()
        .join(" ");
    s.nfkc().collect::<String>().to_lowercase()
}
pub fn safe_name(s: &str) -> String {
    let s: String = s
        .nfc()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*#^[]".contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let s = s.replace("%%", " ");
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut s = s.trim_matches([' ', '.']).to_string();
    if s.is_empty() {
        s = "Без названия".into()
    }
    let head = s.split('.').next().unwrap().to_uppercase();
    if ["CON", "PRN", "AUX", "NUL"].contains(&head.as_str())
        || (head.len() == 4
            && (head.starts_with("COM") || head.starts_with("LPT"))
            && head.ends_with(['1', '2', '3', '4', '5', '6', '7', '8', '9']))
    {
        s = "_".to_string() + &s
    }
    if s.chars().count() > 72 {
        s = format!(
            "{}-{}",
            s.chars().take(63).collect::<String>().trim_end(),
            &hash(&s)[..8]
        )
    }
    s
}
fn clean_component(s: &str) -> String {
    safe_name(&rx(r"\s+[a-fA-F0-9]{32}$").replace(s, ""))
}
fn page_ids(s: &str) -> Vec<String> {
    let s = decode(s);
    let mut result = vec![];
    for m in rx(r"[a-fA-F0-9]{8}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}|[a-fA-F0-9]{32}").find_iter(&s) {
        if s[..m.start()].chars().next_back().map(|c|c.is_ascii_hexdigit()).unwrap_or(false) || s[m.end()..].chars().next().map(|c|c.is_ascii_hexdigit()).unwrap_or(false){continue}
        result.push(m.as_str().replace('-',"").to_lowercase());
    }
    result
}
fn host(url: &str) -> &str {
    url.split_once("://")
        .map(|x| x.1.split(['/', '?', '#']).next().unwrap_or(""))
        .unwrap_or("")
}
fn notion(url: &str) -> bool {
    let h = host(url).to_lowercase();
    [
        "notion.so",
        "www.notion.so",
        "app.notion.com",
        "notion.site",
    ]
    .contains(&h.as_str())
        || h.ends_with(".notion.site")
}
fn split_url(s: &str) -> (String, String) {
    let s = html_escape::decode_html_entities(s);
    let (path, frag) = s.split_once('#').unwrap_or((&s, ""));
    (
        path.split('?').next().unwrap_or(path).to_string(),
        decode(frag),
    )
}
fn code_parts(text: &str) -> Vec<(bool, String)> {
    let mut out = vec![];
    let mut buf = String::new();
    let mut fence: Option<(char, usize)> = None;
    for line in text.split_inclusive('\n') {
        let plain = line
            .trim_start_matches([' ', '\t', '>'])
            .trim_end_matches(['\r', '\n']);
        let m = FENCE.captures(plain);
        if let Some((marker, size)) = fence {
            buf.push_str(line);
            if let Some(m) = m {
                if m[1].starts_with(marker) && m[1].len() >= size && m[2].trim().is_empty() {
                    out.push((true, std::mem::take(&mut buf)));
                    fence = None;
                }
            }
        } else if let Some(m) = m {
            if !buf.is_empty() {
                out.push((false, std::mem::take(&mut buf)));
            }
            fence = Some((m[1].chars().next().unwrap(), m[1].len()));
            buf.push_str(line);
        } else {
            buf.push_str(line)
        }
    }
    if !buf.is_empty() {
        out.push((fence.is_some(), buf));
    }
    out
}
fn inline_parts(s: &str) -> Vec<(bool, &str)> {
    let mut out = vec![];
    let mut start = 0;
    let mut i = 0;
    while i < s.len() {
        if s.as_bytes()[i] == b'`' {
            let mut j = i;
            while j < s.len() && s.as_bytes()[j] == b'`' {
                j += 1;
            }
            let marker = &s[i..j];
            if let Some(end) = s[j..].find(marker) {
                if i > start {
                    out.push((false, &s[start..i]));
                }
                let end = j + end + marker.len();
                out.push((true, &s[i..end]));
                i = end;
                start = end;
                continue;
            }
        }
        i += 1;
    }
    if start < s.len() {
        out.push((false, &s[start..]));
    }
    out
}
fn fenced_contents(text: &str) -> BTreeMap<String, usize> {
    let mut result = BTreeMap::new();
    let mut buffer = String::new();
    let mut active = false;
    for event in pulldown_cmark::Parser::new(text) {
        match event {
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::CodeBlock(
                pulldown_cmark::CodeBlockKind::Fenced(_),
            )) => {
                active = true;
                buffer.clear();
            }
            pulldown_cmark::Event::Text(t) if active => buffer.push_str(&t),
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::CodeBlock) if active => {
                *result.entry(hash(&buffer)).or_default() += 1;
                active = false;
            }
            _ => {}
        }
    }
    result
}

fn fields(text: &str, cols: &[String]) -> BTreeMap<String, String> {
    let body = text.split_once("\n\n").map(|x| x.1).unwrap_or("");
    let mut names = cols.to_vec();
    names.sort_by_key(|s| std::cmp::Reverse(s.len()));
    if names.is_empty() {
        return BTreeMap::new();
    }
    let regex = rx(&format!(
        r"(?m)^({}):[ \t]?",
        names
            .iter()
            .map(|s| regex::escape(s))
            .collect::<Vec<_>>()
            .join("|")
    ));
    let matches: Vec<_> = regex.captures_iter(body).collect();
    let mut out = BTreeMap::new();
    for (i, m) in matches.iter().enumerate() {
        out.insert(
            m[1].to_string(),
            body[m.get(0).unwrap().end()
                ..matches
                    .get(i + 1)
                    .map(|x| x.get(0).unwrap().start())
                    .unwrap_or(body.len())]
                .trim()
                .into(),
        );
    }
    out
}
#[derive(Clone, Default)]
struct Page {
    source: String,
    uid: String,
    title: String,
    text: String,
    target: String,
    db: String,
    row: BTreeMap<String, String>,
    order: usize,
    view_order: usize,
}
#[derive(Clone, Default)]
struct Database {
    source: String,
    uid: String,
    title: String,
    folder: String,
    rows: Vec<BTreeMap<String, String>>,
    columns: Vec<String>,
    view_rows: Vec<BTreeMap<String, String>>,
    view_columns: Vec<String>,
    target: String,
    base: String,
    keys: BTreeMap<String, String>,
    pages: Vec<usize>,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Warning {
    pub kind: String,
    pub source: String,
    pub detail: String,
}
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Missing {
    pub target: String,
    pub urls: Vec<String>,
    pub sources: Vec<String>,
}
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Report {
    pub version: String,
    pub stats: BTreeMap<String, usize>,
    pub warnings: Vec<Warning>,
    pub missing_pages: BTreeMap<String, Missing>,
    pub skipped_pages: BTreeMap<String, Missing>,
    pub databases: Vec<Value>,
    pub mapping: BTreeMap<String, String>,
    pub row_matching: Vec<Value>,
    pub audit: Value,
}
pub struct Converter {
    pages: Vec<Page>,
    dbs: Vec<Database>,
    original: BTreeMap<String, String>,
    targets: BTreeMap<String, String>,
    ids: BTreeMap<String, String>,
    assets: BTreeMap<String, String>,
    used: BTreeSet<String>,
    output: BTreeMap<String, Vec<u8>>,
    section_nodes: BTreeMap<String, Vec<usize>>,
    pub report: Report,
    skip_missing_pages: bool,
    cancel: Arc<AtomicBool>,
    progress: Box<dyn Fn(&str) + Send>,
}
impl Converter {
    pub fn new(progress: impl Fn(&str) + Send + 'static, cancel: Arc<AtomicBool>) -> Self {
        Self {
            pages: vec![],
            dbs: vec![],
            original: BTreeMap::new(),
            targets: BTreeMap::new(),
            ids: BTreeMap::new(),
            assets: BTreeMap::new(),
            used: BTreeSet::new(),
            output: BTreeMap::new(),
            section_nodes: BTreeMap::new(),
            skip_missing_pages: false,
            report: Report {
                version: "0.5.1".into(),
                ..Default::default()
            },
            cancel,
            progress: Box::new(progress),
        }
    }
    /// A Markdown & CSV ZIP does not identify archived pages.
    /// Enable this only when the user has verified that missing pages may be omitted.
    pub fn skip_missing_pages(mut self, skip: bool) -> Self {
        self.skip_missing_pages = skip;
        self
    }
    fn check(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            Err("Преобразование отменено. Итоговая папка не создана.".into())
        } else {
            Ok(())
        }
    }
    fn warn(&mut self, kind: &str, source: &str, detail: &str) {
        self.report.warnings.push(Warning {
            kind: kind.into(),
            source: source.into(),
            detail: detail.into(),
        })
    }
    fn inc(&mut self, key: &str) {
        *self.report.stats.entry(key.into()).or_default() += 1;
    }
    fn reserve(&mut self, path: String, uid: &str) -> String {
        let mut target = path.clone();
        let mut i = 0;
        while self.used.contains(&target.to_lowercase()) {
            i += 1;
            let (stem, ext) = path.rsplit_once('.').unwrap_or((&path, ""));
            let suffix = if uid.is_empty() {
                hash(&path)
            } else {
                uid.into()
            };
            target = format!(
                "{}-{}{}{}",
                stem,
                &suffix[..8],
                if i > 1 {
                    format!("-{i}")
                } else {
                    String::new()
                },
                if ext.is_empty() {
                    String::new()
                } else {
                    format!(".{ext}")
                }
            );
        }
        self.used.insert(target.to_lowercase());
        target
    }
    fn read_archive(&self, path: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
        (self.progress)("Чтение ZIP и проверка файлов…");
        let data = fs::read(path).map_err(|e| e.to_string())?;
        let mut files = BTreeMap::new();
        let mut total = 0;
        self.read_zip(&data, 0, &mut total, &mut files)?;
        if !files.keys().any(|n| n.ends_with(".md")) {
            return Err("Нет Markdown-страниц. Нужен ZIP экспорта Markdown & CSV.".into());
        }
        Ok(files)
    }
    fn read_zip(
        &self,
        data: &[u8],
        depth: usize,
        total: &mut u64,
        files: &mut BTreeMap<String, Vec<u8>>,
    ) -> Result<()> {
        if depth > 3 {
            return Err("Слишком много уровней частей экспорта.".into());
        }
        let mut archive = zip::ZipArchive::new(Cursor::new(data))
            .map_err(|e| format!("Не удалось открыть ZIP: {e}"))?;
        for i in 0..archive.len() {
            self.check()?;
            let mut f = archive.by_index(i).map_err(|e| e.to_string())?;
            if f.is_dir() {
                continue;
            }
            let name = f.name().replace('\\', "/");
            if name.starts_with('/')
                || name.split('/').any(|s| s == "..")
                || rx(r"^[a-zA-Z]:").is_match(&name)
                || f.unix_mode()
                    .map(|m| m & 0o170000 == 0o120000)
                    .unwrap_or(false)
            {
                return Err(format!("Небезопасный путь в архиве: {name}"));
            }
            if name.starts_with("__MACOSX/") || basename(&name) == ".DS_Store" {
                continue;
            }
            *total = total.saturating_add(f.size());
            if *total > 8 * 1024 * 1024 * 1024 || f.size() > 2 * 1024 * 1024 * 1024 {
                return Err("Превышен лимит распаковки: 8 ГБ всего / 2 ГБ на файл.".into());
            }
            let mut bytes = vec![];
            f.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            if rx(r"(?i)(?:^|/)(?:Export[-_][a-f0-9-]{32,36}(?:-Part-\d+)?|Export-Part-\d+|[a-f0-9-]{32,36}-Part-\d+)\.zip$").is_match(&name){self.read_zip(&bytes,depth+1,total,files)?;continue}
            if let Some(old) = files.get(&name) {
                if old != &bytes {
                    return Err(format!("Разные файлы по одному пути: {name}"));
                }
            }
            files.insert(name, bytes);
        }
        Ok(())
    }
    fn read_csv(text: &str) -> Result<(Vec<String>, Vec<BTreeMap<String, String>>)> {
        let mut r = csv::ReaderBuilder::new().from_reader(text.as_bytes());
        let headers = r
            .headers()
            .map_err(|e| e.to_string())?
            .iter()
            .map(String::from)
            .collect::<Vec<_>>();
        if headers.iter().collect::<BTreeSet<_>>().len() != headers.len() {
            return Err("Повторяющиеся имена колонок CSV.".into());
        }
        let mut rows = vec![];
        for row in r.records() {
            let row = row.map_err(|e| e.to_string())?;
            rows.push(
                headers
                    .iter()
                    .cloned()
                    .zip(row.iter().map(String::from))
                    .collect(),
            );
        }
        Ok((headers, rows))
    }
    fn inventory(&mut self, files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
        let mut pageids = BTreeSet::new();
        for (name, data) in files {
            self.check()?;
            if !name.ends_with(".md") && !name.ends_with(".csv") {
                continue;
            }
            let text = String::from_utf8(data.clone())
                .map_err(|_| format!("Не UTF-8: {name}"))?
                .trim_start_matches('\u{feff}')
                .replace("\r\n", "\n");
            self.original.insert(name.clone(), text.clone());
            if name.ends_with(".md") {
                let uid = MD_ID
                    .captures(name)
                    .map(|x| x[1].to_lowercase())
                    .unwrap_or(hash(name)[..32].into());
                if !pageids.insert(uid.clone()) {
                    return Err(format!("Дублирующий UUID страницы: {uid}"));
                }
                self.pages.push(Page {
                    source: name.clone(),
                    uid,
                    title: title_of(&text),
                    text,
                    ..Default::default()
                });
            }
        }
        let full = self
            .original
            .keys()
            .filter(|n| {
                n.ends_with(".csv")
                    && (n.ends_with("_all.csv")
                        || !self
                            .original
                            .contains_key(&format!("{}_all.csv", n.trim_end_matches(".csv"))))
            })
            .cloned()
            .collect::<Vec<_>>();
        for name in full {
            let (columns, rows) =
                Self::read_csv(&self.original[&name]).map_err(|e| format!("{name}: {e}"))?;
            let stem = name.trim_end_matches(".csv").trim_end_matches("_all");
            let uid = ID
                .find_iter(basename(stem))
                .last()
                .map(|m| m.as_str().to_lowercase())
                .unwrap_or(hash(&name)[..32].into());
            let folder = rx(r"\s+[a-fA-F0-9]{32}_").replace_all(stem, "/");
            let folder = rx(r"\s+[a-fA-F0-9]{32}$").replace(&folder, "").to_string();
            let view = join("", &format!("{stem}.csv"));
            let (vc, vr) =
                Self::read_csv(self.original.get(&view).unwrap_or(&self.original[&name]))?;
            if self.dbs.iter().any(|d| d.uid == uid) {
                return Err(format!("Повторяющийся источник данных: {uid}"));
            }
            self.dbs.push(Database {
                source: name.clone(),
                uid,
                title: basename(&folder).into(),
                folder,
                rows,
                columns,
                view_rows: vr,
                view_columns: vc,
                ..Default::default()
            });
        }
        self.report
            .stats
            .insert("source_pages".into(), self.pages.len());
        self.report
            .stats
            .insert("source_databases".into(), self.dbs.len());
        self.report.stats.insert(
            "source_rows".into(),
            self.dbs.iter().map(|d| d.rows.len()).sum(),
        );
        Ok(())
    }
    fn match_rows(&mut self) -> Result<()> {
        (self.progress)("Сопоставление CSV и страниц записей…");
        for di in 0..self.dbs.len() {
            self.check()?;
            let db = self.dbs[di].clone();
            let mut available = self
                .pages
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    p.db.is_empty() && p.source.starts_with(&(db.folder.clone() + "/"))
                })
                .map(|(i, _)| i)
                .collect::<BTreeSet<_>>();
            let props = available
                .iter()
                .map(|&i| (i, fields(&self.pages[i].text, &db.columns)))
                .collect::<BTreeMap<_, _>>();
            let mut remaining = (0..db.rows.len()).collect::<BTreeSet<_>>();
            let mut matches = BTreeMap::new();
            while !remaining.is_empty() && !available.is_empty() {
                let mut proposals = vec![];
                for &ri in &remaining {
                    let row = &db.rows[ri];
                    let title = db
                        .columns
                        .first()
                        .and_then(|k| row.get(k))
                        .map(String::as_str)
                        .unwrap_or("");
                    let mut ranked = vec![];
                    for &pi in &available {
                        let p = &self.pages[pi];
                        let a = norm(title);
                        let b = norm(&p.title);
                        let tm = a == b || (a.is_empty() && b == "untitled");
                        let mut score = if tm { 100 } else { 0 };
                        let mut agreements = 0;
                        for (k, v) in row {
                            if Some(k) == db.columns.first() || v.is_empty() {
                                continue;
                            }
                            let exported = props[&pi].get(k).map(String::as_str).unwrap_or("");
                            let nv = norm(v);
                            if nv == norm(exported) {
                                score += 35;
                                agreements += 1
                            } else if nv.len() > 20
                                && (norm(exported).contains(&nv) || norm(&p.text).contains(&nv))
                            {
                                score += 10;
                            }
                        }
                        ranked.push((score, pi, tm || agreements >= 2));
                    }
                    ranked.sort_by(|a, b| b.cmp(a));
                    if let Some(&(sc, pi, good)) = ranked.first() {
                        if good && sc > 0 && (ranked.len() == 1 || sc > ranked[1].0) {
                            proposals.push((sc, ri, pi));
                        }
                    }
                }
                if proposals.is_empty() {
                    break;
                }
                proposals.sort_by(|a, b| b.cmp(a));
                let mut changed = false;
                for &(sc, ri, pi) in &proposals {
                    if !remaining.contains(&ri) || !available.contains(&pi) {
                        continue;
                    }
                    if proposals
                        .iter()
                        .filter(|&&(s, r, p)| s == sc && p == pi && remaining.contains(&r))
                        .count()
                        > 1
                    {
                        continue;
                    }
                    matches.insert(ri, pi);
                    available.remove(&pi);
                    remaining.remove(&ri);
                    changed = true;
                }
                if !changed {
                    break;
                }
            }
            let mut vremaining = (0..db.view_rows.len()).collect::<BTreeSet<_>>();
            let mut orders = BTreeMap::new();
            for (ri, row) in db.rows.iter().enumerate() {
                let exact = vremaining
                    .iter()
                    .copied()
                    .filter(|&j| {
                        db.view_rows[j].iter().all(|(k, v)| {
                            norm(row.get(k).map(String::as_str).unwrap_or("")) == norm(v)
                        })
                    })
                    .collect::<Vec<_>>();
                if exact.len() == 1 {
                    orders.insert(ri, exact[0] + 1);
                    vremaining.remove(&exact[0]);
                } else if exact.len() > 1 {
                    self.warn(
                        "ambiguous_view_row",
                        &db.source,
                        &format!("Строка {}: одинаковые строки вида.", ri + 1),
                    );
                }
            }
            if !vremaining.is_empty() {
                self.warn(
                    "unmatched_view_rows",
                    &db.source,
                    &format!("Не сопоставлено строк вида: {}", vremaining.len()),
                );
            }
            for (ri, row) in db.rows.iter().enumerate() {
                let pi = if let Some(&pi) = matches.get(&ri) {
                    pi
                } else {
                    let uid = hash(format!("{}\0{}\0{ri}", db.uid, pretty(row)))[..32].to_string();
                    let title = db
                        .columns
                        .first()
                        .and_then(|k| row.get(k))
                        .filter(|s| !s.is_empty())
                        .cloned()
                        .unwrap_or("Без названия".into());
                    let source = format!("{}/__csv_{uid}.md", db.folder);
                    let pi = self.pages.len();
                    self.pages.push(Page {
                        source,
                        uid,
                        title: title.clone(),
                        text: format!("# {title}\n"),
                        ..Default::default()
                    });
                    self.warn(
                        "unmatched_csv_row",
                        &db.source,
                        &format!("Строка {} сохранена отдельно; Markdown не удалён.", ri + 1),
                    );
                    pi
                };
                let p = &mut self.pages[pi];
                p.db = db.uid.clone();
                p.row = row.clone();
                p.order = ri + 1;
                p.view_order = *orders.get(&ri).unwrap_or(&0);
                self.dbs[di].pages.push(pi);
            }
            for pi in available {
                self.warn(
                    "unmatched_markdown",
                    &self.pages[pi].source.clone(),
                    "Не сопоставлена с CSV; сохранена отдельно.",
                );
            }
            self.report.row_matching.push(json!({"database":db.uid,"rows":db.rows.len(),"matched":matches.len(),"unmatched":remaining.len()}));
        }
        Ok(())
    }
    fn hierarchy(
        &mut self,
        files: &BTreeMap<String, Vec<u8>>,
    ) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
        let mut dirs = BTreeMap::from([(String::new(), String::new())]);
        let mut page_folders = BTreeMap::new();
        let mut all_dirs = BTreeSet::new();
        for name in files.keys() {
            let mut dir = parent(name);
            while !dir.is_empty() {
                all_dirs.insert(dir.to_string());
                dir = parent(dir);
            }
        }
        let roots = self
            .pages
            .iter()
            .filter(|p| !p.source.contains('/') && p.text.lines().any(|l| l.starts_with("## ")))
            .cloned()
            .collect::<Vec<_>>();
        for root in roots {
            let physical = rx(r"\s+[a-fA-F0-9]{32}\.md$")
                .replace(&root.source, "")
                .to_string();
            let direct = self
                .pages
                .iter()
                .enumerate()
                .filter(|(_, p)| parent(&p.source) == physical)
                .map(|(i, p)| (i, p.clone()))
                .collect::<Vec<_>>();
            if direct.is_empty() {
                continue;
            }
            let root_folder = safe_name(&root.title);
            dirs.insert(physical.clone(), root_folder.clone());
            page_folders.insert(root.source.clone(), root_folder.clone());
            let mut section = "Другие сервисы".to_string();
            let mut in_aside = false;
            let mut grouped = BTreeMap::<String, String>::new();
            for line in root.text.lines() {
                if line.trim() == "<aside>" {
                    in_aside = true;
                } else if line.trim() == "</aside>" {
                    in_aside = false;
                    section = "Другие сервисы".into();
                } else if in_aside
                    && line.starts_with("**")
                    && line.ends_with("**")
                    && line.len() > 4
                {
                    section = line.trim_matches('*').to_string();
                } else if let Some(title) = line.strip_prefix("## ") {
                    section = title.trim().into();
                }
                for m in rx(r"\]\(([^\n]+\.md)\)").captures_iter(line) {
                    let source = normalize_path(&join(parent(&root.source), &decode(&m[1])));
                    if direct.iter().any(|(_, p)| p.source == source) {
                        if let Some(previous) = grouped.get(&source) {
                            if previous != &section {
                                self.warn("ambiguous_section",&source,"Страница упоминается в нескольких разделах; сохранён первый раздел.");
                            }
                        } else {
                            grouped.insert(source, section.clone());
                        }
                    }
                }
            }
            let mut used = BTreeSet::new();
            let mut claimed_owners = BTreeSet::new();
            let mut duplicate_names = BTreeMap::<String, usize>::new();
            for (_, page) in &direct {
                let raw = rx(r"\s+[a-fA-F0-9]{32}\.md$")
                    .replace(basename(&page.source), "")
                    .to_string();
                *duplicate_names.entry(raw).or_default() += 1;
            }
            for (index, page) in direct {
                let section = grouped.get(&page.source).cloned().unwrap_or_else(|| {
                    fields(&page.text, &vec!["Тэг".into(), "Тег".into()])
                        .values()
                        .next()
                        .cloned()
                        .unwrap_or("Другие сервисы".into())
                });
                let group = join(&root_folder, &safe_name(&section));
                page_folders.insert(page.source.clone(), group.clone());
                self.section_nodes
                    .entry(group.clone())
                    .or_default()
                    .push(index);
                let raw = rx(r"\s+[a-fA-F0-9]{32}\.md$")
                    .replace(basename(&page.source), "")
                    .to_string();
                let exact = join(&physical, &raw);
                let suffix = format!("{} {}-{}", exact, &page.uid[..4], &page.uid[28..]);
                let has_children_reference = decode(&page.text).contains(&(raw.clone() + "/"));
                let own = if all_dirs.contains(&suffix) {
                    Some(suffix)
                } else if all_dirs.contains(&exact)
                    && (duplicate_names[&raw] == 1 || has_children_reference)
                    && !claimed_owners.contains(&exact)
                {
                    Some(exact)
                } else {
                    None
                };
                if let Some(own) = own {
                    claimed_owners.insert(own.clone());
                    let mut target = join(&group, &safe_name(&page.title));
                    if used.contains(&target.to_lowercase()) {
                        target = format!("{target}-{}", &page.uid[..8]);
                    }
                    used.insert(target.to_lowercase());
                    dirs.insert(own, target);
                }
            }
        }
        self.report
            .stats
            .insert("sections".into(), self.section_nodes.len());
        (dirs, page_folders)
    }
    fn paths(&mut self, files: &BTreeMap<String, Vec<u8>>) {
        let (mut dirs, page_folders) = self.hierarchy(files);
        let mut used_dirs = dirs
            .values()
            .map(|p| p.to_lowercase())
            .collect::<BTreeSet<_>>();
        fn directory(
            src: &str,
            dirs: &mut BTreeMap<String, String>,
            used: &mut BTreeSet<String>,
        ) -> String {
            if let Some(x) = dirs.get(src) {
                return x.clone();
            }
            let par = directory(parent(src), dirs, used);
            let mut target = join(&par, &clean_component(basename(src)));
            if used.contains(&target.to_lowercase()) {
                target = format!("{target}-{}", &hash(src)[..8]);
            }
            used.insert(target.to_lowercase());
            dirs.insert(src.into(), target.clone());
            target
        }
        for di in 0..self.dbs.len() {
            let db = self.dbs[di].clone();
            let folder = directory(&db.folder, &mut dirs, &mut used_dirs);
            let target = self.reserve(
                join(&folder, &format!("{}.md", safe_name(&db.title))),
                &db.uid,
            );
            let base = self.reserve(
                join(&folder, &format!("{}.base", safe_name(&db.title))),
                &db.uid,
            );
            self.dbs[di].target = target.clone();
            self.dbs[di].base = base;
            self.targets.insert(db.source.clone(), target.clone());
            self.targets.insert(
                db.source.trim_end_matches("_all.csv").to_string() + ".csv",
                target.clone(),
            );
            self.ids.insert(db.uid.clone(), target);
            for col in &db.columns {
                let key = if is_tag_column(col) {
                    "tags".to_string()
                } else if [
                    "notion_id",
                    "notion_database_id",
                    "notion_row_order",
                    "notion_view_order",
                ]
                .contains(&col.as_str())
                {
                    format!("Поле {col}")
                } else {
                    col.clone()
                };
                self.dbs[di].keys.insert(col.clone(), key);
            }
        }
        for pi in 0..self.pages.len() {
            let p = self.pages[pi].clone();
            let folder = page_folders
                .get(&p.source)
                .cloned()
                .unwrap_or_else(|| directory(parent(&p.source), &mut dirs, &mut used_dirs));
            let target = self.reserve(
                join(&folder, &format!("{}.md", safe_name(&p.title))),
                &p.uid,
            );
            self.pages[pi].target = target.clone();
            self.targets.insert(p.source, target.clone());
            self.ids.insert(p.uid.clone(), target.clone());
            self.report.mapping.insert(p.uid, target);
        }
        for (name, _) in files {
            if name.ends_with(".md") || name.ends_with(".csv") {
                continue;
            }
            let base = basename(name);
            let (stem, ext) = base.rsplit_once('.').unwrap_or((base, ""));
            let target = self.reserve(
                format!(
                    "_attachments/{}-{}{}",
                    safe_name(stem),
                    &hash(name)[..10],
                    if ext.is_empty() {
                        String::new()
                    } else {
                        format!(".{}", ext.to_lowercase())
                    }
                ),
                "",
            );
            self.assets.insert(name.clone(), target.clone());
            self.targets.insert(name.clone(), target);
        }
    }
    fn missing(&mut self, uid: &str, url: &str, source: &str, has_notion_id: bool) -> String {
        if self.skip_missing_pages && has_notion_id {
            // ID-less paths use synthetic hashes and must not be mistaken for Notion pages.
            let item = self.report.skipped_pages.entry(uid.into()).or_insert_with(|| Missing {
                target: format!("https://www.notion.so/{uid}"),
                ..Default::default()
            });
            if !item.urls.iter().any(|s| s == url) {
                item.urls.push(url.into());
            }
            if !item.sources.iter().any(|s| s == source) {
                item.sources.push(source.into());
            }
            return item.target.clone();
        }
        if !self.report.missing_pages.contains_key(uid) {
            let target = self.reserve(
                format!("_Недостающие страницы/Страница-{}.md", &uid[..12]),
                uid,
            );
            self.report.missing_pages.insert(
                uid.into(),
                Missing {
                    target,
                    ..Default::default()
                },
            );
        }
        let item = self.report.missing_pages.get_mut(uid).unwrap();
        if !item.urls.iter().any(|s| s == url) {
            item.urls.push(url.into());
        }
        if !item.sources.iter().any(|s| s == source) {
            item.sources.push(source.into());
        }
        item.target.clone()
    }
    fn resolve(&mut self, url: &str, source: &str) -> Option<(String, String)> {
        let url = html_escape::decode_html_entities(url.trim().trim_matches(['<', '>']));
        let (path, mut anchor) = split_url(&url);
        if path.contains("://") && !notion(&url) {
            return None;
        }
        if path.contains("://") || (path.starts_with('/') && !page_ids(&path).is_empty()) {
            let uid = page_ids(&path).last()?.clone();
            let target = if let Some(target) = self.ids.get(&uid) {
                target.clone()
            } else {
                self.missing(&uid, &url, source, true)
            };
            if !anchor.is_empty() && !target.starts_with("https://") {
                self.warn("block_position_lost", source, &url);
                anchor.clear();
            }
            self.inc("notion_links_converted");
            return Some((target, anchor));
        }
        if rx(r"^[a-zA-Z][a-zA-Z0-9+.-]*:").is_match(&path) || path.is_empty() {
            return None;
        }
        let path = decode(&path);
        let targetpath = normalize_path(&join(parent(source), &path));
        let mut target = self.targets.get(&targetpath).cloned();
        if target.is_none() {
            if let Some(uid) = page_ids(basename(&path)).last() {
                target = self.ids.get(uid).cloned();
            }
        }
        let target = if let Some(target) = target {
            target
        } else {
            let notion_uid = page_ids(&path).last().cloned();
            let uid = notion_uid.clone().unwrap_or(hash(&targetpath)[..32].into());
            let unresolved = self.missing(&uid, &url, source, notion_uid.is_some());
            if !unresolved.starts_with("https://") {
                self.warn("missing_local_link", source, &url);
            }
            unresolved
        };
        if !target.starts_with("https://") && rx(r"^[a-fA-F0-9-]{32,36}$").is_match(&anchor) {
            self.warn("block_position_lost", source, &url);
            anchor.clear();
        }
        self.inc("local_links_converted");
        Some((target, anchor))
    }
    fn wiki(target: &str, label: &str, embed: bool, table: bool, anchor: &str) -> String {
        let target = target.strip_suffix(".md").unwrap_or(target);
        let label = label
            .chars()
            .map(|c| if "[]\r\n|".contains(c) { ' ' } else { c })
            .collect::<String>();
        let label = label.trim();
        if target.starts_with("https://") {
            let url = if anchor.is_empty() { target.to_string() } else { format!("{target}#{anchor}") };
            // A missing page must not be rendered as a remote image/embed.
            return format!(
                "[{}]({url})",
                if label.is_empty() { "Страница Notion" } else { label }
            );
        }
        let target = if anchor.is_empty() {
            target.into()
        } else {
            format!("{target}#{anchor}")
        };
        format!(
            "{}[[{}{}]]",
            if embed { "!" } else { "" },
            target,
            if label.is_empty() || embed {
                String::new()
            } else {
                format!("{}{label}", if table { "\\|" } else { "|" })
            }
        )
    }
    fn rewrite_plain(&mut self, text: &str, source: &str) -> String {
        let defs = rx(r"(?m)^( {0,3}\[[^\]\n]+\]:[ \t]*)(\S+)(.*)$");
        let mut replacements = vec![];
        let text = defs
            .replace_all(text, |c: &regex::Captures| {
                let line = if let Some((target, anchor)) = self.resolve(&c[2], source) {
                    if target.starts_with("https://") {
                        format!("{}{}{}{}", &c[1], target, if anchor.is_empty() { String::new() } else { format!("#{anchor}") }, &c[3])
                    } else {
                    let own = self.targets.get(source).map(String::as_str).unwrap_or("");
                    let a = parent(own)
                        .split('/')
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>();
                    let b = target.split('/').collect::<Vec<_>>();
                    let common = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
                    let relative = std::iter::repeat("..")
                        .take(a.len() - common)
                        .chain(b[common..].iter().copied())
                        .collect::<Vec<_>>()
                        .join("/");
                    let escaped = relative
                        .split('/')
                        .map(|p| {
                            percent_encoding::utf8_percent_encode(
                                p,
                                percent_encoding::NON_ALPHANUMERIC,
                            )
                            .to_string()
                        })
                        .collect::<Vec<_>>()
                        .join("/");
                    format!(
                        "{}{}{}{}",
                        &c[1],
                        escaped,
                        if anchor.is_empty() {
                            String::new()
                        } else {
                            format!("#{anchor}")
                        },
                        &c[3]
                    )
                    }
                } else {
                    c[0].to_string()
                };
                let token = format!("NTOREFTOKEN{:06}END", replacements.len());
                replacements.push((token.clone(), line));
                token
            })
            .to_string();
        let text = text.as_str();
        let regex = rx(r"(!?)\[([^\[\]\n]*)\]\(");
        let mut out = String::new();
        let mut cursor = 0;
        while let Some(m) = regex.captures(&text[cursor..]) {
            let start = cursor + m.get(0).unwrap().start();
            let begin = cursor + m.get(0).unwrap().end();
            out.push_str(&text[cursor..start]);
            let mut j = begin;
            let mut depth = 1;
            while j < text.len() && depth > 0 {
                match text.as_bytes()[j] {
                    b'\\' => {
                        j += 2;
                        continue;
                    }
                    b'(' => depth += 1,
                    b')' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            if depth != 0 {
                out.push_str(&text[start..]);
                cursor = text.len();
                break;
            }
            let dest = &text[begin..j - 1];
            let url = if dest.starts_with('<') {
                dest.split_once('>')
                    .map(|x| x.0.trim_start_matches('<'))
                    .unwrap_or(dest)
            } else {
                dest.split(" \"").next().unwrap_or(dest)
            };
            if let Some((target, anchor)) = self.resolve(url, source) {
                let line = &text[text[..start].rfind('\n').map(|i| i + 1).unwrap_or(0)..start];
                let table = line.trim_start_matches([' ', '>', '\t']).starts_with('|');
                out.push_str(&Self::wiki(&target, &m[2], &m[1] == "!", table, &anchor));
            } else {
                out.push_str(&text[start..j]);
            }
            cursor = j;
        }
        out.push_str(&text[cursor..]);
        let text = out;
        let mut out = String::new();
        let mut cursor = 0;
        for m in URL.find_iter(&text) {
            out.push_str(&text[cursor..m.start()]);
            let raw = m.as_str();
            let mut url = raw;
            while url.ends_with(['.', ',', ';', '»'])
                || (url.ends_with(')') && url.matches(')').count() > url.matches('(').count())
            {
                url = &url[..url.len() - url.chars().next_back().unwrap().len_utf8()];
            }
            if notion(url) {
                if let Some((target, anchor)) = self.resolve(url, source) {
                    if target.starts_with("https://") {
                        out.push_str(raw);
                        cursor = m.end();
                        continue;
                    }
                    let label = self
                        .pages
                        .iter()
                        .find(|p| p.target == target)
                        .map(|p| p.title.clone())
                        .unwrap_or("Связанная страница".into());
                    let line =
                        &text[text[..m.start()].rfind('\n').map(|i| i + 1).unwrap_or(0)..m.start()];
                    out.push_str(&Self::wiki(
                        &target,
                        &label,
                        false,
                        line.trim_start_matches([' ', '>', '\t']).starts_with('|'),
                        &anchor,
                    ));
                    out.push_str(&raw[url.len()..]);
                } else {
                    out.push_str(raw)
                }
            } else {
                out.push_str(raw)
            }
            cursor = m.end();
        }
        out.push_str(&text[cursor..]);
        let text = out;
        let relation = rx(r"\(([^\s()]*[a-fA-F0-9]{32}\.(?:md|csv))\)");
        let mut out = String::new();
        let mut cursor = 0;
        for m in relation.captures_iter(&text) {
            let full = m.get(0).unwrap();
            out.push_str(&text[cursor..full.start()]);
            if let Some((target, anchor)) = self.resolve(&m[1], source) {
                out.push_str(&Self::wiki(&target, "", false, false, &anchor))
            } else {
                out.push_str(full.as_str())
            }
            cursor = full.end();
        }
        out.push_str(&text[cursor..]);
        for (token, line) in replacements {
            out = out.replace(&token, &line);
        }
        out
    }
    fn rewrite(&mut self, text: &str, source: &str) -> String {
        let mut out = String::new();
        for (code, chunk) in code_parts(text) {
            if code {
                if chunk.contains("notion.so") || chunk.contains("app.notion.com") {
                    self.warn(
                        "notion_url_in_code",
                        source,
                        "URL оставлен как часть копируемого кода.",
                    );
                }
                out.push_str(&chunk);
            } else {
                for (protected, chunk) in inline_parts(&chunk) {
                    if protected {
                        out.push_str(chunk)
                    } else {
                        out.push_str(&self.rewrite_plain(chunk, source));
                    }
                }
            }
        }
        out
    }
    fn markup(&mut self, text: &str) -> String {
        let mut saved = vec![];
        let mut masked = String::new();
        for (code, chunk) in code_parts(text) {
            if code {
                let token = format!("NTOCODETOKEN{:06}END", saved.len());
                masked.push_str(&token);
                if chunk.ends_with('\n') {
                    masked.push('\n');
                }
                saved.push((token, chunk, true));
            } else {
                for (protected, s) in inline_parts(&chunk) {
                    if protected {
                        let token = format!("NTOCODETOKEN{:06}END", saved.len());
                        masked.push_str(&token);
                        saved.push((token, s.into(), false));
                    } else {
                        masked.push_str(s)
                    }
                }
            }
        }
        let icons = rx(r#"<img\b[^>]*src=["']https://app\.notion\.com/icons/([^"']+)["'][^>]*>"#);
        let mut text = icons
            .replace_all(&masked, |c: &regex::Captures| {
                if c[1].contains("star") {
                    "⭐"
                } else {
                    "ℹ️"
                }
            })
            .to_string();
        let open = rx(r"(?m)^([ \t]*)<(aside|details)\b[^>]*>");
        for _ in 0..256 {
            let matches = open
                .captures_iter(&text)
                .map(|c| {
                    (
                        c.get(0).unwrap().start(),
                        c.get(0).unwrap().end(),
                        c[1].to_string(),
                        c[2].to_string(),
                    )
                })
                .collect::<Vec<_>>();
            let Some((start, end, indent, tag)) = matches.last() else {
                break;
            };
            let close = format!("</{tag}>");
            let Some(ci) = text[*end..].find(&close) else {
                break;
            };
            let ci = end + ci;
            let body = &text[*end..ci];
            let mut body = body
                .lines()
                .map(|line| line.strip_prefix(indent).unwrap_or(line))
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string();
            let mut title = String::new();
            if tag == "details" {
                let sum = rx(r"(?s)<summary[^>]*>(.*?)</summary>");
                if let Some(c) = sum.captures(&body) {
                    title = rx(r"<[^>]+>").replace_all(&c[1], "").to_string();
                    body = sum.replace(&body, "").trim().to_string();
                }
            }
            let header = if tag == "details" {
                if let Some((_, clean)) = emoji_prefix(&title) {
                    title = clean;
                }
                format!("[!example]- {title}")
            } else {
                let (kind, title, clean) = clean_callout(&body);
                body = clean;
                if title.is_empty() {
                    format!("[!{kind}]")
                } else {
                    format!("[!{kind}] {title}")
                }
            };
            let replacement = format!(
                "\n\n{indent}> {header}\n{}\n\n",
                body.lines()
                    .map(|l| if l.is_empty() {
                        format!("{indent}>")
                    } else {
                        format!("{indent}> {l}")
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            text.replace_range(*start..ci + close.len(), &replacement);
        }
        for (token, chunk, block) in saved {
            if block {
                let regex = rx(&format!(r"(?m)^([ >\t]*){}", token));
                text = regex
                    .replace_all(&text, |c: &regex::Captures| {
                        chunk
                            .trim_end_matches('\n')
                            .split('\n')
                            .map(|l| {
                                if l.is_empty() {
                                    c[1].trim_end().to_string()
                                } else {
                                    format!("{}{l}", &c[1])
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .to_string();
                text = text.replace(&token, chunk.trim_end_matches('\n'));
            } else {
                text = text.replace(&token, &chunk);
            }
        }
        text
    }
    fn restore_toc(&mut self, body: String, source: &str) -> String {
        let headings = section_headings(&body);
        if headings.len() < 2 || body.contains("[[#") || !source.contains('/') {
            return body;
        }
        let mut seen = BTreeSet::new();
        let mut toc = String::from("**Содержание**\n\n");
        for heading in &headings {
            if !seen.insert(heading.anchor.clone()) {
                self.warn(
                    "duplicate_toc_heading",
                    source,
                    "Одинаковые заголовки в одном разделе ведут к первому совпадению.",
                );
            }
            toc.push_str(&format!(
                "{}- {}\n",
                "  ".repeat(heading.depth),
                Self::wiki("", &heading.title, false, false, &heading.anchor)
            ));
        }
        let first = headings[0].start;
        let prefix = &body[..first];
        let pairs = rx(r"(?m)^---[ \t]*\n(?:[ \t]*\n)+---[ \t]*\n(?:[ \t]*\n)*\z");
        // Notion's omitted TOC often leaves two adjacent rules. Fill that gap.
        let insert = if let Some(m) = pairs.find(prefix) {
            let relative = m.as_str().rfind("---").unwrap();
            m.start() + relative
        } else {
            first
        };
        let mut out = body[..insert].to_string();
        out.push_str(&toc);
        out.push('\n');
        out.push_str(&body[insert..]);
        self.inc("tables_of_contents_restored");
        out
    }
    fn inline_databases(&mut self, body: &str, source: &str) -> (String, bool) {
        let targets = self
            .dbs
            .iter()
            .map(|db| db.target.clone())
            .collect::<BTreeSet<_>>();
        let mut result = String::new();
        let mut any = false;
        for (code, chunk) in code_parts(body) {
            if code {
                result.push_str(&chunk);
                continue;
            }
            for line in chunk.split_inclusive('\n') {
                let prefix = line
                    .chars()
                    .take_while(|c| c.is_whitespace() || *c == '>')
                    .collect::<String>();
                let value = line.trim_start_matches([' ', '\t', '>']).trim();
                if value.starts_with("[[")
                    && value.ends_with("]]")
                    && value.matches("[[").count() == 1
                {
                    let target = value[2..value.len() - 2]
                        .split('|')
                        .next()
                        .unwrap()
                        .trim_end_matches('\\');
                    let file = if target.ends_with(".md") {
                        target.to_string()
                    } else {
                        format!("{target}.md")
                    };
                    if targets.contains(&file) {
                        result.push_str(prefix.trim_end_matches(['\r', '\n']));
                        result.push_str(&Self::wiki(&file, "", true, false, "^notion-table"));
                        if line.ends_with('\n') {
                            result.push('\n');
                        }
                        any = true;
                        self.inc("databases_embedded_inline");
                        continue;
                    }
                }
                result.push_str(line);
            }
        }
        if any {
            (self.progress)(&format!("Встроены таблицы: {}", source));
        }
        (result, any)
    }
    fn render(&mut self, files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
        (self.progress)("Преобразование страниц, ссылок и баз…");
        for pi in 0..self.pages.len() {
            self.check()?;
            let p = self.pages[pi].clone();
            let mut meta = BTreeMap::<String, Value>::new();
            let mut body = p.text.clone();
            let mut tags = metadata_tags(&p.text).into_iter().collect::<BTreeSet<_>>();
            let header = p
                .text
                .split_once("\n\n")
                .map(|(_, s)| s.split("\n\n").next().unwrap_or(""))
                .unwrap_or("");
            let top_tags = header
                .lines()
                .filter_map(|l| l.split_once(':'))
                .filter(|(k, _)| is_tag_column(k))
                .map(|(k, v)| (k.to_string(), v.trim().to_string()))
                .collect::<BTreeMap<_, _>>();
            body = remove_tag_lines(&body, &top_tags);
            if !p.db.is_empty() {
                let db = self.dbs.iter().find(|d| d.uid == p.db).unwrap().clone();
                for (key, value) in &p.row {
                    if is_tag_column(key) {
                        tags.extend(obsidian_tags(value));
                        continue;
                    }
                    let rendered = self.rewrite(value, &p.source);
                    meta.insert(db.keys[key].clone(), json!(rendered));
                }
                let tag_values = p
                    .row
                    .iter()
                    .filter(|(k, _)| is_tag_column(k))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                body = remove_tag_lines(&body, &tag_values);
                let exported = fields(&p.text, &db.columns);
                for col in &db.columns {
                    let value = p.row.get(col).map(String::as_str).unwrap_or("");
                    if is_tag_column(col)
                        || Some(col) == db.columns.first()
                        || value.is_empty()
                        || exported.contains_key(col)
                    {
                        continue;
                    }
                    body = format!("{}\n\n## {col}\n\n{value}\n", body.trim_end());
                }
            }
            if !tags.is_empty() {
                meta.insert("tags".into(), json!(tags.into_iter().collect::<Vec<_>>()));
            }
            let body = self.rewrite(&body, &p.source);
            let body = self.markup(&body);
            let body = if p.db.is_empty() {
                self.restore_toc(body, &p.source)
            } else {
                body
            };
            let (body, embedded) = self.inline_databases(&body, &p.source);
            let _ = embedded;
            self.output.insert(
                p.target,
                if meta.is_empty() {
                    format!("{}\n", body.trim_end()).into_bytes()
                } else {
                    format!("---\n{}\n---\n\n{}\n", pretty(&meta), body.trim_end()).into_bytes()
                },
            );
        }
        let mut table_data = json!({"byPage":{},"byRowPath":{}});
        for db in self.dbs.clone() {
            self.check()?;
            let title_col = db.columns.first().map(String::as_str).unwrap_or("Название");
            let property = |c: &String| {
                if c == title_col {
                    "formula.Название".to_string()
                } else {
                    format!("note.{}", db.keys[c])
                }
            };
            let order = |cols: &[String]| {
                let mut used = BTreeSet::new();
                cols.iter()
                    .filter(|c| db.keys.contains_key(*c))
                    .map(property)
                    .filter(|id| used.insert(id.clone()))
                    .collect::<Vec<_>>()
            };
            let props = db
                .columns
                .iter()
                .map(|c| (property(c), json!({"displayName":c})))
                .collect::<BTreeMap<_, _>>();
            let widths = db
                .columns
                .iter()
                .map(|c| {
                    (
                        property(c),
                        if c == title_col {
                            320
                        } else if matches!(c.as_str(), "Ответ" | "Примечание" | "Текст" | "Text")
                        {
                            680
                        } else {
                            220
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();
            // Membership and sort state belong to the .base configuration, not public Properties.
            let paths = db
                .pages
                .iter()
                .map(|&i| self.pages[i].target.clone())
                .collect::<Vec<_>>();
            let mut view_pages = db
                .pages
                .iter()
                .map(|&i| &self.pages[i])
                .filter(|p| p.view_order > 0)
                .collect::<Vec<_>>();
            view_pages.sort_by_key(|p| p.view_order);
            let view_paths = view_pages
                .iter()
                .map(|p| p.target.clone())
                .collect::<Vec<_>>();
            let all_list = serde_json::to_string(&paths).unwrap();
            let view_list = serde_json::to_string(&view_paths).unwrap();
            let all_rank = format!("{all_list}.reduce(if(file.path == value || file.path.endsWith(\"/\" + value), index, acc), -1)");
            let view_rank = format!("{view_list}.reduce(if(file.path == value || file.path.endsWith(\"/\" + value), index, acc), -1)");
            let membership = |paths: &Vec<String>| {
                if paths.is_empty() {
                    json!("false")
                } else {
                    json!({"or":paths.iter().map(|path|format!("file.path == {} || file.path.endsWith({})",serde_json::to_string(path).unwrap(),serde_json::to_string(&format!("/{path}")).unwrap())).collect::<Vec<_>>()})
                }
            };
            let title_key = serde_json::to_string(&db.keys[title_col]).unwrap();
            let title_link = format!(
                "link(file.path, if(note[{title_key}], note[{title_key}], \"Без названия\"))"
            );
            let base = json!({"filters":{"and":["file.ext == \"md\"",membership(&paths)]},"properties":props,
            "formulas":{"Порядок":all_rank,"Порядок вида":view_rank,"Название":title_link},
            "views":[
              {"type":"table","name":"Экспортированный вид","filters":{"and":[membership(&view_paths)]},"order":order(&db.view_columns),"sort":[{"property":"formula.Порядок вида","direction":"ASC"}],"rowHeight":"tall","columnSize":widths},
              {"type":"table","name":"Все записи","order":order(&db.columns),"sort":[{"property":"formula.Порядок","direction":"ASC"}],"rowHeight":"tall","columnSize":widths},
              {"type":"notion-full-text","name":"Полный текст · копировать","order":order(&db.view_columns),"sort":[{"property":"formula.Порядок","direction":"ASC"}]}
            ]});
            self.output
                .insert(db.base.clone(), pretty(&base).into_bytes());
            let cols = if db.view_columns.is_empty() {
                &db.columns
            } else {
                &db.view_columns
            };
            let mut body=format!("# {}\n\nВсего записей: **{}**. {}\n\nВсе ячейки ниже показаны полностью. {}\n\n",db.title,db.rows.len(),Self::wiki(&db.base,"Открыть базу: фильтры и редактирование",false,false,""),"Для копирования по клику включите вложенный плагин Notion Full Tables; без него можно выделить текст и нажать Ctrl+C.");
            body.push_str(&format!(
                "| {} |\n| {} |\n",
                cols.iter()
                    .map(|c| markdown_cell(c))
                    .collect::<Vec<_>>()
                    .join(" | "),
                cols.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
            ));
            let mut raw_rows = vec![];
            for &pi in &db.pages {
                let page = self.pages[pi].clone();
                let mut cells = vec![];
                let mut values = vec![];
                for col in cols {
                    let value = page.row.get(col).cloned().unwrap_or_default();
                    let raw = self.rewrite(&value, &page.source);
                    values.push(raw.clone());
                    if col == title_col {
                        let shown = if value.is_empty() {
                            "Без названия"
                        } else {
                            value.as_str()
                        };
                        cells.push(Self::wiki(&page.target, shown, false, true, ""));
                    } else {
                        let cell = markdown_cell(&raw);
                        // Escaping original Markdown syntax must not escape generated internal links.
                        cells.push(cell);
                    }
                }
                body.push_str(&format!("| {} |\n", cells.join(" | ")));
                raw_rows.push(values);
            }
            body.push_str("\n^notion-table\n");
            self.output.insert(db.target.clone(), body.into_bytes());
            table_data["byPage"][&db.target] = json!({"columns":cols,"rows":raw_rows,"titleIndex":cols.iter().position(|c|c==title_col)});
            for &pi in &db.pages {
                table_data["byRowPath"][&self.pages[pi].target] = json!({"titleColumn":title_col});
            }
            self.report.databases.push(json!({"id":db.uid,"title":db.title,"rows":db.rows.len(),"view_rows":db.view_rows.len(),"base":db.base,"readable_table":db.target,"columns":db.keys}));
        }
        let plugin = ".obsidian/plugins/notion-table-copy/";
        for (name, data) in [
            (
                "main.js",
                include_str!("../assets/notion-table-copy/main.js"),
            ),
            (
                "manifest.json",
                include_str!("../assets/notion-table-copy/manifest.json"),
            ),
            (
                "styles.css",
                include_str!("../assets/notion-table-copy/styles.css"),
            ),
        ] {
            self.output
                .insert(format!("{plugin}{name}"), data.as_bytes().to_vec());
        }
        self.output.insert(
            format!("{plugin}tables.json"),
            pretty(&table_data).into_bytes(),
        );
        self.output.insert(
            ".obsidian/snippets/notion-full-tables.css".into(),
            include_str!("../assets/notion-table-copy/styles.css")
                .as_bytes()
                .to_vec(),
        );
        self.output.insert(
            ".obsidian/appearance.json".into(),
            pretty(&json!({"enabledCssSnippets":["notion-full-tables"]})).into_bytes(),
        );
        self.output
            .insert(".obsidian/community-plugins.json".into(), b"[]".to_vec());
        self.output.insert(
            ".obsidian/app.json".into(),
            pretty(&json!({"showInlineTitle":false})).into_bytes(),
        );
        for (uid, item) in &self.report.missing_pages {
            self.output.insert(item.target.clone(),format!("# Страница отсутствовала в экспорте\n\n> [!warning]\n> Содержимое не перенесено. Это заглушка.\n\nИдентификатор Notion: `{uid}`. Исходные адреса и места использования указаны в .notion-to-obsidian/migration-report.json.\n").into_bytes());
        }
        for (source, target) in &self.assets {
            self.output.insert(target.clone(), files[source].clone());
        }
        self.report
            .stats
            .insert("converted_pages".into(), self.pages.len());
        self.report
            .stats
            .insert("attachments".into(), self.assets.len());
        self.report
            .stats
            .insert("missing_pages".into(), self.report.missing_pages.len());
        self.report.stats.insert("skipped_pages".into(), self.report.skipped_pages.len());
        self.report.stats.insert("bases".into(), self.dbs.len());
        let home=format!("# Инструкция — экспорт v0.5.1\n\nОткройте эту папку как хранилище. Основной вид — таблицы с полным текстом; для фильтрации доступны **Bases / Базы**.\n\n## Главная страница\n\n{}\n\n## Базы\n\n{}\n\n## Как пользоваться\n\n- Поиск по содержимому: Ctrl+Shift+F.\n- Переход к заметке: Ctrl+O.\n- Полные ответы видны на страницах таблиц и в отдельных заметках.\n- Копирование одним кликом: Настройки → Сторонние плагины → включите Notion Full Tables. Плагин уже находится в этом хранилище; скачивать его не нужно.\n- В Bases после включения плагина доступен вид «Полный текст · копировать».\n- Теги сохранены в стандартном свойстве tags. Пробелы в тегах заменены подчёркиваниями.\n- Экспортированный вид — снимок состава строк.\n- Недостающие страницы обозначены заглушками, если не включён ручной пропуск.\n- Повторный запуск создаёт отдельное хранилище, без слияния локальных правок.\n",self.pages.iter().filter(|p|!p.source.contains('/')).map(|p|format!("- {}",Self::wiki(&p.target,&p.title,false,false,""))).collect::<Vec<_>>().join("\n"),self.dbs.iter().map(|d|format!("- {}",Self::wiki(&d.target,&d.title,false,false,""))).collect::<Vec<_>>().join("\n"));
        self.output
            .insert("Начать здесь.md".into(), home.into_bytes());
        Ok(())
    }
    fn audit(&mut self, files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
        (self.progress)("Проверка ссылок, Markdown и контрольных сумм…");
        let mut errors = vec![];
        let wiki = rx(r"!?\[\[([^\]\n]+)\]\]");
        let mut fences = 0;
        let mut tables = 0;
        for (name, data) in &self.output {
            self.check()?;
            if name.ends_with(".base") {
                if serde_json::from_slice::<Value>(data).is_err() {
                    errors.push(format!("Неверная база: {name}"));
                }
                continue;
            }
            if !name.ends_with(".md") {
                continue;
            }
            let text = std::str::from_utf8(data).unwrap();
            if let Some(rest) = text.strip_prefix("---\n") {
                if let Some((front, _)) = rest.split_once("\n---\n") {
                    if let Ok(Value::Object(meta)) = serde_json::from_str::<Value>(front) {
                        if meta.keys().any(|k| {
                            [
                                "notion_id",
                                "notion_database_id",
                                "notion_row_order",
                                "notion_view_order",
                            ]
                            .contains(&k.as_str())
                                || rx(r"^db_[a-f0-9]{8,32}_").is_match(k)
                        }) {
                            errors.push(format!("Служебные Properties в заметке: {name}"));
                        }
                    }
                }
            }
            let body = if let Some(stripped) = text.strip_prefix("---\n") {
                if let Some((front, body)) = stripped.split_once("\n---\n") {
                    if serde_json::from_str::<Value>(front).is_err() {
                        errors.push(format!("Неверные свойства: {name}"));
                    }
                    body
                } else {
                    errors.push(format!("Незакрытые свойства: {name}"));
                    text
                }
            } else {
                text
            };
            for (code, chunk) in code_parts(text) {
                if code {
                    continue;
                }
                for (protect, s) in inline_parts(&chunk) {
                    if protect {
                        continue;
                    }
                    for m in wiki.captures_iter(s) {
                        let target = m[1]
                            .split('|')
                            .next()
                            .unwrap()
                            .trim_end_matches('\\')
                            .split('#')
                            .next()
                            .unwrap()
                            .to_string();
                        if !target.is_empty()
                            && !self.output.contains_key(&target)
                            && !self.output.contains_key(&(target.clone() + ".md"))
                        {
                            errors.push(format!("Неразрешённая ссылка: {name} → {target}"));
                        }
                        let raw = m[1].split('|').next().unwrap().trim_end_matches('\\');
                        if let Some((_, anchor)) = raw.split_once('#') {
                            let resolved = if target.is_empty() {
                                name.clone()
                            } else if self.output.contains_key(&target) {
                                target.clone()
                            } else {
                                target.clone() + ".md"
                            };
                            if let Some(bytes) = self.output.get(&resolved) {
                                let content = std::str::from_utf8(bytes).unwrap_or("");
                                if let Some(id) = anchor.strip_prefix('^') {
                                    if !rx(&format!(r"\^{}(?:\s|$)", regex::escape(id)))
                                        .is_match(content)
                                    {
                                        errors.push(format!(
                                            "Не найден блок встраивания: {resolved}#{anchor}"
                                        ));
                                    }
                                } else if target.is_empty()
                                    && !section_headings(content)
                                        .iter()
                                        .any(|h| h.anchor == anchor || h.title == anchor)
                                {
                                    errors.push(format!(
                                        "Не найден пункт оглавления: {name}#{anchor}"
                                    ));
                                }
                            }
                        }
                    }
                    // Explicitly skipped Notion pages intentionally stay as
                    // external links. Every other unresolved Notion URL remains
                    // a migration error, including links with no recognized ID.
                    for matched in URL.find_iter(s).filter(|m| notion(m.as_str())) {
                        let (url_path, _) = split_url(matched.as_str());
                        let intentionally_skipped = page_ids(&url_path)
                            .last()
                            .map(|uid| self.report.skipped_pages.contains_key(uid))
                            .unwrap_or(false);
                        if !intentionally_skipped {
                            errors.push(format!("Осталась ссылка Notion: {name}"));
                        }
                    }
                    if rx(r"</?(?:aside|details|summary)\b").is_match(s) {
                        errors.push(format!("Осталась разметка Notion: {name}"));
                    }
                }
            }
            for event in
                pulldown_cmark::Parser::new_ext(body, pulldown_cmark::Options::ENABLE_TABLES)
            {
                match event {
                    pulldown_cmark::Event::Start(pulldown_cmark::Tag::CodeBlock(
                        pulldown_cmark::CodeBlockKind::Fenced(_),
                    )) => fences += 1,
                    pulldown_cmark::Event::Start(pulldown_cmark::Tag::Table(_)) => tables += 1,
                    pulldown_cmark::Event::Start(pulldown_cmark::Tag::CodeBlock(
                        pulldown_cmark::CodeBlockKind::Indented,
                    )) => errors.push(format!("Случайный indented-code блок: {name}")),
                    _ => {}
                }
            }
        }
        let mut external = 0;
        for p in &self.pages {
            let target = std::str::from_utf8(&self.output[&p.target]).unwrap();
            let source_fences = fenced_contents(&p.text);
            let target_body = target
                .split_once("\n---\n")
                .map(|(_, b)| b)
                .unwrap_or(target);
            let target_fences = fenced_contents(target_body);
            for (code_hash, count) in source_fences {
                if target_fences.get(&code_hash).copied().unwrap_or(0) < count {
                    errors.push(format!("Изменён копируемый блок: {}", p.source));
                }
            }
            for m in URL.find_iter(&p.text) {
                if notion(m.as_str()) {
                    continue;
                }
                if !target.contains(m.as_str()) {
                    errors.push(format!("Изменён внешний адрес: {}", p.source));
                }
                external += 1;
            }
        }
        for (source, target) in &self.assets {
            if hash(&files[source]) != hash(&self.output[target]) {
                errors.push(format!("Изменено вложение: {source}"));
            }
        }
        errors.sort();
        errors.dedup();
        self.report
            .stats
            .insert("external_url_occurrences_preserved".into(), external);
        self.report.audit = json!({"passed":errors.is_empty(),"errors":errors,"attachment_hashes_verified":self.assets.len(),"fenced_blocks":fences,"tables":tables});
        if !errors.is_empty() {
            return Err(format!(
                "Аудит не пройден: {}",
                errors
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
        Ok(())
    }
    pub fn inspect(mut self, archive: &Path) -> Result<Report> {
        let files = self.read_archive(archive)?;
        self.inventory(&files)?;
        self.match_rows()?;
        self.report.databases = self
            .dbs
            .iter()
            .map(|d| json!({"title":d.title,"rows":d.rows.len(),"view_rows":d.view_rows.len()}))
            .collect();
        Ok(self.report)
    }
    pub fn run(mut self, archive: &Path, destination: &Path) -> Result<Report> {
        if destination.exists() {
            return Err("Папка результата уже существует. Выберите новое имя.".into());
        }
        let files = self.read_archive(archive)?;
        self.inventory(&files)?;
        self.match_rows()?;
        self.paths(&files);
        self.render(&files)?;
        self.audit(&files)?;
        let summary=format!("# Отчёт о переносе\n\n- Исходных страниц: **{}**\n- Записей баз: **{}**\n- Баз: **{}**\n- Вложений: **{}**\n- Недостающих страниц: **{}**\n- Пропущенных вручную страниц Notion: **{}**\n\nСтруктурный аудит: **пройден**. Внутренние ссылки разрешаются, контрольные суммы вложений совпадают.\n\nПредупреждений: {}. Подробности и исходные адреса — в .notion-to-obsidian/migration-report.json.\n",self.report.stats["source_pages"],self.report.stats["source_rows"],self.dbs.len(),self.assets.len(),self.report.missing_pages.len(),self.report.skipped_pages.len(),self.report.warnings.len());
        self.output
            .insert("Отчёт о переносе.md".into(), summary.into_bytes());
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let stage = tempfile::Builder::new()
            .prefix(".notion-convert-")
            .tempdir_in(parent)
            .map_err(|e| e.to_string())?;
        for (name, data) in &self.output {
            self.check()?;
            let path = stage.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
            fs::write(path, data).map_err(|e| e.to_string())?;
        }
        fs::create_dir_all(stage.path().join(".notion-to-obsidian")).map_err(|e| e.to_string())?;
        fs::write(
            stage.path().join(".notion-to-obsidian/migration-report.json"),
            pretty(&self.report),
        )
        .map_err(|e| e.to_string())?;
        let state = json!({"version":"0.5.1","archive_sha256":hash(fs::read(archive).map_err(|e|e.to_string())?),"pages":self.pages.iter().map(|p|(p.uid.clone(),json!({"path":p.target,"source_sha256":hash(&p.text),"output_sha256":hash(&self.output[&p.target]),"database_id":p.db,"row_order":p.order,"view_order":p.view_order}))).collect::<BTreeMap<_,_>>()});
        fs::write(stage.path().join(".notion-to-obsidian/import-state.json"), pretty(&state))
            .map_err(|e| e.to_string())?;
        fs::write(stage.path().join(".notion-to-obsidian/Открыть.txt"),"В Obsidian откройте папку результата как хранилище. Включите встроенный модуль Bases. Начните с заметки «Начать здесь».\n").map_err(|e|e.to_string())?;
        self.check()?;
        if destination.exists() {
            return Err("Папка результата появилась во время работы.".into());
        }
        fs::rename(stage.path(), destination).map_err(|e| e.to_string())?;
        (self.progress)("Готово. Откройте папку результата в Obsidian.");
        Ok(self.report)
    }
}
pub fn convert(archive: &Path, destination: &Path) -> Result<Report> {
    Converter::new(|_| {}, Arc::new(AtomicBool::new(false))).run(archive, destination)
}
