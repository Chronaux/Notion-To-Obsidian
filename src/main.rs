#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#[cfg(windows)]
mod gui;

use notion_to_obsidian::{options::{ConversionOptions, Profile}, Converter};

fn main() {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        #[cfg(windows)]
        {
            gui::launch(None);
            return;
        }
        #[cfg(not(windows))]
        {
            eprintln!("Использование: notion-to-obsidian ARCHIVE.zip OUTPUT_FOLDER [--profile universal|knowledge] [--config FILE.json] [--skip-missing] [--inspect]");
            return;
        }
    }
    #[cfg(windows)]
    if args.len() == 1 && args[0] != "--inspect" && args[0] != "--skip-missing"
        && args[0] != "--profile" && args[0] != "--config" {
        gui::launch(Some(std::path::PathBuf::from(&args[0])));
        return;
    }
    let result = run_cli(args);
    let code = match result {
        Ok(report) => {
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            0
        }
        Err(message) => { eprintln!("Ошибка: {message}"); 1 }
    };
    std::process::exit(code)
}
fn run_cli(args: Vec<std::ffi::OsString>) -> Result<notion_to_obsidian::Report, String> {
    let mut inspect = false;
    let mut skip_missing = false;
    let mut config = None;
    let mut preset = None;
    let mut clean = Vec::new();
    let mut items = args.into_iter();
    while let Some(item) = items.next() {
        match item.to_string_lossy().as_ref() {
            "--inspect" => inspect = true,
            "--skip-missing" => skip_missing = true,
            "--profile" => {
                let name = items.next().ok_or("После --profile укажите universal или knowledge")?;
                preset = Some(match name.to_string_lossy().as_ref() {
                    "universal" => Profile::Universal,
                    "knowledge" => Profile::Knowledge,
                    _ => return Err("Допустимые значения --profile: universal, knowledge".into()),
                });
            }
            "--config" => {
                config = Some(items.next().ok_or("После --config укажите путь JSON-профиля")?);
            }
            "--help" | "-h" => return Err("Использование: notion-to-obsidian ARCHIVE.zip OUTPUT_FOLDER [--profile universal|knowledge] [--config FILE.json] [--skip-missing] [--inspect]".into()),
            arg if arg.starts_with("--") => return Err(format!("Неизвестный параметр: {arg}")),
            _ => clean.push(item),
        }
    }
    if config.is_some() && preset.is_some() {
        return Err("Выберите --profile или --config, но не оба одновременно.".into());
    }
    let options = match (preset, config) {
        (_, Some(path)) => ConversionOptions::load(std::path::Path::new(&path))?,
        (Some(Profile::Universal), _) => ConversionOptions::universal(),
        _ => ConversionOptions::knowledge(), // old CLI behavior is unchanged
    };
    let c = Converter::new(
        |s| eprintln!("{s}"),
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    ).skip_missing_pages(skip_missing).options(options);
    if inspect && !clean.is_empty() {
        c.inspect(std::path::Path::new(&clean[0]))
    } else if clean.len() >= 2 {
        c.run(std::path::Path::new(&clean[0]), std::path::Path::new(&clean[1]))
    } else {
        Err("Укажите архив и новую папку результата.".into())
    }
}
