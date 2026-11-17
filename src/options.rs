//! Versioned, portable export settings. No input paths or user data belong here.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    Universal,
    Knowledge,
    Custom,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sections { Source, RootOutline }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unmatched { Source, NamedFolder }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParentPage { InsideChildren, Sibling }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseLocation { Source, SeparateRoot }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableOutput { MarkdownAndBases, MarkdownOnly, BasesOnly }
impl TableOutput {
    pub fn markdown(self) -> bool { self != Self::BasesOnly }
    pub fn bases(self) -> bool { self != Self::MarkdownOnly }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleColumn { First, Explicit }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Widths { Uniform, LongTextPresets, PerColumn }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Toc { Preserve, RestoreIfMissing }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Callouts { ConvertKeepEmoji, ConvertStripEmoji, PreserveMarkdown }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tags { Auto, ConfiguredColumns, Off }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Structure {
    pub sections: Sections,
    pub unmatched: Unmatched,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unmatched_folder: Option<String>,
    pub parent_page: ParentPage,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_column: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_columns: Option<Vec<String>>,
    #[serde(default)]
    pub column_widths: BTreeMap<String, u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<TableOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embed_in_articles: Option<bool>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Databases {
    pub location: DatabaseLocation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_name: Option<String>,
    pub output: TableOutput,
    pub embed_in_articles: bool,
    pub title_column: TitleColumn,
    pub widths: Widths,
    #[serde(default)]
    pub overrides: BTreeMap<String, DatabaseOverride>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Content {
    pub toc: Toc,
    pub callouts: Callouts,
    pub tags: Tags,
    #[serde(default)]
    pub tag_columns: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extras {
    pub start_note: bool,
    pub table_css: bool,
    pub table_plugin: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversionOptions {
    pub schema_version: u32,
    pub profile: Profile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_profile: Option<Profile>,
    pub structure: Structure,
    pub databases: Databases,
    pub content: Content,
    pub extras: Extras,
}
impl Default for ConversionOptions {
    fn default() -> Self { Self::knowledge() }
}
impl ConversionOptions {
    pub fn knowledge() -> Self {
        Self {
            schema_version: 1, profile: Profile::Knowledge, base_profile: None,
            structure: Structure {
                sections: Sections::RootOutline, unmatched: Unmatched::NamedFolder,
                unmatched_folder: Some("Другие сервисы".into()),
                parent_page: ParentPage::InsideChildren,
            },
            databases: Databases {
                location: DatabaseLocation::SeparateRoot,
                folder_name: Some("Базы данных".into()),
                output: TableOutput::MarkdownAndBases, embed_in_articles: true,
                title_column: TitleColumn::First, widths: Widths::LongTextPresets,
                overrides: BTreeMap::new(),
            },
            content: Content {
                toc: Toc::RestoreIfMissing, callouts: Callouts::ConvertStripEmoji,
                tags: Tags::Auto, tag_columns: vec![],
            },
            extras: Extras { start_note: true, table_css: true, table_plugin: true },
        }
    }
    pub fn universal() -> Self {
        Self {
            schema_version: 1, profile: Profile::Universal, base_profile: None,
            structure: Structure {
                sections: Sections::Source, unmatched: Unmatched::Source,
                unmatched_folder: None, parent_page: ParentPage::InsideChildren,
            },
            databases: Databases {
                location: DatabaseLocation::Source, folder_name: None,
                output: TableOutput::MarkdownAndBases, embed_in_articles: false,
                title_column: TitleColumn::First, widths: Widths::Uniform,
                overrides: BTreeMap::new(),
            },
            content: Content {
                toc: Toc::Preserve, callouts: Callouts::ConvertKeepEmoji,
                tags: Tags::Auto, tag_columns: vec![],
            },
            extras: Extras { start_note: false, table_css: false, table_plugin: false },
        }
    }
    pub fn custom_from(base: Profile) -> Self {
        let mut options = if base == Profile::Knowledge { Self::knowledge() } else { Self::universal() };
        options.profile = Profile::Custom;
        options.base_profile = Some(if base == Profile::Knowledge { Profile::Knowledge } else { Profile::Universal });
        options
    }
    pub fn from_json(text: &str) -> Result<Self, String> {
        let options: Self = serde_json::from_str(text).map_err(|e| format!("Неверный профиль JSON: {e}"))?;
        options.validate()?;
        Ok(options)
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("Не удалось прочитать профиль: {e}"))?;
        Self::from_json(&text)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, format!("{json}\n")).map_err(|e| format!("Не удалось записать профиль: {e}"))
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 { return Err("Неподдерживаемая версия схемы профиля (нужна 1).".into()); }
        if self.profile == Profile::Custom {
            if !matches!(self.base_profile, Some(Profile::Knowledge | Profile::Universal)) {
                return Err("Пользовательскому профилю требуется base_profile: universal или knowledge.".into());
            }
        } else if self.base_profile.is_some() {
            return Err("Готовый профиль не должен иметь base_profile.".into());
        }
        if self.structure.unmatched == Unmatched::NamedFolder {
            validate_folder(self.structure.unmatched_folder.as_deref(), "unmatched_folder")?;
        }
        if self.databases.location == DatabaseLocation::SeparateRoot {
            validate_folder(self.databases.folder_name.as_deref(), "folder_name")?;
        }
        if self.databases.title_column == TitleColumn::Explicit && self.databases.overrides.is_empty() {
            return Err("title_column: explicit требует overrides для базы.".into());
        }
        if self.content.tags == Tags::ConfiguredColumns && self.content.tag_columns.is_empty()
            && self.databases.overrides.values().all(|o| o.tag_columns.as_ref().is_none_or(Vec::is_empty)) {
            return Err("tags: configured_columns требует хотя бы одну колонку.".into());
        }
        for (uid, o) in &self.databases.overrides {
            if uid.len() != 32 || !uid.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(format!("Неверный UUID базы в overrides: {uid}"));
            }
            if o.title_column.as_ref().is_some_and(|s| s.trim().is_empty()) {
                return Err(format!("Пустая колонка названия для базы {uid}"));
            }
            if o.column_widths.values().any(|&w| !(80..=2000).contains(&w)) {
                return Err(format!("Ширина колонки вне диапазона 80–2000 для базы {uid}"));
            }
            if o.output == Some(TableOutput::BasesOnly) && o.embed_in_articles == Some(true) {
                return Err(format!("Нельзя встроить Markdown-таблицу базы {uid} в режиме bases_only"));
            }
        }
        if self.databases.output == TableOutput::BasesOnly && self.databases.embed_in_articles {
            return Err("Встраивание Markdown-таблиц несовместимо с bases_only.".into());
        }
        Ok(())
    }
    pub fn database_override(&self, uid: &str) -> Option<&DatabaseOverride> {
        self.databases.overrides.get(uid)
    }
    pub fn table_output(&self, uid: &str) -> TableOutput {
        self.database_override(uid).and_then(|x| x.output).unwrap_or(self.databases.output)
    }
    pub fn embed_database(&self, uid: &str) -> bool {
        let requested = self.database_override(uid).and_then(|x| x.embed_in_articles).unwrap_or(self.databases.embed_in_articles);
        requested && self.table_output(uid).markdown()
    }
    pub fn title_key<'a>(&'a self, uid: &str, columns: &'a [String]) -> Option<&'a str> {
        let specific = self.database_override(uid).and_then(|o| o.title_column.as_deref());
        specific.or_else(|| columns.first().map(String::as_str))
    }
    pub fn is_tag_column(&self, uid: &str, col: &str) -> bool {
        let configured = self.database_override(uid).and_then(|o| o.tag_columns.as_ref());
        match self.content.tags {
            Tags::Off => false,
            Tags::ConfiguredColumns => configured.unwrap_or(&self.content.tag_columns).iter().any(|c| c == col),
            Tags::Auto => {
                if let Some(list) = configured { return list.iter().any(|c| c == col); }
                matches!(col.trim().to_lowercase().as_str(), "тег" | "тэг" | "теги" | "тэги" | "tag" | "tags")
            }
        }
    }
    pub fn column_width(&self, uid: &str, name: &str, is_title: bool) -> u32 {
        if let Some(width) = self.database_override(uid).and_then(|x| x.column_widths.get(name)) { return *width; }
        if is_title { return 320; }
        if self.databases.widths == Widths::LongTextPresets
            && matches!(name, "Ответ" | "Примечание" | "Текст" | "Text") { 680 } else { 220 }
    }
}
fn validate_folder(name: Option<&str>, field: &str) -> Result<(), String> {
    let Some(name) = name else { return Err(format!("Требуется {field}")); };
    if name.is_empty() || name != name.trim() || name.ends_with('.')
        || name.chars().any(|c| c.is_control() || "<>:\"/\\|?*#^[]".contains(c)) || name == "." || name == ".."
        || crate::safe_name(name) != name {
        return Err(format!("Недопустимое имя папки в {field}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn presets_are_valid() {
        ConversionOptions::universal().validate().unwrap();
        ConversionOptions::knowledge().validate().unwrap();
        assert_eq!(ConversionOptions::default(), ConversionOptions::knowledge());
    }
    #[test] fn round_trip_and_schema_validation() {
        let mut custom = ConversionOptions::custom_from(Profile::Universal);
        custom.databases.location = DatabaseLocation::SeparateRoot;
        custom.databases.folder_name = Some("Таблицы".into());
        let json = serde_json::to_string(&custom).unwrap();
        assert_eq!(ConversionOptions::from_json(&json).unwrap(), custom);
        let altered = json.replace("\"schema_version\":1", "\"schema_version\":9");
        assert!(ConversionOptions::from_json(&altered).is_err());
        assert!(ConversionOptions::from_json(&json.replace("\"Таблицы\"", "\"CON\"")).is_err());
    }
    #[test] fn per_database_rules() {
        let mut custom = ConversionOptions::custom_from(Profile::Knowledge);
        let uid = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
        custom.databases.overrides.insert(uid.clone(), DatabaseOverride {
            title_column: Some("Name".into()), tag_columns: Some(vec!["Labels".into()]),
            column_widths: BTreeMap::from([("Body".into(), 850)]),
            output: Some(TableOutput::MarkdownOnly), embed_in_articles: Some(false),
        });
        custom.validate().unwrap();
        assert!(custom.is_tag_column(&uid, "Labels"));
        assert!(!custom.is_tag_column(&uid, "Теги"));
        assert_eq!(custom.title_key(&uid, &["Other".into(), "Name".into()]), Some("Name"));
        assert_eq!(custom.column_width(&uid, "Body", false), 850);
    }
}
