#![allow(non_snake_case)]
use notion_to_obsidian::{Converter, Report};
use std::{
    ffi::c_void,
    path::{Path, PathBuf},
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    thread,
};
type H = isize;
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
// DRAWITEMSTRUCT on Win64. Used by BS_OWNERDRAW for native dark buttons.
#[repr(C)]
struct DrawItem {
    control_type: u32,
    control_id: u32,
    item_id: u32,
    item_action: u32,
    item_state: u32,
    hwnd: H,
    dc: H,
    rect: Rect,
    item_data: usize,
}
const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}
const BG: u32 = rgb(17, 19, 27);
const HEADER: u32 = rgb(24, 27, 39);
const FIELD: u32 = rgb(32, 36, 50);
const LOG_BG: u32 = rgb(20, 23, 33);
const BORDER: u32 = rgb(60, 66, 88);
const TEXT: u32 = rgb(239, 241, 249);
const MUTED: u32 = rgb(164, 171, 192);
const ACCENT: u32 = rgb(123, 103, 244);
#[repr(C)]
struct Msg {
    hwnd: H,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt: Point,
    private: u32,
}
#[repr(C)]
struct Class {
    size: u32,
    style: u32,
    proc: Option<unsafe extern "system" fn(H, u32, usize, isize) -> isize>,
    cls: i32,
    win: i32,
    instance: H,
    icon: H,
    cursor: H,
    brush: H,
    menu: *const u16,
    name: *const u16,
    small_icon: H,
}
#[repr(C)]
struct OpenFile {
    size: u32,
    owner: H,
    instance: H,
    filter: *const u16,
    custom: *mut u16,
    max_custom: u32,
    filter_index: u32,
    file: *mut u16,
    max_file: u32,
    file_title: *mut u16,
    max_title: u32,
    initial_dir: *const u16,
    title: *const u16,
    flags: u32,
    offset: u16,
    extension: u16,
    def_ext: *const u16,
    cust: isize,
    hook: usize,
    template: *const u16,
    reserved: *mut c_void,
    reserved2: u32,
    flags_ex: u32,
}
#[repr(C)]
struct Browse {
    owner: H,
    root: *const c_void,
    display: *mut u16,
    title: *const u16,
    flags: u32,
    callback: usize,
    param: isize,
    image: i32,
}
#[link(name = "user32")]
extern "system" {
    fn RegisterClassExW(c: *const Class) -> u16;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        text: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: H,
        menu: H,
        instance: H,
        param: *mut c_void,
    ) -> H;
    fn DefWindowProcW(hwnd: H, msg: u32, w: usize, l: isize) -> isize;
    fn GetMessageW(msg: *mut Msg, hwnd: H, min: u32, max: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> isize;
    fn ShowWindow(hwnd: H, cmd: i32) -> i32;
    fn UpdateWindow(hwnd: H) -> i32;
    fn PostQuitMessage(code: i32);
    fn GetWindowLongPtrW(hwnd: H, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: H, index: i32, value: isize) -> isize;
    fn SendMessageW(hwnd: H, msg: u32, w: usize, l: isize) -> isize;
    fn SetWindowTextW(hwnd: H, text: *const u16) -> i32;
    fn GetWindowTextLengthW(hwnd: H) -> i32;
    fn GetWindowTextW(hwnd: H, text: *mut u16, max: i32) -> i32;
    fn EnableWindow(hwnd: H, enable: i32) -> i32;
    fn MoveWindow(hwnd: H, x: i32, y: i32, w: i32, h: i32, repaint: i32) -> i32;
    fn GetClientRect(hwnd: H, rect: *mut Rect) -> i32;
    fn InvalidateRect(hwnd: H, rect: *const Rect, erase: i32) -> i32;
    fn FillRect(dc: H, rect: *const Rect, brush: H) -> i32;
    fn DrawTextW(dc: H, text: *const u16, len: i32, rect: *mut Rect, format: u32) -> i32;
    fn MessageBoxW(hwnd: H, text: *const u16, title: *const u16, flags: u32) -> i32;
    fn SetTimer(hwnd: H, id: usize, ms: u32, callback: usize) -> usize;
    fn KillTimer(hwnd: H, id: usize) -> i32;
    fn LoadCursorW(instance: H, name: *const u16) -> H;
    fn LoadIconW(instance: H, name: *const u16) -> H;
    fn SetProcessDPIAware() -> i32;
    fn GetDpiForSystem() -> u32;
    fn IsDialogMessageW(hwnd: H, msg: *mut Msg) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> H;
}
#[link(name = "gdi32")]
extern "system" {
    fn CreateFontW(
        height: i32,
        width: i32,
        escape: i32,
        orientation: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strike: u32,
        charset: u32,
        out: u32,
        clip: u32,
        quality: u32,
        pitch: u32,
        face: *const u16,
    ) -> H;
    fn DeleteObject(object: H) -> i32;
    fn CreateSolidBrush(color: u32) -> H;
    fn CreatePen(style: i32, width: i32, color: u32) -> H;
    fn SelectObject(dc: H, object: H) -> H;
    fn SetTextColor(dc: H, color: u32) -> u32;
    fn SetBkColor(dc: H, color: u32) -> u32;
    fn SetBkMode(dc: H, mode: i32) -> i32;
    fn RoundRect(dc: H, left: i32, top: i32, right: i32, bottom: i32, width: i32, height: i32) -> i32;
}
#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(hwnd: H, attribute: u32, value: *const i32, size: u32) -> i32;
}
#[link(name = "uxtheme")]
extern "system" {
    fn SetWindowTheme(hwnd: H, theme: *const u16, sub_id: *const u16) -> i32;
}
#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(file: *mut OpenFile) -> i32;
}
#[link(name = "shell32")]
extern "system" {
    fn SHBrowseForFolderW(info: *mut Browse) -> *mut c_void;
    fn SHGetPathFromIDListW(id: *const c_void, path: *mut u16) -> i32;
    fn ShellExecuteW(
        hwnd: H,
        op: *const u16,
        file: *const u16,
        params: *const u16,
        dir: *const u16,
        show: i32,
    ) -> H;
    fn DragAcceptFiles(hwnd: H, accept: i32);
    fn DragQueryFileW(drop: H, index: u32, file: *mut u16, size: u32) -> u32;
    fn DragFinish(drop: H);
}
#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(ptr: *mut c_void, flags: u32) -> i32;
    fn CoTaskMemFree(ptr: *mut c_void);
    fn CoUninitialize();
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
unsafe fn set(h: H, s: &str) {
    SetWindowTextW(h, wide(s).as_ptr());
}
unsafe fn get(h: H) -> String {
    let len = GetWindowTextLengthW(h);
    let mut s = vec![0; len as usize + 1];
    GetWindowTextW(h, s.as_mut_ptr(), len + 1);
    String::from_utf16_lossy(&s[..len as usize])
}
unsafe fn dialog(h: H, s: &str) {
    MessageBoxW(
        h,
        wide(s).as_ptr(),
        wide("Notion → Obsidian").as_ptr(),
        0x30,
    );
}
// GDI-only dark UI. No WebView, external theme DLLs, or runtime dependencies.
unsafe fn rounded(dc: H, rect: &Rect, fill: u32, outline: u32, radius: i32) {
    let brush = CreateSolidBrush(fill);
    let pen = CreatePen(0, 1, outline);
    let old_brush = SelectObject(dc, brush);
    let old_pen = SelectObject(dc, pen);
    RoundRect(dc, rect.left, rect.top, rect.right, rect.bottom, radius, radius);
    SelectObject(dc, old_pen);
    SelectObject(dc, old_brush);
    DeleteObject(pen);
    DeleteObject(brush);
}

unsafe fn background(hwnd: H, dc: H, app: &App) {
    let mut client = Rect { left: 0, top: 0, right: 0, bottom: 0 };
    GetClientRect(hwnd, &mut client);
    FillRect(dc, &client, app.bg_brush);
    let scale = app.scale;
    let v = |n: i32| (n as f32 * scale) as i32;
    let header = Rect { left: 0, top: 0, right: client.right, bottom: v(111) };
    FillRect(dc, &header, app.header_brush);
    let accent = Rect { left: v(28), top: v(23), right: v(33), bottom: v(67) };
    let accent_brush = CreateSolidBrush(ACCENT);
    FillRect(dc, &accent, accent_brush);
    DeleteObject(accent_brush);

    let m = v(28);
    let gap = v(12);
    let btn = v(170);
    let editw = (client.right - 2 * m - gap - btn).max(v(120));
    for y in [155, 249] {
        rounded(
            dc,
            &Rect { left: m, top: v(y), right: m + editw, bottom: v(y + 46) },
            FIELD, BORDER, v(12),
        );
    }
    rounded(
        dc,
        &Rect {
            left: m,
            top: v(496),
            right: client.right - m,
            bottom: client.bottom - v(75),
        },
        LOG_BG, BORDER, v(14),
    );
}

unsafe fn draw_button(item: &DrawItem, app: &App) {
    let disabled = item.item_state & 0x4 != 0;
    let down = item.item_state & 0x1 != 0;
    let focused = item.item_state & 0x10 != 0;
    let check = item.control_id == 110;
    let primary = item.control_id == 106;
    let can_open = item.control_id == 109;
    let (background, outline, foreground) = if disabled {
        (rgb(33, 36, 47), rgb(47, 51, 66), rgb(102, 109, 130))
    } else if primary {
        (if down { rgb(96, 77, 205) } else { ACCENT }, ACCENT, TEXT)
    } else if can_open {
        (if down { rgb(54, 48, 94) } else { rgb(40, 37, 68) },
         rgb(103, 87, 174), TEXT)
    } else if check {
        (rgb(27, 30, 43), if focused { ACCENT } else { BORDER }, TEXT)
    } else {
        (if down { rgb(48, 53, 72) } else { FIELD },
         if focused { ACCENT } else { BORDER }, TEXT)
    };
    let radius = ((if check { 12.0 } else { 14.0 }) * app.scale) as i32;
    rounded(item.dc, &item.rect, background, outline, radius);
    SetBkMode(item.dc, 1); // TRANSPARENT
    SetTextColor(item.dc, foreground);
    let old_font = SelectObject(item.dc, app.font);
    let label = get(item.hwnd);
    let wide_label = wide(&label);
    let mut text_rect = item.rect;
    let format = 0x20 | 0x4 | 0x8000; // SINGLELINE, VCENTER, END_ELLIPSIS
    if check {
        let offset = (14.0 * app.scale) as i32;
        let side = (20.0 * app.scale) as i32;
        let middle = (item.rect.top + item.rect.bottom) / 2;
        let box_rect = Rect {
            left: item.rect.left + offset,
            top: middle - side / 2,
            right: item.rect.left + offset + side,
            bottom: middle + side / 2,
        };
        rounded(
            item.dc, &box_rect,
            if app.skip_missing { ACCENT } else { FIELD },
            if app.skip_missing { ACCENT } else { BORDER },
            (6.0 * app.scale) as i32
        );
        if app.skip_missing {
            let tick = wide("✓");
            let mut r = box_rect;
            DrawTextW(item.dc, tick.as_ptr(), (tick.len() - 1) as i32, &mut r,
                      0x1 | 0x4 | 0x20);
        }
        text_rect.left += (47.0 * app.scale) as i32;
        text_rect.right -= (9.0 * app.scale) as i32;
        DrawTextW(item.dc, wide_label.as_ptr(), (wide_label.len() - 1) as i32,
                  &mut text_rect, format);
    } else {
        DrawTextW(item.dc, wide_label.as_ptr(), (wide_label.len() - 1) as i32,
                  &mut text_rect, format | 0x1); // centered
    }
    SelectObject(item.dc, old_font);
}

enum Event {
    Message(String),
    Done(std::result::Result<Report, String>, bool),
}
struct App {
    hwnd: H,
    controls: Vec<H>,
    font: H,
    titlefont: H,
    bg_brush: H,
    header_brush: H,
    field_brush: H,
    log_brush: H,
    skip_missing: bool,
    rx: Option<Receiver<Event>>,
    stop: Arc<AtomicBool>,
    busy: bool,
    last_output: PathBuf,
    scale: f32,
}
impl App {
    unsafe fn create(&mut self) {
        self.bg_brush = CreateSolidBrush(BG);
        self.header_brush = CreateSolidBrush(HEADER);
        self.field_brush = CreateSolidBrush(FIELD);
        self.log_brush = CreateSolidBrush(LOG_BG);
        self.font = CreateFontW(
            -(16.0 * self.scale) as i32,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            1,
            0,
            0,
            5,
            0,
            wide("Segoe UI").as_ptr(),
        );
        self.titlefont = CreateFontW(
            -(30.0 * self.scale) as i32,
            0,
            0,
            0,
            600,
            0,
            0,
            0,
            1,
            0,
            0,
            5,
            0,
            wide("Segoe UI").as_ptr(),
        );
        let defs = [
            ("STATIC", "Notion → Obsidian", 0u32, 0),
            (
                "STATIC",
                "Перенос экспорта Notion в готовое хранилище Obsidian · локально и безопасно",
                0,
                0,
            ),
            ("STATIC", "Архив Notion · Markdown & CSV", 0, 0),
            ("EDIT", "", 0x80 | 0x10000, 101),
            ("BUTTON", "Выбрать ZIP…", 0x10000, 102),
            ("STATIC", "Новая папка результата", 0, 0),
            ("EDIT", "", 0x80 | 0x10000, 103),
            ("BUTTON", "Выбрать папку…", 0x10000, 104),
            (
                "STATIC",
                "Папка результата сразу откроется в Obsidian как хранилище. Для таблиц включите Bases.",
                0,
                0,
            ),
            ("BUTTON", "Проверить архив", 0x10000, 105),
            ("BUTTON", "Преобразовать", 0x10000 | 1, 106),
            ("BUTTON", "Отменить", 0x10000, 107),
            ("STATIC", "Выберите ZIP или перетащите его в окно.", 0, 0),
            ("EDIT", "", 0x4 | 0x800 | 0x40 | 0x1000 | 0x200000, 108),
            ("BUTTON", "Открыть результат", 0x10000, 109),
            (
                "BUTTON",
                "Не создавать заглушки отсутствующих страниц Notion (проверено вручную)",
                0x10000 | 0xB, // BS_OWNERDRAW; state is held by App
                110,
            ),
        ];
        for (class, text, style, id) in defs {
            let ex = 0u32; // No light 3D borders: fields are framed by the parent.
            let style = if class == "BUTTON" { (style & !0x0f) | 0xB } else { style };
            let h = CreateWindowExW(
                ex,
                wide(class).as_ptr(),
                wide(text).as_ptr(),
                0x40000000 | 0x10000000 | style,
                0,
                0,
                0,
                0,
                self.hwnd,
                id,
                GetModuleHandleW(null()),
                null_mut(),
            );
            SendMessageW(
                h,
                0x30,
                if self.controls.is_empty() {
                    self.titlefont as usize
                } else {
                    self.font as usize
                },
                1,
            );
            self.controls.push(h);
        }
        let default = std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or(PathBuf::from("."))
            .join("Documents")
            .join("Notion_Obsidian_v0.5.0");
        set(self.controls[6], &fresh_path(default).to_string_lossy());
        EnableWindow(self.controls[11], 0);
        EnableWindow(self.controls[14], 0);
        // Slight insets provide breathing room without adding custom controls.
        for i in [3usize, 6] {
            SendMessageW(self.controls[i], 0x00D3, 3, (10 << 16) | 10); // EM_SETMARGINS
        }
        SendMessageW(self.controls[13], 0x00D3, 3, (12 << 16) | 12);
        for i in [3usize, 6, 13] {
            SetWindowTheme(self.controls[i], wide("DarkMode_Explorer").as_ptr(), null());
        }
        DragAcceptFiles(self.hwnd, 1);
        SetTimer(self.hwnd, 1, 100, 0);
        self.layout();
    }
    unsafe fn layout(&self) {
        let mut r = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        GetClientRect(self.hwnd, &mut r);
        let w = r.right;
        let h = r.bottom;
        let s = self.scale;
        let v = |n: i32| (n as f32 * s) as i32;
        let m = v(28);
        let gap = v(12);
        let btn = v(170);
        let editw = (w - 2 * m - gap - btn).max(v(120));
        let rows = [
            (m + v(17), v(24), w - 2 * m - v(17), v(44)), // Brand title
            (m, v(77), w - 2 * m, v(24)),          // Subtitle
            (m, v(127), w - 2 * m, v(24)),         // ZIP label
            (m + v(12), v(164), editw - v(24), v(28)),
            (m + editw + gap, v(155), btn, v(46)),
            (m, v(221), w - 2 * m, v(24)),         // Output label
            (m + v(12), v(258), editw - v(24), v(28)),
            (m + editw + gap, v(249), btn, v(46)),
            (m, v(312), w - 2 * m, v(24)),         // Hint
            (m, v(401), v(178), v(44)),
            (m + v(190), v(401), v(178), v(44)),
            (m + v(380), v(401), v(132), v(44)),
            (m, v(467), w - 2 * m, v(24)),         // Status
            (m + v(10), v(506), w - 2 * m - v(20), (h - v(592)).max(v(68))),
            (m, h - v(62), w - 2 * m, v(42)),      // Open result
            (m, v(351), w - 2 * m, v(36)),        // Manual skip
        ];
        for (i, (x, y, width, height)) in rows.iter().enumerate() {
            MoveWindow(self.controls[i], *x, *y, *width, *height, 1);
        }
        InvalidateRect(self.hwnd, null(), 1);
    }
    unsafe fn log(&self, s: &str) {
        let h = self.controls[13];
        let line = s.replace('\n', "\r\n") + "\r\n";
        SendMessageW(h, 0xb1, usize::MAX, -1);
        SendMessageW(h, 0xc2, 0, wide(&line).as_ptr() as isize);
        SendMessageW(h, 0xb7, 0, 0);
    }
    unsafe fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
        for i in [3, 4, 6, 7, 9, 10, 15] {
            EnableWindow(self.controls[i], (!busy) as i32);
        }
        EnableWindow(self.controls[11], busy as i32);
    }
    unsafe fn start(&mut self, inspect: bool) {
        if self.busy {
            return;
        }
        let archive = PathBuf::from(get(self.controls[3]).trim());
        let output = PathBuf::from(get(self.controls[6]).trim());
        let skip_missing = self.skip_missing;
        if !archive.is_file() {
            dialog(self.hwnd, "Выберите существующий ZIP-файл.");
            return;
        }
        if !inspect && (get(self.controls[6]).trim().is_empty() || output.exists()) {
            dialog(
                self.hwnd,
                "Укажите новую папку, которой ещё нет. Существующие файлы не перезаписываются.",
            );
            return;
        }
        set(self.controls[13], "");
        set(
            self.controls[12],
            if inspect {
                "Проверка архива…"
            } else {
                "Преобразование…"
            },
        );
        EnableWindow(self.controls[14], 0);
        self.set_busy(true);
        self.last_output = output.clone();
        self.stop = Arc::new(AtomicBool::new(false));
        let stop = self.stop.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        thread::spawn(move || {
            let progress = tx.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let c = Converter::new(
                    move |s| {
                        let _ = progress.send(Event::Message(s.into()));
                    },
                    stop,
                ).skip_missing_pages(skip_missing);
                if inspect {
                    c.inspect(&archive)
                } else {
                    c.run(&archive, &output)
                }
            }))
            .unwrap_or_else(|_| Err("Внутренняя ошибка. Итоговая папка не опубликована.".into()));
            let _ = tx.send(Event::Done(result, inspect));
        });
    }
    unsafe fn poll(&mut self) {
        let events = self
            .rx
            .as_ref()
            .map(|r| r.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Message(s) => {
                    set(self.controls[12], &s);
                    self.log(&s)
                }
                Event::Done(result, inspect) => {
                    self.set_busy(false);
                    self.rx = None;
                    match result {
                        Ok(r) => {
                            set(
                                self.controls[12],
                                if inspect {
                                    "Проверка завершена"
                                } else {
                                    "Готово — хранилище создано"
                                },
                            );
                            self.log(&format!(
                                "\nСтраниц: {} · Баз: {} · Записей: {}",
                                r.stats.get("source_pages").unwrap_or(&0),
                                r.stats.get("source_databases").unwrap_or(&0),
                                r.stats.get("source_rows").unwrap_or(&0)
                            ));
                            let matched = r
                                .row_matching
                                .iter()
                                .filter_map(|x| x["matched"].as_u64())
                                .sum::<u64>();
                            self.log(&format!("Сопоставлено записей с Markdown: {matched}"));
                            for d in &r.databases {
                                self.log(&format!(
                                    "  {}: {} записей, в текущем виде {}",
                                    d["title"].as_str().unwrap_or(""),
                                    d["rows"],
                                    d["view_rows"]
                                ));
                            }
                            if !inspect {
                                self.log(&format!(
                                    "\nВложений: {} · Заглушек: {} · Пропущено страниц: {}",
                                    r.stats.get("attachments").unwrap_or(&0),
                                    r.stats.get("missing_pages").unwrap_or(&0),
                                    r.stats.get("skipped_pages").unwrap_or(&0)
                                ));
                                self.log("Откройте папку результата в Obsidian. Подробности — в .notion-to-obsidian/migration-report.json.");
                                EnableWindow(self.controls[14], 1);
                            }
                            self.log(&format!("Предупреждений: {}", r.warnings.len()));
                            for w in r.warnings.iter().take(10) {
                                self.log(&format!("  {}: {}", w.kind, w.detail));
                            }
                        }
                        Err(e) => {
                            set(
                                self.controls[12],
                                "Операция завершена с ошибкой или отменена",
                            );
                            self.log(&e);
                        }
                    }
                }
            }
        }
    }
    unsafe fn choose_zip(&self) {
        let mut file = vec![0u16; 32768];
        let filter = wide("Notion ZIP (*.zip)\0*.zip\0Все файлы\0*.*\0");
        let title = wide("Выберите экспорт Notion (Markdown & CSV)");
        let mut of: OpenFile = std::mem::zeroed();
        of.size = std::mem::size_of::<OpenFile>() as u32;
        of.owner = self.hwnd;
        of.file = file.as_mut_ptr();
        of.max_file = file.len() as u32;
        of.filter = filter.as_ptr();
        of.title = title.as_ptr();
        of.flags = 0x1000 | 0x800 | 0x80000;
        of.filter_index = 1;
        if GetOpenFileNameW(&mut of) != 0 {
            let end = file.iter().position(|&x| x == 0).unwrap();
            set(self.controls[3], &String::from_utf16_lossy(&file[..end]));
        }
    }
    unsafe fn choose_output(&self) {
        let mut display = vec![0u16; 260];
        let title = wide("Выберите папку, внутри которой будет создан новый результат");
        let mut b = Browse {
            owner: self.hwnd,
            root: null(),
            display: display.as_mut_ptr(),
            title: title.as_ptr(),
            flags: 0x40 | 1,
            callback: 0,
            param: 0,
            image: 0,
        };
        let pidl = SHBrowseForFolderW(&mut b);
        if !pidl.is_null() {
            let mut buf = vec![0u16; 32768];
            if SHGetPathFromIDListW(pidl, buf.as_mut_ptr()) != 0 {
                let end = buf.iter().position(|&x| x == 0).unwrap();
                let path = fresh_path(
                    PathBuf::from(String::from_utf16_lossy(&buf[..end]))
                        .join("Notion_Obsidian_v0.5.0"),
                );
                set(self.controls[6], &path.to_string_lossy());
            }
            CoTaskMemFree(pidl);
        }
    }
}
fn fresh_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let parent = path.parent().unwrap_or(Path::new("."));
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    for i in 2.. {
        let next = parent.join(format!("{name}_{i}"));
        if !next.exists() {
            return next;
        }
    }
    unreachable!()
}
unsafe extern "system" fn procedure(hwnd: H, msg: u32, w: usize, l: isize) -> isize {
    if msg == 1 {
        let ptr = *(l as *const *mut App);
        SetWindowLongPtrW(hwnd, -21, ptr as isize);
        (*ptr).hwnd = hwnd;
        (*ptr).create();
        return 0;
    }
    let ptr = GetWindowLongPtrW(hwnd, -21) as *mut App;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let app = &mut *ptr;
    match msg {
        0x14 => { // WM_ERASEBKGND
            if w != 0 {
                background(hwnd, w as H, app);
                1
            } else {
                0
            }
        }
        0x133 | 0x138 => { // WM_CTLCOLOREDIT / WM_CTLCOLORSTATIC
            let dc = w as H;
            let child = l as H;
            let index = app.controls.iter().position(|&h| h == child);
            let is_log = index == Some(13);
            let is_field = index == Some(3) || index == Some(6);
            let is_header = index == Some(0) || index == Some(1);
            let background_color =
                if is_log { LOG_BG } else if is_field { FIELD } else if is_header { HEADER } else { BG };
            SetBkColor(dc, background_color);
            SetBkMode(dc, if is_field || is_log { 2 } else { 1 });
            SetTextColor(dc, if is_log || is_field || index == Some(0)
                || index == Some(2) || index == Some(5) {
                TEXT
            } else if index == Some(12) {
                ACCENT
            } else {
                MUTED
            });
            if is_log {
                app.log_brush
            } else if is_field {
                app.field_brush
            } else if is_header {
                app.header_brush
            } else {
                app.bg_brush
            }
        }
        0x2b => { // WM_DRAWITEM
            if l != 0 {
                let item = &*(l as *const DrawItem);
                if item.control_type == 4 { // ODT_BUTTON
                    draw_button(item, app);
                    return 1;
                }
            }
            DefWindowProcW(hwnd, msg, w, l)
        }
        0x24 => {
            let points = l as *mut Point;
            (*points.add(3)).x = (760.0 * app.scale) as i32;
            (*points.add(3)).y = (660.0 * app.scale) as i32;
            0
        }
        5 => {
            if !app.controls.is_empty() {
                app.layout();
            }
            0
        }
        0x111 => {
            match w & 0xffff {
                102 => app.choose_zip(),
                104 => app.choose_output(),
                105 => app.start(true),
                106 => app.start(false),
                107 => {
                    app.stop.store(true, Ordering::Relaxed);
                    EnableWindow(app.controls[11], 0);
                    set(app.controls[12], "Отмена…");
                }
                110 if !app.busy => {
                    app.skip_missing = !app.skip_missing;
                    InvalidateRect(app.controls[15], null(), 1);
                }
                109 => {
                    ShellExecuteW(
                        hwnd,
                        wide("open").as_ptr(),
                        wide(&app.last_output.to_string_lossy()).as_ptr(),
                        null(),
                        null(),
                        1,
                    );
                }
                _ => {}
            }
            0
        }
        0x113 => {
            app.poll();
            0
        }
        0x233 => {
            if !app.busy {
                let mut buf = vec![0u16; 32768];
                let len = DragQueryFileW(w as H, 0, buf.as_mut_ptr(), buf.len() as u32);
                if len > 0 {
                    set(
                        app.controls[3],
                        &String::from_utf16_lossy(&buf[..len as usize]),
                    );
                }
            }
            DragFinish(w as H);
            0
        }
        0x10 => {
            if app.busy {
                app.stop.store(true, Ordering::Relaxed);
                dialog(hwnd,"Останавливаю преобразование. Дождитесь завершения отмены, затем закройте окно.");
                0
            } else {
                DefWindowProcW(hwnd, msg, w, l)
            }
        }
        2 => {
            KillTimer(hwnd, 1);
            DeleteObject(app.font);
            DeleteObject(app.titlefont);
            for brush in [app.bg_brush, app.header_brush, app.field_brush, app.log_brush] {
                DeleteObject(brush);
            }
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
pub fn launch(archive: Option<PathBuf>) {
    unsafe {
        SetProcessDPIAware();
        let scale = GetDpiForSystem() as f32 / 96.0;
        CoInitializeEx(null_mut(), 2);
        let instance = GetModuleHandleW(null());
        let name = wide("NotionObsidianRustWindow");
        let cls = Class {
            size: std::mem::size_of::<Class>() as u32,
            style: 3,
            proc: Some(procedure),
            cls: 0,
            win: 0,
            instance,
            icon: LoadIconW(instance, 1usize as *const u16),
            cursor: LoadCursorW(0, 32512usize as *const u16),
            brush: 0,
            menu: null(),
            name: name.as_ptr(),
            small_icon: LoadIconW(instance, 1usize as *const u16),
        };
        RegisterClassExW(&cls);
        let app = Box::new(App {
            hwnd: 0,
            controls: vec![],
            font: 0,
            titlefont: 0,
            bg_brush: 0,
            header_brush: 0,
            field_brush: 0,
            log_brush: 0,
            skip_missing: false,
            rx: None,
            stop: Arc::new(AtomicBool::new(false)),
            busy: false,
            last_output: PathBuf::new(),
            scale,
        });
        let ptr = Box::into_raw(app);
        let width = (940.0 * scale) as i32;
        let height = (770.0 * scale) as i32;
        let x = ((GetSystemMetrics(0) - width) / 2).max(0);
        let y = ((GetSystemMetrics(1) - height) / 2).max(0);
        let hwnd = CreateWindowExW(
            0,
            name.as_ptr(),
            wide("Notion → Obsidian · 0.5.0").as_ptr(),
            0x00cf0000,
            x,
            y,
            width,
            height,
            0,
            0,
            instance,
            ptr as *mut c_void,
        );
        if hwnd == 0 {
            dialog(0, "Не удалось создать окно приложения.");
            drop(Box::from_raw(ptr));
            CoUninitialize();
            return;
        }
        // Windows 10 1809+ / 11: opt into a dark native titlebar, if supported.
        let dark = 1i32;
        if DwmSetWindowAttribute(hwnd, 20, &dark, 4) != 0 {
            DwmSetWindowAttribute(hwnd, 19, &dark, 4);
        }
        if let Some(archive) = archive {
            set((&(*ptr).controls)[3], &archive.to_string_lossy());
        }
        ShowWindow(hwnd, 1);
        UpdateWindow(hwnd);
        let mut msg: Msg = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &mut msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        drop(Box::from_raw(ptr));
        CoUninitialize();
    }
}

// ABI sizes are checked against the Windows x64 SDK structures.
const _: [(); 80] = [(); std::mem::size_of::<Class>()];
const _: [(); 48] = [(); std::mem::size_of::<Msg>()];
const _: [(); 152] = [(); std::mem::size_of::<OpenFile>()];
const _: [(); 64] = [(); std::mem::size_of::<Browse>()];
const _: [(); 64] = [(); std::mem::size_of::<DrawItem>()];
