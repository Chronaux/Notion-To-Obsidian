# Сборка и тестирование

Для запуска готового Windows-приложения инструменты разработчика не нужны. Этот раздел относится только к сборке исходников.

## Проверка ядра

Установите стабильный Rust и выполните из корня репозитория:

```sh
cargo test --locked
cargo build --release --locked
```

В Linux соберётся консольный конвертер. Нативное окно включается только для Windows.

## Windows x64

Текущий процесс сборки использует GNU-цель и MinGW-w64, включая `windres` для manifest/version. Эти инструменты должны быть в PATH. Сборка через MSVC отдельно не проверялась.

Для кросс-компиляции из Linux нужны Rust, GNU-цель Windows и MinGW-w64, включая `x86_64-w64-mingw32-gcc` и `x86_64-w64-mingw32-windres` в PATH:

```sh
rustup target add x86_64-pc-windows-gnu
cargo build --release --locked --target x86_64-pc-windows-gnu
```

Результат: `target/x86_64-pc-windows-gnu/release/notion-to-obsidian.exe`. В готовом ZIP он переименован в `NotionToObsidian_v0.4.0.exe`. Приложению нужны только системные DLL Windows; сторонние среды выполнения не требуются.

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

## Выпуск

Сборка и упаковка выполняются вручную; готовый архив размещается в `downloads/`. Workflow GitHub Actions не добавлены. В архив приложения входят EXE, инструкция, история изменений, уведомления о сторонних компонентах, файлы лицензий и SHA256SUMS.txt.

В новых исходниках после 0.4.0 папка результата сама является Obsidian vault: проверочный скрипт `tests/bases.cjs` ожидает Bases в корне результата, а отчёт в `.notion-to-obsidian/migration-report.json`. Префиксы `Vault/` и `Копия/Vault/` по-прежнему проверяются для совместимости фильтров при переносе заметок в подкаталоги. Скачиваемый архив v0.4.0 ещё использует старую структуру.
