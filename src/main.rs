#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#[cfg(windows)]
mod gui;
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
            eprintln!("Использование: notion-to-obsidian ARCHIVE.zip OUTPUT_FOLDER [--inspect]");
            return;
        }
    }
    #[cfg(windows)]
    if args.len() == 1 && args[0] != "--inspect" {
        gui::launch(Some(std::path::PathBuf::from(&args[0])));
        return;
    }
    let inspect = args.iter().any(|s| s == "--inspect");
    let clean = args
        .iter()
        .filter(|s| *s != "--inspect")
        .collect::<Vec<_>>();
    let c = notion_to_obsidian::Converter::new(
        |s| eprintln!("{s}"),
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    );
    let result = if inspect {
        c.inspect(std::path::Path::new(clean[0]))
    } else if clean.len() >= 2 {
        c.run(
            std::path::Path::new(clean[0]),
            std::path::Path::new(clean[1]),
        )
    } else {
        Err("Укажите архив и новую папку результата.".into())
    };
    let code = match result {
        Ok(r) => {
            println!("{}", serde_json::to_string_pretty(&r).unwrap());
            0
        }
        Err(e) => {
            eprintln!("Ошибка: {e}");
            1
        }
    };
    std::process::exit(code)
}
