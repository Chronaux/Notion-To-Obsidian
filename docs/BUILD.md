# Сборка и тестирование

Для запуска готового Windows-приложения инструменты разработчика не нужны. Этот раздел относится только к сборке исходников.

## Проверка ядра

Установите стабильный Rust и выполните из корня репозитория:

```sh
cargo test --locked
cargo build --release --locked
node tests/ui.cjs  # Проверка ICO, версии и исходников Win32
```

В Linux соберётся консольный конвертер. Нативное окно включается только для Windows.

## Windows x64

Текущий процесс сборки использует GNU-цель и MinGW-w64, включая `windres` для manifest/version. Эти инструменты должны быть в PATH. Сборка через MSVC отдельно не проверялась.

Для кросс-компиляции из Linux нужны Rust, GNU-цель Windows и MinGW-w64, включая `x86_64-w64-mingw32-gcc` и `x86_64-w64-mingw32-windres` в PATH:

```sh
rustup target add x86_64-pc-windows-gnu
cargo build --release --locked --target x86_64-pc-windows-gnu
```

Результат: `target/x86_64-pc-windows-gnu/release/notion-to-obsidian.exe`. Для версии 0.5.3 он упаковывается как `NotionToObsidian_v0.5.3.exe` автоматическим скриптом. Скрипт проверяет заголовок PE x64 и формирует `NotionToObsidian_Windows_x64.zip` и `SHA256SUMS.txt`. Приложению нужны только системные DLL Windows; сторонние среды выполнения не требуются.

## Командная строка

```sh
notion-to-obsidian ARCHIVE.zip --inspect
notion-to-obsidian ARCHIVE.zip NEW_OUTPUT_FOLDER
```

Папка результата должна быть новой. В Windows запуск EXE без параметров открывает GUI; один аргумент ZIP открывает GUI с выбранным архивом.

## Плагин и реальные Bases

Для DOM-проверки нужен Node.js и `jsdom@26` только в окружении разработчика:

```sh
NODE_PATH=/path/to/node_modules node tests/plugin.cjs
node tests/bases.cjs RESULT_FOLDER
```

Вторая команда проверяет состав всех записей и экспортированного вида для каждого `.base`, в том числе при дополнительных папках `Vault/` и `Копия/Vault/`. Пользовательский экспорт для неё в репозиторий не включён.

## GitHub Actions: проверка и публикация

Workflow: [`.github/workflows/windows-release.yml`](../.github/workflows/windows-release.yml).

- При `push` в `main` или ручном `Run workflow` проверяются Rust-тесты и ресурс иконки.
- На Ubuntu c Rust stable и MinGW-w64 собирается самодостаточный Windows x64 EXE (GNU static CRT). Скрипт `scripts/package_release.py` создаёт ZIP с EXE, лицензиями и инструкцией; формируются SHA-256.
- На настоящем Windows runner проверяются контрольная сумма ZIP, запуск конвертера в режиме CLI, создание Obsidian vault без вложенной `Vault`, отсутствие синтетических `Оглавление.md` и наличие содержания внутри статьи.
- **Только после успешных проверок** архив `NotionToObsidian_Windows_x64.zip` и `SHA256SUMS.txt` публикуются как GitHub Release по тегу `v<version>`. Постоянная ссылка — [latest/download](https://github.com/Chronaux/Notion-To-Obsidian/releases/latest/download/NotionToObsidian_Windows_x64.zip).
- Для каждой новой версии сначала измените номер версии в `Cargo.toml`, `Cargo.lock`, `app.rc`, `src/lib.rs`, `src/gui.rs` и `tests/ui.cjs`. Выпущенный тег не переписывается другой версией кода: CI потребует поднять номер.

GitHub Actions больше не нужно запускать локально для релиза, но файл EXE недоступен до завершения первого успешного workflow. Никаких сторонних сред установки пользователю не требуется.

## Проверка новой темы и значка (v0.5.3)

На Windows 10/11 после локальной сборки проверьте тёмные фоны окна, поля ZIP/результата, светлые подписи, контраст кнопок, работу клавиатуры и флажка, блокировку кнопок при преобразовании, прокрутку журнала, тёмный заголовок окна и значок в Проводнике/на панели задач. `app.rc` включает `assets/app.ico` (разрешения 16, 32, 48, 64, 128 и 256 px); скрипт сборки требует `x86_64-w64-mingw32-windres` для ресурсов. Проверьте `cargo test --locked`, `cargo build --release --locked --target x86_64-pc-windows-gnu`, затем EXE на настоящей Windows и контрольную сумму ZIP. GitHub Actions выполняет автоматизированные проверки и сборку; ручная визуальная проверка интерфейса Windows остаётся рекомендуемой.

В версии 0.5.3 проверьте, что статьи остаются в папке инструкции, а таблицы, `.base` и заметки строк находятся под `Базы данных/` с сохранением иерархии. Проверьте встроенные таблицы в статьях, кликабельность записей, фильтры Bases и отсутствие белых уголков у кнопок. Изменения размещения заметок не должны затрагивать файлы без баз данных. Для проверки актуальной версии смотрите статус workflow в GitHub Actions. Прежняя среда разработки не содержала Rust/MinGW-w64; CI предоставляет необходимые инструменты отдельно.
