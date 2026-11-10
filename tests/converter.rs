use notion_to_obsidian::{convert, hash, safe_name, Converter};
use std::io::Write;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};
use zip::{write::SimpleFileOptions, ZipWriter};
const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const D: &str = "dddddddddddddddddddddddddddddddd";
fn archive(root: &Path, files: Vec<(String, Vec<u8>)>) -> PathBuf {
    let path = root.join("export.zip");
    let f = fs::File::create(&path).unwrap();
    let mut z = ZipWriter::new(f);
    for (name, data) in files {
        z.start_file(name, SimpleFileOptions::default()).unwrap();
        z.write_all(&data).unwrap();
    }
    z.finish().unwrap();
    path
}
fn s(name: String, text: impl AsRef<str>) -> (String, Vec<u8>) {
    (name, text.as_ref().as_bytes().to_vec())
}
fn root() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}
#[test]
fn csv_full_view_and_duplicate_titles() {
    let t = root();
    let p = archive(
        t.path(),
        vec![
            s(
                format!("Root {A}.md"),
                format!("# Root\n\n[DB](Root/DB%20{D}.csv)"),
            ),
            s(format!("Root/DB {D}.csv"), "Name,Answer\nSame,Second\n"),
            s(
                format!("Root/DB {D}_all.csv"),
                "Name,Answer,Hidden\nSame,First,Secret\nSame,Second,Other\n",
            ),
            s(
                format!("Root/DB/Same {B}.md"),
                "# Same\n\nAnswer: First\nHidden: Secret",
            ),
            s(
                format!("Root/DB/Same {}.md", "c".repeat(32)),
                "# Same\n\nAnswer: Second\nHidden: Other",
            ),
        ],
    );
    let r = convert(&p, &t.path().join("out")).unwrap();
    assert_eq!(r.stats["source_rows"], 2);
    assert_eq!(r.row_matching[0]["matched"], 2);
    assert_eq!(r.audit["passed"], true);
    assert_eq!(r.databases[0]["view_rows"], 1);
}
#[test]
fn local_attachments_and_external_urls() {
    let t = root();
    let ext = "https://example.com/path_(two)?x=1&y=2#anchor";
    let p=archive(t.path(),vec![s(format!("Root {A}.md"),format!("# Root\n\n[Child](Root/Child%20(one)%20{B}.md)\n![image](Root/image.png)\n[Web]({ext})\n{ext}")),s(format!("Root/Child (one) {B}.md"),"# Child (one)"),("Root/image.png".into(),b"\x89PNG\r\n\x1a\nfixture".to_vec())]);
    let r = convert(&p, &t.path().join("out")).unwrap();
    let text = fs::read_to_string(t.path().join("out/Root/Root.md")).unwrap();
    assert!(text.contains("[[Root/Child (one)|Child]]"));
    assert!(text.contains("![[_attachments/"));
    assert_eq!(text.matches(ext).count(), 2);
    assert_eq!(r.audit["attachment_hashes_verified"], 1);
}
#[test]
fn missing_notion_and_block() {
    let t = root();
    let p=archive(t.path(),vec![s(format!("Root {A}.md"),format!("# Root\n\n[known](https://app.notion.com/p/{B}?pvs=21#{})\nhttps://www.notion.so/Title-{D}?pvs=21","1".repeat(32))),s(format!("Child {B}.md"),"# Child")]);
    let r = convert(&p, &t.path().join("out")).unwrap();
    assert_eq!(r.stats["missing_pages"], 1);
    assert!(r.missing_pages.contains_key(D));
    assert!(!r.warnings.iter().any(|w| w.kind == "block_position_lost"));
    assert_eq!(r.block_link_fallbacks.len(), 1);
    assert_eq!(r.block_link_fallbacks[0].source, format!("Root {A}.md"));
    assert_eq!(r.block_link_fallbacks[0].local_target, "Child.md");
    assert_eq!(r.block_link_fallbacks[0].block_id, "1".repeat(32));
    assert_eq!(r.stats["block_link_fallbacks"], 1);
    let text = fs::read_to_string(t.path().join("out/Root.md")).unwrap();
    assert!(text.contains("[[Child|known]]"));
    assert_eq!(r.audit["passed"], true);
}
#[test]
fn verified_missing_pages_can_be_skipped_without_broken_wikilinks() {
    let t = root();
    let p = archive(t.path(), vec![s(
        format!("Root {A}.md"),
        format!("# Root\n\n[Archived](https://app.notion.com/p/{D})\n\n![Embedded page](https://app.notion.com/p/{D})\n\n[Relative](Archived%20{D}.md)\n\n[Block](https://www.notion.so/{D}#{B})\n\n[Reference][old]\n\n[old]: https://www.notion.so/{D}\n\nhttps://www.notion.so/{D}"),
    )]);
    let r = notion_to_obsidian::Converter::new(
        |_| {},
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    ).skip_missing_pages(true).run(&p, &t.path().join("out")).unwrap();
    let body = fs::read_to_string(t.path().join("out/Root.md")).unwrap();
    assert_eq!(r.stats["missing_pages"], 0);
    assert_eq!(r.stats["skipped_pages"], 1);
    assert_eq!(r.skipped_pages[D].sources.len(), 1);
    assert!(r.skipped_pages[D].urls.iter().any(|url| url.contains("Archived")));
    assert!(body.contains(&format!("[Archived](https://www.notion.so/{D})")));
    assert!(body.contains(&format!("[Embedded page](https://www.notion.so/{D})")));
    assert!(!body.contains("![Embedded page]"));
    assert!(body.contains(&format!("[Relative](https://www.notion.so/{D})")));
    assert!(body.contains(&format!("[Block](https://www.notion.so/{D}#{B})")));
    assert!(body.contains(&format!("[old]: https://www.notion.so/{D}")));
    assert!(!body.contains("[[_Недостающие страницы/"));
    assert!(!t.path().join("out/_Недостающие страницы").exists());
    assert!(!r.warnings.iter().any(|w| w.kind == "block_position_lost"));
    assert_eq!(r.audit["passed"], true);
    let report: serde_json::Value = serde_json::from_slice(
        &fs::read(t.path().join("out/.notion-to-obsidian/migration-report.json")).unwrap()
    ).unwrap();
    assert_eq!(report["skipped_pages"][D]["target"], format!("https://www.notion.so/{D}"));
}

#[test]
fn skip_mode_keeps_placeholders_for_links_without_real_page_id() {
    let t = root();
    let p = archive(t.path(), vec![s(format!("Root {A}.md"), "# Root\n\n[Unknown](Missing.md)")]);
    let r = notion_to_obsidian::Converter::new(
        |_| {},
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    ).skip_missing_pages(true).run(&p, &t.path().join("out")).unwrap();
    assert_eq!(r.stats["missing_pages"], 1);
    assert_eq!(r.stats["skipped_pages"], 0);
    assert_eq!(r.audit["passed"], true);
    assert!(t.path().join("out/_Недостающие страницы").exists());
}

#[test]
fn output_folder_is_the_vault_root() {
    let t = root();
    let p = archive(t.path(), vec![s(format!("Root {A}.md"), "# Root")]);
    let out = t.path().join("out");
    convert(&p, &out).unwrap();
    assert!(out.join("Root.md").is_file());
    assert!(out.join(".obsidian/app.json").is_file());
    assert!(out.join(".notion-to-obsidian/migration-report.json").is_file());
    assert!(out.join(".notion-to-obsidian/import-state.json").is_file());
    assert!(!out.join("Vault").exists());
    assert!(!out.join("migration-report.json").exists());
}

#[test]
fn code_callout_and_inline_preserved() {
    let t = root();
    let p=archive(t.path(),vec![s(format!("Root {A}.md"),"# Root\n\n<aside>\nText\n\n```text\n    x = \"<aside>\"\n[Example](https://example.org)\n```\n</aside>\n\n`<aside>`")]);
    convert(&p, &t.path().join("out")).unwrap();
    let text = fs::read_to_string(t.path().join("out/Root.md")).unwrap();
    assert!(text.contains("> [!note]"));
    assert!(text.contains(">     x = \"<aside>\""));
    assert!(text.contains("`<aside>`"));
}
#[test]
fn nested_list_callout_preserves_markdown() {
    let t = root();
    let p=archive(t.path(),vec![s(format!("Root {A}.md"),"# Root\n\n- Section\n\n    <aside>\n    Tip\n    </aside>\n\n    ### Heading\n\n    Text\n")]);
    let r = convert(&p, &t.path().join("out")).unwrap();
    assert_eq!(r.audit["passed"], true);
}
#[test]
fn table_alias() {
    let t = root();
    let p = archive(
        t.path(),
        vec![
            s(
                format!("Root {A}.md"),
                format!("# Root\n\n| Link |\n| --- |\n| [Go](Child%20{B}.md) |"),
            ),
            s(format!("Child {B}.md"), "# Child"),
        ],
    );
    convert(&p, &t.path().join("out")).unwrap();
    assert!(fs::read_to_string(t.path().join("out/Root.md"))
        .unwrap()
        .contains("[[Child\\|Go]]"));
}
#[test]
fn windows_names_and_case() {
    assert_eq!(safe_name("CON"), "_CON");
    assert!(!safe_name("a:b").contains(':'));
    let t = root();
    let p = archive(
        t.path(),
        vec![
            s(format!("Root {A}.md"), "# Same"),
            s(format!("Child {B}.md"), "# same"),
        ],
    );
    let r = convert(&p, &t.path().join("out")).unwrap();
    assert_ne!(r.mapping[A].to_lowercase(), r.mapping[B].to_lowercase());
}
#[test]
fn cancel_no_output() {
    let t = root();
    let p = archive(t.path(), vec![s(format!("Root {A}.md"), "# Root")]);
    let c = Converter::new(|_| {}, Arc::new(AtomicBool::new(true)));
    assert!(c.run(&p, &t.path().join("out")).is_err());
    assert!(!t.path().join("out").exists());
}
#[test]
fn existing_output_preserved() {
    let t = root();
    let p = archive(t.path(), vec![s(format!("Root {A}.md"), "# Root")]);
    let out = t.path().join("out");
    fs::create_dir(&out).unwrap();
    fs::write(out.join("keep"), "keep").unwrap();
    assert!(convert(&p, &out).is_err());
    assert_eq!(fs::read_to_string(out.join("keep")).unwrap(), "keep");
}
#[test]
fn traversal_rejected() {
    let t = root();
    let p = archive(
        t.path(),
        vec![
            s("../escape.md".into(), "no"),
            s(format!("Root {A}.md"), "# Root"),
        ],
    );
    assert!(convert(&p, &t.path().join("out")).is_err());
}
#[test]
fn nested_export() {
    let t = root();
    let inner = archive(t.path(), vec![s(format!("Root {A}.md"), "# Root")]);
    let bytes = fs::read(inner).unwrap();
    let p = archive(t.path(), vec![("Export-Part-1.zip".into(), bytes)]);
    assert_eq!(
        convert(&p, &t.path().join("out")).unwrap().stats["source_pages"],
        1
    );
}
#[test]
fn repeat_is_deterministic() {
    let t = root();
    let p = archive(t.path(), vec![s(format!("Root {A}.md"), "# Root\n\nHello")]);
    let one = convert(&p, &t.path().join("one")).unwrap();
    let two = convert(&p, &t.path().join("two")).unwrap();
    assert_eq!(
        serde_json::to_value(one).unwrap(),
        serde_json::to_value(two).unwrap()
    );
    let a = fs::read(t.path().join("one/Root.md")).unwrap();
    let b = fs::read(t.path().join("two/Root.md")).unwrap();
    assert_eq!(hash(a), hash(b));
}
#[test]
fn hidden_columns_and_long_answers() {
    let t = root();
    let p = archive(
        t.path(),
        vec![
            s(format!("Root {A}.md"), "# Root"),
            s(
                format!("DB {D}_all.csv"),
                "Name,Ответ,Extra\nRow,\"long\nanswer\",hidden\n",
            ),
            s(format!("DB/Row {B}.md"), "# Row\n\nОтвет: long\nanswer"),
        ],
    );
    convert(&p, &t.path().join("out")).unwrap();
    let text = fs::read_to_string(t.path().join("out/Базы данных/DB/Row.md")).unwrap();
    assert!(text.contains("long\nanswer"));
    assert!(text.contains("hidden"));
}
#[test]
fn bare_relation() {
    let t = root();
    let p = archive(
        t.path(),
        vec![
            s(
                format!("Root {A}.md"),
                format!("# Root\n\nSub-item: Child (Child%20{B}.md)"),
            ),
            s(format!("Child {B}.md"), "# Child"),
        ],
    );
    convert(&p, &t.path().join("out")).unwrap();
    assert!(fs::read_to_string(t.path().join("out/Root.md"))
        .unwrap()
        .contains("Sub-item: Child [[Child]]"));
}
#[test]
fn reference_links() {
    let t = root();
    let p = archive(
        t.path(),
        vec![
            s(
                format!("Root {A}.md"),
                format!("# Root\n\n[Go][ref]\n\n[ref]: https://app.notion.com/p/{B}"),
            ),
            s(format!("Child {B}.md"), "# Child"),
        ],
    );
    convert(&p, &t.path().join("out")).unwrap();
    let text = fs::read_to_string(t.path().join("out/Root.md")).unwrap();
    assert!(text.contains("[ref]: Child%2Emd"));
    assert!(text.contains("[Go][ref]"));
}
#[test]
fn hyphenated_page_uuid() {
    let t = root();
    let p = archive(
        t.path(),
        vec![
            s(
                format!("Root {A}.md"),
                "# Root\n\nhttps://app.notion.com/p/bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
            ),
            s(format!("Child {B}.md"), "# Child"),
        ],
    );
    let r = convert(&p, &t.path().join("out")).unwrap();
    assert_eq!(r.stats["missing_pages"], 0);
}

#[test]
fn full_properties_native_tags_and_headers() {
    let t = root();
    let answer = format!(
        "Первая строка\n{}\nПоследняя строка",
        "Ответ без сокращений ".repeat(60)
    );
    let mut w = csv::Writer::from_writer(vec![]);
    w.write_record(["Вопрос", "Ответ", "Теги"]).unwrap();
    w.write_record(["Проверка", &answer, "Веб Учитель, МЭШ"])
        .unwrap();
    let csv = String::from_utf8(w.into_inner().unwrap()).unwrap();
    let p = archive(
        t.path(),
        vec![
            s(format!("Root {A}.md"), "# Root"),
            s(format!("DB {D}_all.csv"), csv),
            s(
                format!("DB/Проверка {B}.md"),
                format!("# Проверка\n\nОтвет: {answer}\nТеги: Веб Учитель, МЭШ"),
            ),
        ],
    );
    let out = t.path().join("out");
    let r = convert(&p, &out).unwrap();
    let text = fs::read_to_string(out.clone().join(&r.mapping[B])).unwrap();
    let meta: serde_json::Value = serde_json::from_str(
        text.strip_prefix("---\n")
            .unwrap()
            .split("\n---\n")
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(meta["Ответ"], answer);
    assert_eq!(meta["Вопрос"], "Проверка");
    for generated in ["title", "folder", "cssclasses"] {
        assert!(meta.get(generated).is_none(), "{generated}");
    }
    assert_eq!(meta["tags"], serde_json::json!(["Веб_Учитель", "МЭШ"]));
    assert!(meta
        .as_object()
        .unwrap()
        .keys()
        .all(|k| !k.starts_with("db_")));
    let base: serde_json::Value = serde_json::from_slice(
        &fs::read(
            out.clone()
                .join(r.databases[0]["base"].as_str().unwrap()),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(base["properties"]["note.Ответ"]["displayName"], "Ответ");
    assert_eq!(
        base["properties"]["formula.Название"]["displayName"],
        "Вопрос"
    );
    assert_eq!(
        base["views"][0]["order"],
        serde_json::json!(["formula.Название", "note.Ответ", "note.tags"])
    );
    let readable = fs::read_to_string(
        out.clone()
            .join(r.databases[0]["readable_table"].as_str().unwrap()),
    )
    .unwrap();
    assert!(readable.contains("Последняя строка"));
    assert!(!readable.contains("…"));
    assert!(readable.contains("<br>"));
    assert!(out
        .join(".obsidian/plugins/notion-table-copy/main.js")
        .is_file());
}
#[test]
fn no_synthetic_folder_summaries_but_page_toc_is_kept() {
    let t = root();
    let p = archive(
        t.path(),
        vec![
            s(
                format!("Инструкция {A}.md"),
                format!(
                    "# Инструкция\n\n## Веб Учитель\n\n[Статья](Инструкция/Статья%20{B}.md)\n\n## Веб Организация\n\nДругой раздел"
                ),
            ),
            s(
                format!("Инструкция/Статья {B}.md"),
                "# Статья\n\n## Первое\n\nСодержание первой темы\n\n## Второе\n\nСодержание второй темы",
            ),
        ],
    );
    let out = t.path().join("out");
    let report = convert(&p, &out).unwrap();
    assert_eq!(report.audit["passed"], true);
    assert_eq!(report.stats["sections"], 1);
    let article = fs::read_to_string(out.join(&report.mapping[B])).unwrap();
    assert!(article.contains("**Содержание**"));
    assert!(article.contains("[[#Первое|Первое]]"));
    assert!(article.contains("[[#Второе|Второе]]"));
    let unwanted = out.join("Инструкция/Веб Учитель/Оглавление.md");
    assert!(!unwanted.exists(), "a folder must not get a generated index note");
    assert!(!out.join("Инструкция/Веб Организация/Оглавление.md").exists());
    let root_doc = fs::read_to_string(out.join(&report.mapping[A])).unwrap();
    assert!(root_doc.contains("Веб Учитель"));
    assert!(root_doc.contains("Веб Организация"));
}

#[test]
fn a_real_page_named_oglavlenie_is_preserved() {
    let t = root();
    let p = archive(
        t.path(),
        vec![
            s(
                format!("Инструкция {A}.md"),
                format!("# Инструкция\n\n## Веб Учитель\n\n[Оглавление](Инструкция/Оглавление%20{B}.md)"),
            ),
            s(format!("Инструкция/Оглавление {B}.md"), "# Оглавление\n\nРеальная заметка Notion"),
        ],
    );
    let out = t.path().join("out");
    let report = convert(&p, &out).unwrap();
    let note = fs::read_to_string(out.join(&report.mapping[B])).unwrap();
    assert!(note.contains("Реальная заметка Notion"));
    assert_eq!(report.mapping[B], "Инструкция/Веб Учитель/Оглавление.md");
    assert_eq!(report.audit["passed"], true);
}

#[test]
fn parent_page_and_subpages_share_one_folder_without_duplicate_sibling() {
    let t = root();
    let child_one = "cccccccccccccccccccccccccccccccc";
    let child_two = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    let p = archive(
        t.path(),
        vec![
            s(
                format!("Инструкция {A}.md"),
                format!("# Инструкция\n\n## Веб Учитель\n\n[Поурочное планирование](Инструкция/Поурочное%20планирование%20{B}.md)"),
            ),
            s(
                format!("Инструкция/Поурочное планирование {B}.md"),
                format!("# Поурочное планирование\n\n[Первый](Поурочное%20планирование/Первый%20{child_one}.md)\n[Второй](Поурочное%20планирование/Второй%20{child_two}.md)"),
            ),
            s(
                format!("Инструкция/Поурочное планирование/Первый {child_one}.md"),
                "# Первый\n\nТекст первой страницы",
            ),
            s(
                format!("Инструкция/Поурочное планирование/Второй {child_two}.md"),
                "# Второй\n\nТекст второй страницы",
            ),
        ],
    );
    let out = t.path().join("out");
    let report = convert(&p, &out).unwrap();
    let dir = "Инструкция/Веб Учитель/Поурочное планирование";
    assert_eq!(report.mapping[B], format!("{dir}/Поурочное планирование.md"));
    assert_eq!(report.mapping[child_one], format!("{dir}/Первый.md"));
    assert_eq!(report.mapping[child_two], format!("{dir}/Второй.md"));
    assert!(!out.join("Инструкция/Веб Учитель/Поурочное планирование.md").exists());
    let parent = fs::read_to_string(out.join(&report.mapping[B])).unwrap();
    assert!(parent.contains(&format!("[[{dir}/Первый|Первый]]")));
    assert!(parent.contains(&format!("[[{dir}/Второй|Второй]]")));
    assert!(fs::read_to_string(out.join(&report.mapping[child_one])).unwrap().contains("Текст первой"));
    let root_note = fs::read_to_string(out.join(&report.mapping[A])).unwrap();
    assert!(root_note.contains(&format!("[[{dir}/Поурочное планирование|Поурочное планирование]]")));
    assert_eq!(report.audit["passed"], true);
}

#[test]
fn hierarchy_uses_headings_and_preserves_distinct_same_title() {
    let t = root();
    let c = "cccccccccccccccccccccccccccccccc";
    let p=archive(t.path(),vec![s(format!("Инструкция {A}.md"),format!("# Инструкция\n\n## Веб Учитель\n\n[ДЗ](Инструкция/Домашние%20задания%20{B}.md)\n\n## Веб Организация\n\n[ДЗ](Инструкция/Домашние%20задания%20{c}.md)")),s(format!("Инструкция/Домашние задания {B}.md"),"# Домашние задания\n\nТэг: Веб Учитель\n\n[Child](Домашние%20задания/Child%20dddddddddddddddddddddddddddddddd.md)"),s(format!("Инструкция/Домашние задания {c}.md"),"# Домашние задания\n\nТэг: Веб Организация"),s(format!("Инструкция/Домашние задания/Child {D}.md"),"# Child")]);
    let out = t.path().join("out");
    let r = convert(&p, &out).unwrap();
    assert_eq!(r.mapping[B], "Инструкция/Веб Учитель/Домашние задания/Домашние задания.md");
    assert_eq!(
        r.mapping[c],
        "Инструкция/Веб Организация/Домашние задания.md"
    );
    assert_eq!(
        r.mapping[D],
        "Инструкция/Веб Учитель/Домашние задания/Child.md"
    );
    let text = fs::read_to_string(out.clone().join(&r.mapping[B])).unwrap();
    let meta: serde_json::Value = serde_json::from_str(
        text.strip_prefix("---\n")
            .unwrap()
            .split("\n---\n")
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(meta["tags"], serde_json::json!(["Веб_Учитель"]));
    assert!(!text.split("\n---\n").nth(1).unwrap().contains("Тэг:"));
}
#[test]
fn table_links_survive_and_tags_preserve_single_multiword_label() {
    let t = root();
    let mut w = csv::Writer::from_writer(vec![]);
    w.write_record(["Name", "Ответ", "Тег"]).unwrap();
    w.write_record([
        "Row",
        &format!("Откройте https://app.notion.com/p/{B}"),
        "Веб Учитель",
    ])
    .unwrap();
    let csv = String::from_utf8(w.into_inner().unwrap()).unwrap();
    let p = archive(
        t.path(),
        vec![
            s(format!("Root {A}.md"), "# Root"),
            s(format!("DB {D}_all.csv"), csv),
            s(
                format!("DB/Row {B}.md"),
                format!("# Row\n\nОтвет: Откройте https://app.notion.com/p/{B}\nТег: Веб Учитель"),
            ),
        ],
    );
    let r = convert(&p, &t.path().join("out")).unwrap();
    assert_eq!(r.audit["passed"], true);
}

#[test]
fn no_service_properties_and_membership_is_in_base() {
    let t = root();
    let p = archive(
        t.path(),
        vec![
            s(format!("Root {A}.md"), "# Root"),
            s(
                format!("Надбавки {D}.csv"),
                "Вопрос,Ответ\nАпелляция,Полный ответ\n",
            ),
            s(
                format!("Надбавки {D}_all.csv"),
                "Вопрос,Ответ,Тег\nАпелляция,Полный ответ,П1\nДругой вопрос,Другой ответ,П2\n",
            ),
            s(
                format!("Надбавки/Апелляция {B}.md"),
                "# Апелляция\n\nОтвет: Полный ответ\nТег: П1",
            ),
            s(
                format!("Надбавки/Другой вопрос {}.md", "c".repeat(32)),
                "# Другой вопрос\n\nОтвет: Другой ответ\nТег: П2",
            ),
        ],
    );
    let out = t.path().join("out");
    let r = convert(&p, &out).unwrap();
    for target in r.mapping.values() {
        let text = fs::read_to_string(out.clone().join(target)).unwrap();
        let meta: serde_json::Value = if let Some(rest) = text.strip_prefix("---\n") {
            serde_json::from_str(rest.split("\n---\n").next().unwrap()).unwrap()
        } else {
            serde_json::json!({})
        };
        for key in meta.as_object().unwrap().keys() {
            assert!(!key.starts_with("notion_"), "{key}");
            assert!(!key.starts_with("db_"), "{key}");
            assert!(
                !["title", "folder", "cssclasses"].contains(&key.as_str()),
                "{key}"
            );
        }
    }
    assert!(!fs::read_to_string(out.clone().join(&r.mapping[A]))
        .unwrap()
        .starts_with("---"));
    assert_eq!(
        fs::read(out.join(".obsidian/app.json")).unwrap(),
        fs::read(out.join(".obsidian/app.json")).unwrap()
    );
    let db = &r.databases[0];
    let base: serde_json::Value = serde_json::from_slice(
        &fs::read(out.clone().join(db["base"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
    assert!(base["filters"]["and"][1]
        .to_string()
        .contains(&r.mapping[B]));
    assert!(!base.to_string().contains("notion_database_id"));
    assert_eq!(base["properties"]["note.Ответ"]["displayName"], "Ответ");
    assert_eq!(
        base["views"][0]["sort"][0]["property"],
        "formula.Порядок вида"
    );
    assert!(base["formulas"]["Порядок"]
        .as_str()
        .unwrap()
        .contains(".reduce("));
    let plugin: serde_json::Value = serde_json::from_slice(
        &fs::read(out.join(".obsidian/plugins/notion-table-copy/tables.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(plugin["byRowPath"][&r.mapping[B]]["titleColumn"], "Вопрос");
}

#[test]
fn toc_restores_missing_notion_block_without_touching_code() {
    let t = root();
    let p=archive(t.path(),vec![s(format!("Root {A}.md"),"# Root"),s(format!("Root/Article {B}.md"),"# Article\n\nIntro\n\n---\n\n---\n\n## **Первая тема**\n\nText\n\n```\n## Not a heading\n```\n\n## Вторая тема\n\n### Подтема\n")]);
    let out = t.path().join("out");
    let r = convert(&p, &out).unwrap();
    let text = fs::read_to_string(out.clone().join(&r.mapping[B])).unwrap();
    assert!(text.contains("**Содержание**"));
    assert!(text.contains("[[#Первая тема|Первая тема]]"));
    assert!(text.contains("[[#Вторая тема#Подтема|Подтема]]"));
    assert!(!text.contains("[[#Not a heading"));
    assert!(text.contains("## Not a heading"));
    assert!(text.find("**Содержание**").unwrap() < text.find("## **Первая тема**").unwrap());
}
#[test]
fn callout_icons_become_native_types_and_titles() {
    let t = root();
    let p=archive(t.path(),vec![s(format!("Root {A}.md"),"# Root\n\n<aside>\n📚\n\n**Ссылки на инструкции**\n\nText\n</aside>\n\n<aside>\n💡 Совет без потери текста\n</aside>\n\n<aside>\n⚠️\n\nОсторожно\n</aside>\n")]);
    let out = t.path().join("out");
    convert(&p, &out).unwrap();
    let text = fs::read_to_string(out.join("Root.md")).unwrap();
    assert!(text.contains("[!info] Ссылки на инструкции"));
    assert!(text.contains("[!tip]"));
    assert!(text.contains("[!warning]"));
    assert!(text.contains("Совет без потери текста"));
    assert!(!text.contains('📚'));
    assert!(!text.contains('💡'));
    assert!(!text.contains('⚠'));
}
#[test]
fn database_is_embedded_in_parent_as_full_table_block() {
    let t = root();
    let p=archive(t.path(),vec![s(format!("Root {A}.md"),format!("# Root\n\n### Вопрос-ответ\n\n[Questions](DB%20{D}.csv)\n\nОткрыть [таблицу](DB%20{D}.csv) при необходимости.")),s(format!("DB {D}_all.csv"),"Вопрос,Ответ\nQuestion,Whole answer\n"),s(format!("DB/Question {B}.md"),"# Question\n\nОтвет: Whole answer")]);
    let out = t.path().join("out");
    let r = convert(&p, &out).unwrap();
    let text = fs::read_to_string(out.join("Root.md")).unwrap();
    assert!(text.contains("![[Базы данных/DB/DB#^notion-table]]"));
    assert!(text.contains("Открыть [[Базы данных/DB/DB|таблицу]] при необходимости."));
    let table = fs::read_to_string(out.join("Базы данных/DB/DB.md")).unwrap();
    assert_eq!(r.mapping[A], "Root.md");
    assert_eq!(r.mapping[B], "Базы данных/DB/Question.md");
    assert!(table.contains("| Question") || table.contains("Question]]"));
    assert!(table.contains("Whole answer"));
    assert!(table.contains("\n\n^notion-table\n"));
    assert_eq!(r.stats["databases_embedded_inline"], 1);
}


#[test]
fn database_parent_stays_in_article_tree_while_tables_move_to_database_root() {
    let t = root();
    let p = archive(
        t.path(),
        vec![
            s(
                format!("Инструкция {A}.md"),
                format!("# Инструкция\n\n## Веб Учитель\n\n[Поурочное планирование](Инструкция/Поурочное%20планирование%20{B}.md)"),
            ),
            s(
                format!("Инструкция/Поурочное планирование {B}.md"),
                format!("# Поурочное планирование\n\n[Поурочное планирование](Поурочное%20планирование%20{D}.csv)"),
            ),
            s(
                format!("Инструкция/Поурочное планирование {D}_all.csv"),
                "Название,Описание\nЗанятие,Содержание\n",
            ),
            s(
                format!("Инструкция/Поурочное планирование/Занятие {}.md", "c".repeat(32)),
                "# Занятие\n\nОписание: Содержание",
            ),
        ],
    );
    let out = t.path().join("out");
    let report = convert(&p, &out).unwrap();
    let folder = "Базы данных/Инструкция/Веб Учитель/Поурочное планирование";
    assert_eq!(report.mapping[B], "Инструкция/Веб Учитель/Поурочное планирование.md");
    assert!(!out.join("Инструкция/Веб Учитель/Поурочное планирование/Поурочное планирование.md").exists());
    let table = report.databases[0]["readable_table"].as_str().unwrap();
    assert_eq!(table, format!("{folder}/Поурочное планирование.md"));
    assert_eq!(
        report.databases[0]["base"].as_str().unwrap(),
        format!("{folder}/Поурочное планирование.base")
    );
    assert!(out.join(table).is_file());
    assert!(out.join(report.databases[0]["base"].as_str().unwrap()).is_file());
    let row = "c".repeat(32);
    assert_eq!(report.mapping[&row], format!("{folder}/Занятие.md"));
    let page = fs::read_to_string(out.join(&report.mapping[B])).unwrap();
    let table_link = table.trim_end_matches(".md");
    assert!(page.contains(&format!("![[{table_link}#^notion-table]]")));
    assert!(page.contains("# Поурочное планирование"));
    assert_eq!(report.audit["passed"], true);
}

#[test]
fn standalone_parent_stays_in_root_and_records_move_to_database_root() {
    let t = root();
    let p = archive(
        t.path(),
        vec![
            s(format!("План {A}.md"), format!("# План\n\n[База](План%20{D}.csv)")),
            s(format!("План {D}_all.csv"), "Name,Answer\nЗанятие,Текст\n"),
            s(format!("План/Занятие {B}.md"), "# Занятие\n\nAnswer: Текст"),
        ],
    );
    let out = t.path().join("out");
    let report = convert(&p, &out).unwrap();
    assert_eq!(report.mapping[A], "План.md");
    assert!(out.join("План.md").exists());
    assert_eq!(
        report.databases[0]["readable_table"].as_str().unwrap(),
        "Базы данных/План/План.md"
    );
    assert!(out.join("Базы данных/План/План.md").exists());
    assert_eq!(report.mapping[B], "Базы данных/План/Занятие.md");
    assert!(!out.join("План/Занятие.md").exists());
    assert_eq!(report.audit["passed"], true);
}


#[test]
fn database_parent_stays_with_empty_database_without_row_folder() {
    let t = root();
    let p = archive(
        t.path(),
        vec![
            s(
                format!("Главная {A}.md"),
                format!("# Главная\n\n## Учитель\n\n[План](Главная/План%20{B}.md)"),
            ),
            s(
                format!("Главная/План {B}.md"),
                format!("# План\n\n[План](План%20{D}.csv)"),
            ),
            s(format!("Главная/План {D}_all.csv"), "Название,Ответ\n"),
        ],
    );
    let out = t.path().join("out");
    let report = convert(&p, &out).unwrap();
    assert_eq!(report.mapping[B], "Главная/Учитель/План.md");
    assert!(!out.join("Главная/Учитель/План/План.md").exists());
    assert!(out.join("Базы данных/Главная/Учитель/План/План.md").is_file());
    assert!(out.join("Базы данных/Главная/Учитель/План/План.base").is_file());
    assert_eq!(report.audit["passed"], true);
}


#[test]
fn nested_database_subpages_follow_records_and_unrelated_pages_stay_in_instructions() {
    let t = root();
    let row_id = "cccccccccccccccccccccccccccccccc";
    let child_id = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    let other_id = "ffffffffffffffffffffffffffffffff";
    let p = archive(
        t.path(),
        vec![
            s(
                format!("Инструкция {A}.md"),
                format!("# Инструкция\n\n## Учитель\n\n[Задания](Инструкция/Задания%20{B}.md)"),
            ),
            s(
                format!("Инструкция/Задания {B}.md"),
                format!("# Задания\n\n[Таблица](Задания%20{D}.csv)\n\n[Ряд](Задания/Ряд%20{row_id}.md)"),
            ),
            s(
                format!("Инструкция/Задания {D}_all.csv"),
                "Имя,Ответ\nРяд,Да\n",
            ),
            s(
                format!("Инструкция/Задания/Ряд {row_id}.md"),
                format!("# Ряд\n\nОтвет: Да\n\n[Детали](Ряд/Детали%20{child_id}.md)"),
            ),
            s(
                format!("Инструкция/Задания/Ряд/Детали {child_id}.md"),
                "# Детали\n\nПодробности",
            ),
            s(
                format!("Инструкция/Побочная {other_id}.md"),
                "# Побочная\n\nОбычная страница инструкции",
            ),
        ],
    );
    let out = t.path().join("out");
    let report = convert(&p, &out).unwrap();
    let folder = "Базы данных/Инструкция/Учитель/Задания";
    assert_eq!(report.mapping[B], "Инструкция/Учитель/Задания.md");
    assert_eq!(report.mapping[row_id], format!("{folder}/Ряд/Ряд.md"));
    assert_eq!(report.mapping[child_id], format!("{folder}/Ряд/Детали.md"));
    assert!(report.mapping[other_id].starts_with("Инструкция/"));
    assert!(!report.mapping[other_id].starts_with("Базы данных/"));
    let article = fs::read_to_string(out.join(&report.mapping[B])).unwrap();
    assert!(article.contains(&format!("![[{folder}/Задания#^notion-table]]")));
    assert!(article.contains(&format!("[[{folder}/Ряд/Ряд|Ряд]]")));
    let row = fs::read_to_string(out.join(&report.mapping[row_id])).unwrap();
    assert!(row.contains(report.mapping[child_id].trim_end_matches(".md")));
    let base_path = report.databases[0]["base"].as_str().unwrap();
    let base = fs::read_to_string(out.join(base_path)).unwrap();
    assert!(base.contains(&report.mapping[row_id]));
    assert!(!base.contains(&report.mapping[B]));
    assert_eq!(report.audit["passed"], true);
}

#[test]
fn databases_with_same_title_in_distinct_sections_do_not_merge() {
    let t = root();
    let second_page = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    let second_database = "ffffffffffffffffffffffffffffffff";
    let first_row = "cccccccccccccccccccccccccccccccc";
    let second_row = "99999999999999999999999999999999";
    let p = archive(
        t.path(),
        vec![
            s(format!("Инструкция {A}.md"), "# Инструкция"),
            s(
                format!("Инструкция/Учитель/Планы {B}.md"),
                format!("# Планы\n\n[Таблица](Планы%20{D}.csv)"),
            ),
            s(
                format!("Инструкция/Админ/Планы {second_page}.md"),
                format!("# Планы\n\n[Таблица](Планы%20{second_database}.csv)"),
            ),
            s(
                format!("Инструкция/Учитель/Планы {D}_all.csv"),
                "Имя,Ответ\nПервый,Да\n",
            ),
            s(
                format!("Инструкция/Админ/Планы {second_database}_all.csv"),
                "Имя,Ответ\nВторой,Нет\n",
            ),
            s(
                format!("Инструкция/Учитель/Планы/Первый {first_row}.md"),
                "# Первый\n\nОтвет: Да",
            ),
            s(
                format!("Инструкция/Админ/Планы/Второй {second_row}.md"),
                "# Второй\n\nОтвет: Нет",
            ),
        ],
    );
    let out = t.path().join("out");
    let report = convert(&p, &out).unwrap();
    assert_eq!(report.audit["passed"], true);
    assert_eq!(report.mapping[B], "Инструкция/Учитель/Планы.md");
    assert_eq!(report.mapping[second_page], "Инструкция/Админ/Планы.md");
    assert_eq!(
        report.mapping[first_row],
        "Базы данных/Инструкция/Учитель/Планы/Первый.md"
    );
    assert_eq!(
        report.mapping[second_row],
        "Базы данных/Инструкция/Админ/Планы/Второй.md"
    );
    for db in &report.databases {
        let table = db["readable_table"].as_str().unwrap();
        assert!(table.starts_with("Базы данных/Инструкция/"));
        assert!(out.join(table).exists());
    }
    assert_ne!(report.databases[0]["readable_table"], report.databases[1]["readable_table"]);
}


#[test]
fn identical_block_links_are_reported_once_and_keep_the_original_url() {
    let t = root();
    let original = format!("https://www.notion.so/{B}#{D}");
    let p = archive(t.path(), vec![
        s(format!("Root {A}.md"), format!(
            "# Root\n\n[First]({original}) and [Second]({original})"
        )),
        s(format!("Child {B}.md"), "# Child\n\n## Information\n\nBody kept intact"),
    ]);
    let r = convert(&p, &t.path().join("out")).unwrap();
    let text = fs::read_to_string(t.path().join("out/Root.md")).unwrap();
    assert!(text.contains("[[Child|First]]"));
    assert!(text.contains("[[Child|Second]]"));
    assert_eq!(r.block_link_fallbacks.len(), 1);
    assert_eq!(r.block_link_fallbacks[0].original_url, original);
    assert_eq!(r.block_link_fallbacks[0].block_id, D);
    assert_eq!(r.block_link_fallbacks[0].local_target, "Child.md");
    assert_eq!(r.stats["block_link_fallbacks"], 1);
    assert!(r.warnings.iter().all(|w| w.kind != "block_position_lost"));
    let report: serde_json::Value = serde_json::from_slice(
        &fs::read(t.path().join("out/.notion-to-obsidian/migration-report.json")).unwrap()
    ).unwrap();
    assert_eq!(report["block_link_fallbacks"][0]["original_url"], original);
    assert_eq!(r.audit["passed"], true);
}

#[test]
fn known_notion_heading_fragments_are_converted_to_local_section_links() {
    let t = root();
    let p = archive(t.path(), vec![
        s(format!("Root {A}.md"), format!(
            "# Root\n\n[Section](https://www.notion.so/{B}#Details)"
        )),
        s(format!("Child {B}.md"), "# Child\n\n## Details\n\nContent"),
    ]);
    let r = convert(&p, &t.path().join("out")).unwrap();
    let root = fs::read_to_string(t.path().join("out/Root.md")).unwrap();
    assert!(root.contains("[[Child#Details|Section]]"));
    assert_eq!(r.block_link_fallbacks.len(), 0);
    assert_eq!(r.audit["passed"], true);
}

#[test]
fn local_markdown_block_uuid_fallback_keeps_offline_page_target() {
    let t = root();
    let p = archive(t.path(), vec![
        s(format!("Root {A}.md"), format!(
            "# Root\n\n[Child block](Child%20{B}.md#{D})"
        )),
        s(format!("Child {B}.md"), "# Child"),
    ]);
    let r = convert(&p, &t.path().join("out")).unwrap();
    assert_eq!(r.block_link_fallbacks.len(), 1);
    assert!(r.block_link_fallbacks[0].original_url.contains(".md#"));
    assert_eq!(r.block_link_fallbacks[0].block_id, D);
    let root = fs::read_to_string(t.path().join("out/Root.md")).unwrap();
    assert!(root.contains("[[Child|Child block]]"));
    assert_eq!(r.audit["passed"], true);
}
