//! Modal native Windows settings editor. JSON import/export is local only.
use super::*;
use notion_to_obsidian::options::{
    ConversionOptions, Profile, Sections, Unmatched, ParentPage, DatabaseLocation,
    TableOutput, Toc, Callouts, Tags,
};

#[link(name = "comdlg32")]
extern "system" {
    fn GetSaveFileNameW(file: *mut OpenFile) -> i32;
}
struct Editor {
    hwnd: H,
    parent: H,
    scale: f32,
    controls: Vec<H>,
    font: H,
    bg: H,
    field: H,
    options: ConversionOptions,
    checks: [bool; 4],
    accepted: bool,
}
impl Editor {
    unsafe fn add(&mut self, class: &str, label: &str, style: u32, id: isize, x: i32, y: i32, w: i32, h: i32) -> H {
        let v = |n: i32| (n as f32 * self.scale).round() as i32;
        let handle = CreateWindowExW(
            0, wide(class).as_ptr(), wide(label).as_ptr(),
            0x40000000 | 0x10000000 | style,
            v(x), v(y), v(w), v(h), self.hwnd, id, GetModuleHandleW(null()), null_mut(),
        );
        SendMessageW(handle, 0x0030, self.font as usize, 1);
        if class == "COMBOBOX" {
            SetWindowTheme(handle, wide("DarkMode_Explorer").as_ptr(), null());
            let previous = SetWindowLongPtrW(handle, -4, dark_combo_proc as isize);
            SetWindowLongPtrW(handle, -21, previous);
        } else if class == "EDIT" {
            SetWindowTheme(handle, wide("DarkMode_Explorer").as_ptr(), null());
        }
        self.controls.push(handle);
        handle
    }
    unsafe fn label(&mut self, text: &str, x: i32, y: i32) {
        self.add("STATIC", text, 0, 0, x, y, 330, 20);
    }
    unsafe fn combo(&mut self, id: isize, x: i32, y: i32, items: &[&str]) {
        let h = self.add("COMBOBOX", "", 0x00010000 | 0x00200000 | 0x3 | 0x10 | 0x200, id, x, y, 332, 225);
        for item in items {
            SendMessageW(h, 0x0143, 0, wide(item).as_ptr() as isize); // CB_ADDSTRING
        }
    }
    unsafe fn check(&mut self, id: isize, text: &str, x: i32, y: i32) {
        self.add("BUTTON", text, 0x00010000 | 0xB, id, x, y, 335, 27);
    }
    unsafe fn edit(&mut self, id: isize, x: i32, y: i32) {
        self.add("EDIT", "", 0x00010000 | 0x00000080 | 0x00800000, id, x, y, 330, 29);
    }
    unsafe fn button(&mut self, id: isize, text: &str, x: i32, y: i32, w: i32) {
        self.add("BUTTON", text, 0x00010000 | 0xB, id, x, y, w, 33);
    }
    unsafe fn create(&mut self) {
        self.font = CreateFontW(
            -(15.0 * self.scale) as i32, 0, 0, 0, 500, 0, 0, 0, 1, 0, 0, 5, 0,
            wide("Segoe UI").as_ptr(),
        );
        self.bg = CreateSolidBrush(BG);
        self.field = CreateSolidBrush(FIELD);
        self.label("Структура разделов", 20, 16);
        self.combo(600, 20, 39, &["Исходная иерархия Notion", "Группировать по корневому оглавлению"]);
        self.label("Страницы без раздела", 20, 80);
        self.combo(601, 20, 103, &["Оставить в исходной папке", "Переместить в отдельную папку"]);
        self.label("Название папки без раздела", 20, 144);
        self.edit(602, 20, 167);
        self.label("Родительская страница", 20, 208);
        self.combo(603, 20, 231, &["Внутри папки дочерних страниц", "Рядом с папкой"]);
        self.label("Расположение баз данных", 20, 272);
        self.combo(604, 20, 295, &["В исходных разделах", "В общей корневой папке"]);
        self.label("Название общей папки баз", 20, 336);
        self.edit(605, 20, 359);
        self.label("Формат таблиц", 20, 400);
        self.combo(606, 20, 423, &["Markdown + Bases", "Только Markdown", "Только Bases"]);

        self.label("Оглавления страниц", 385, 16);
        self.combo(607, 385, 39, &["Не добавлять", "Восстанавливать по заголовкам"]);
        self.label("Блоки и выноски", 385, 80);
        self.combo(608, 385, 103, &["Callout с сохранением эмодзи", "Callout без эмодзи", "Сохранять исходную разметку"]);
        self.label("Распознавание тегов", 385, 144);
        self.combo(609, 385, 167, &["Автоматически", "Указанные колонки JSON", "Не преобразовывать"]);
        self.check(610, "Встраивать таблицы в статьи", 385, 214);
        self.check(611, "Создавать заметку «Начать здесь»", 385, 246);
        self.check(612, "Добавлять CSS для полных таблиц", 385, 278);
        self.check(613, "Добавлять плагин полных таблиц", 385, 310);
        self.label("Индивидуальные правила баз задаются", 385, 360);
        self.label("через профиль JSON (по UUID базы).", 385, 382);
        self.button(614, "Загрузить JSON", 20, 506, 153);
        self.button(615, "Сохранить JSON", 187, 506, 165);
        self.button(616, "Отмена", 491, 506, 96);
        self.button(617, "Применить", 600, 506, 118);
        self.refresh();
    }
    unsafe fn child(&self, id: usize) -> H {
        // Settings controls have IDs 600..617, and were created in label/interspersed order.
        self.controls.iter().copied().find(|&h| GetWindowLongPtrW(h, -12) as usize == id).unwrap_or(0)
    }
    unsafe fn selected(&self, id: usize) -> usize {
        SendMessageW(self.child(id), 0x0147, 0, 0).max(0) as usize // CB_GETCURSEL
    }
    unsafe fn set_selected(&self, id: usize, selected: usize) {
        SendMessageW(self.child(id), 0x014E, selected, 0); // CB_SETCURSEL
    }
    unsafe fn refresh(&mut self) {
        let o = &self.options;
        self.set_selected(600, if o.structure.sections == Sections::Source {0} else {1});
        self.set_selected(601, if o.structure.unmatched == Unmatched::Source {0} else {1});
        set(self.child(602), o.structure.unmatched_folder.as_deref().unwrap_or("Другие сервисы"));
        self.set_selected(603, if o.structure.parent_page == ParentPage::InsideChildren {0} else {1});
        self.set_selected(604, if o.databases.location == DatabaseLocation::Source {0} else {1});
        set(self.child(605), o.databases.folder_name.as_deref().unwrap_or("Базы данных"));
        self.set_selected(606, match o.databases.output {
            TableOutput::MarkdownAndBases => 0, TableOutput::MarkdownOnly => 1, TableOutput::BasesOnly => 2
        });
        self.set_selected(607, if o.content.toc == Toc::Preserve {0} else {1});
        self.set_selected(608, match o.content.callouts {
            Callouts::ConvertKeepEmoji => 0, Callouts::ConvertStripEmoji => 1, Callouts::PreserveMarkdown => 2
        });
        self.set_selected(609, match o.content.tags {
            Tags::Auto => 0, Tags::ConfiguredColumns => 1, Tags::Off => 2
        });
        self.checks = [
            o.databases.embed_in_articles,
            o.extras.start_note,
            o.extras.table_css,
            o.extras.table_plugin,
        ];
    }
    unsafe fn collect(&self) -> Result<ConversionOptions, String> {
        let mut o = self.options.clone();
        o.structure.sections = if self.selected(600) == 0 { Sections::Source } else { Sections::RootOutline };
        o.structure.unmatched = if self.selected(601) == 0 { Unmatched::Source } else { Unmatched::NamedFolder };
        o.structure.unmatched_folder = if o.structure.unmatched == Unmatched::NamedFolder {
            Some(get(self.child(602))) } else { None };
        o.structure.parent_page = if self.selected(603) == 0 { ParentPage::InsideChildren } else { ParentPage::Sibling };
        o.databases.location = if self.selected(604) == 0 { DatabaseLocation::Source } else { DatabaseLocation::SeparateRoot };
        o.databases.folder_name = if o.databases.location == DatabaseLocation::SeparateRoot {
            Some(get(self.child(605))) } else { None };
        o.databases.output = match self.selected(606) {
            1 => TableOutput::MarkdownOnly, 2 => TableOutput::BasesOnly, _ => TableOutput::MarkdownAndBases
        };
        o.content.toc = if self.selected(607) == 0 { Toc::Preserve } else { Toc::RestoreIfMissing };
        o.content.callouts = match self.selected(608) {
            1 => Callouts::ConvertStripEmoji, 2 => Callouts::PreserveMarkdown, _ => Callouts::ConvertKeepEmoji
        };
        o.content.tags = match self.selected(609) {
            1 => Tags::ConfiguredColumns, 2 => Tags::Off, _ => Tags::Auto
        };
        o.databases.embed_in_articles = self.checks[0] && o.databases.output.markdown();
        o.extras.start_note = self.checks[1];
        o.extras.table_css = self.checks[2];
        o.extras.table_plugin = self.checks[3];
        if o.profile != Profile::Custom {
            let base = o.profile;
            o.profile = Profile::Custom;
            o.base_profile = Some(base);
        }
        o.validate()?;
        Ok(o)
    }
    unsafe fn json_path(&self, save: bool) -> Option<PathBuf> {
        let mut name = vec![0u16; 32768];
        if save {
            let default = wide("export-profile.json");
            name[..default.len()].copy_from_slice(&default);
        }
        let filters = wide("JSON (*.json)\0*.json\0Все файлы\0*.*\0");
        let mut file: OpenFile = std::mem::zeroed();
        file.size = std::mem::size_of::<OpenFile>() as u32;
        file.owner = self.hwnd;
        file.filter = filters.as_ptr();
        file.file = name.as_mut_ptr();
        file.max_file = name.len() as u32;
        file.flags = 0x00080000 | 0x00001000 | if save { 0x2 } else { 0x800 };
        file.filter_index = 1;
        let ok = if save { GetSaveFileNameW(&mut file) } else { GetOpenFileNameW(&mut file) };
        if ok == 0 { return None; }
        let length = name.iter().position(|&c| c == 0).unwrap_or(0);
        Some(PathBuf::from(String::from_utf16_lossy(&name[..length])))
    }
}

unsafe fn draw_settings_checkbox(item: &DrawItem, state: &Editor) {
    let id = item.control_id as usize;
    let index = id.saturating_sub(610).min(3);
    let checked = state.checks[index];
    let disabled = item.item_state & 0x0004 != 0; // ODS_DISABLED
    let pressed = item.item_state & 0x0001 != 0; // ODS_SELECTED
    let focused = item.item_state & 0x0010 != 0; // ODS_FOCUS
    let hovered = item.item_state & 0x0040 != 0; // ODS_HOTLIGHT
    let p = |n: i32| (n as f32 * state.scale).round() as i32;

    let row_fill = if disabled { BG } else if pressed {
        rgb(48, 43, 62)
    } else if hovered {
        rgb(34, 32, 41)
    } else {
        BG
    };
    rounded(item.dc, &item.rect, row_fill,
        if focused { ACCENT } else { row_fill }, p(4));

    let side = p(16);
    let left = item.rect.left + p(2);
    let center = (item.rect.top + item.rect.bottom) / 2;
    let glyph = Rect {
        left, top: center - side / 2,
        right: left + side, bottom: center + side / 2,
    };
    rounded(item.dc, &glyph,
        if checked && !disabled { ACCENT } else { FIELD },
        if checked && !disabled { ACCENT } else if disabled { BORDER } else { MUTED },
        p(4));
    if checked && !disabled {
        let mark = wide("✓");
        let mut mark_rect = glyph;
        SetBkMode(item.dc, 1);
        SetTextColor(item.dc, TEXT);
        let old = SelectObject(item.dc, state.font);
        DrawTextW(item.dc, mark.as_ptr(), (mark.len() - 1) as i32,
            &mut mark_rect, 0x1 | 0x4 | 0x20);
        SelectObject(item.dc, old);
    }

    let label = wide(&get(item.hwnd));
    let mut text_rect = item.rect;
    text_rect.left += p(27);
    text_rect.left = text_rect.left.min(text_rect.right);
    SetBkMode(item.dc, 1);
    SetTextColor(item.dc, if disabled { MUTED } else { TEXT });
    let old = SelectObject(item.dc, state.font);
    DrawTextW(item.dc, label.as_ptr(), (label.len() - 1) as i32,
        &mut text_rect, 0x20 | 0x4 | 0x800 | 0x8000);
    SelectObject(item.dc, old);
}

unsafe fn draw_settings_button(item: &DrawItem, state: &Editor) {
    let id = item.control_id;
    let disabled = item.item_state & 0x0004 != 0;
    let down = item.item_state & 0x0001 != 0;
    let focused = item.item_state & 0x0010 != 0;
    let hovered = item.item_state & 0x0040 != 0;
    let primary = id == 617;
    let fill = if disabled {
        FIELD
    } else if primary {
        if down { rgb(94, 72, 192) } else if hovered { rgb(137, 111, 245) } else { ACCENT }
    } else if down {
        rgb(54, 50, 61)
    } else if hovered {
        rgb(48, 44, 58)
    } else {
        FIELD
    };
    let outline = if focused { ACCENT } else if primary { ACCENT } else if hovered { rgb(88, 77, 132) } else { BORDER };
    rounded(item.dc, &item.rect, fill, outline, (8.0 * state.scale).round() as i32);
    SetBkMode(item.dc, 1);
    SetTextColor(item.dc, if disabled { MUTED } else { TEXT });
    let old_font = SelectObject(item.dc, state.font);
    let label = wide(&get(item.hwnd));
    let mut text_rect = item.rect;
    text_rect.left += (7.0 * state.scale).round() as i32;
    text_rect.right -= (7.0 * state.scale).round() as i32;
    DrawTextW(item.dc, label.as_ptr(), (label.len() - 1) as i32,
        &mut text_rect, 0x1 | 0x4 | 0x20 | 0x800 | 0x8000);
    SelectObject(item.dc, old_font);
}

unsafe extern "system" fn settings_proc(hwnd: H, msg: u32, w: usize, l: isize) -> isize {
    if msg == 1 {
        let state = *(l as *const *mut Editor);
        SetWindowLongPtrW(hwnd, -21, state as isize);
        (*state).hwnd = hwnd;
        (*state).create();
        return 0;
    }
    let state = GetWindowLongPtrW(hwnd, -21) as *mut Editor;
    if state.is_null() { return DefWindowProcW(hwnd, msg, w, l); }
    let state = &mut *state;
    match msg {
        0x0014 => 1, // avoid default light background erase
        0x000F => {
            let mut paint: Paint = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut paint);
            let mut bounds = Rect { left: 0, top: 0, right: 0, bottom: 0 };
            GetClientRect(hwnd, &mut bounds);
            FillRect(dc, &bounds, state.bg);
            EndPaint(hwnd, &paint);
            0
        }
        0x0133 | 0x0134 | 0x0138 => {
            let edit = w != 0 && state.controls.iter().any(|&h| h == l as H && (h == state.child(602) || h == state.child(605)));
            let dc = w as H;
            SetBkMode(dc, 1);
            SetBkColor(dc, if edit {FIELD} else {BG});
            SetTextColor(dc, TEXT);
            if edit {state.field} else {state.bg}
        }
        0x002B => { // WM_DRAWITEM
            if l != 0 {
                let item = &*(l as *const DrawItem);
                if item.control_type == 4 && matches!(item.control_id, 614..=617) {
                    draw_settings_button(item, state);
                    return 1;
                }
                if item.control_type == 4 && matches!(item.control_id, 610..=613) {
                    draw_settings_checkbox(item, state);
                    return 1;
                }
                // ODT_COMBOBOX is 3; ODT_LISTBOX (2) is not sent for CBS_OWNERDRAWFIXED.
                if item.control_type == 3 && matches!(item.control_id, 600..=609) {
                    draw_combo_item(item, state.font, state.scale);
                    return 1;
                }
            }
            DefWindowProcW(hwnd, msg, w, l)
        }
        0x002C => { // WM_MEASUREITEM
            if l != 0 {
                let measure = &mut *(l as *mut MeasureItem);
                if measure.control_type == 3 && matches!(measure.control_id, 600..=609) {
                    measure.item_height = (28.0 * state.scale).round() as u32;
                    return 1;
                }
            }
            DefWindowProcW(hwnd, msg, w, l)
        }
        0x0111 => {
            match w & 0xffff {
                610..=613 if (w >> 16) & 0xffff == 0 => {
                    let index = (w & 0xffff) as usize - 610;
                    state.checks[index] = !state.checks[index];
                    InvalidateRect(state.child(index + 610), null(), 0);
                },
                614 => if let Some(path) = state.json_path(false) {
                    match ConversionOptions::load(&path) {
                        Ok(options) => {state.options = options; state.refresh();},
                        Err(err) => dialog(hwnd, &err),
                    }
                },
                615 => match state.collect() {
                    Ok(options) => if let Some(path) = state.json_path(true) {
                        if let Err(err) = options.save(&path) { dialog(hwnd, &err); }
                    },
                    Err(err) => dialog(hwnd, &err),
                },
                616 => { DestroyWindow(hwnd); },
                617 => match state.collect() {
                    Ok(options) => {
                        state.options = options;
                        state.accepted = true;
                        DestroyWindow(hwnd);
                    }
                    Err(err) => dialog(hwnd, &err),
                },
                _ => {}
            }
            0
        }
        0x0010 => { DestroyWindow(hwnd); 0 }
        0x0002 => {
            DeleteObject(state.font);
            DeleteObject(state.bg);
            DeleteObject(state.field);
            PostQuitMessage(0); // only the nested modal message loop consumes this
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
pub unsafe fn show(parent: H, options: &ConversionOptions, scale: f32) -> Option<ConversionOptions> {
    let instance = GetModuleHandleW(null());
    let name = wide("NotionExportProfileEditor");
    let cls = Class {
        size: std::mem::size_of::<Class>() as u32,
        style: 3, proc: Some(settings_proc), cls: 0, win: 0, instance,
        icon: LoadIconW(instance, 1usize as *const u16),
        cursor: LoadCursorW(0, 32512usize as *const u16),
        brush: 0, menu: null(), name: name.as_ptr(),
        small_icon: LoadIconW(instance, 1usize as *const u16),
    };
    RegisterClassExW(&cls);
    let mut state = Box::new(Editor {
        hwnd: 0, parent, scale, controls: vec![], font: 0, bg: 0, field: 0,
        options: options.clone(), checks: [false; 4], accepted: false,
    });
    let ptr: *mut Editor = &mut *state;
    let width = (756.0 * scale) as i32;
    let height = (590.0 * scale) as i32;
    let mut p = Rect { left: 0, top: 0, right: 0, bottom: 0 };
    GetWindowRect(parent, &mut p);
    let x = ((p.left + p.right - width) / 2).max(0);
    let y = ((p.top + p.bottom - height) / 2).max(0);
    let hwnd = CreateWindowExW(0, name.as_ptr(), wide("Настройки экспорта").as_ptr(),
        0x00ca0000 | 0x02000000, x, y, width, height,
        parent, 0, instance, ptr as *mut c_void);
    if hwnd == 0 { dialog(parent, "Не удалось открыть настройки."); return None; }
    let dark = 1i32;
    if DwmSetWindowAttribute(hwnd, 20, &dark, 4) != 0 { DwmSetWindowAttribute(hwnd, 19, &dark, 4); }
    EnableWindow(parent, 0);
    ShowWindow(hwnd, 1);
    UpdateWindow(hwnd);
    let mut msg: Msg = std::mem::zeroed();
    while GetMessageW(&mut msg, 0, 0, 0) > 0 {
        if IsDialogMessageW(hwnd, &mut msg) == 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    EnableWindow(parent, 1);
    SetFocus(parent);
    if state.accepted { Some(state.options) } else { None }
}
