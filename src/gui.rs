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
// Windows x64 PAINTSTRUCT. Drawing is double-buffered to keep animations flicker-free.
#[repr(C)]
struct Paint {
    dc: H,
    erase: i32,
    area: Rect,
    restore: i32,
    inc_update: i32,
    reserved: [u8; 32],
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
const BG: u32 = rgb(18, 18, 20);
const FIELD: u32 = rgb(39, 38, 42);
const BORDER: u32 = rgb(52, 50, 55);
const TEXT: u32 = rgb(242, 241, 244);
const MUTED: u32 = rgb(162, 160, 169);
const ACCENT: u32 = rgb(119, 91, 237);
const TRACK: u32 = rgb(43, 41, 46);
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
    fn BeginPaint(hwnd: H, paint: *mut Paint) -> H;
    fn EndPaint(hwnd: H, paint: *const Paint) -> i32;
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
    fn CreateCompatibleDC(dc: H) -> H;
    fn DeleteDC(dc: H) -> i32;
    fn CreateCompatibleBitmap(dc: H, width: i32, height: i32) -> H;
    fn BitBlt(dst: H, dx: i32, dy: i32, width: i32, height: i32,
              src: H, sx: i32, sy: i32, op: u32) -> i32;
    fn MoveToEx(dc: H, x: i32, y: i32, old: *mut Point) -> i32;
    fn LineTo(dc: H, x: i32, y: i32) -> i32;
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


unsafe fn rule(dc: H, x1: i32, y: i32, x2: i32) {
    let pen = CreatePen(0, 1, BORDER);
    let previous = SelectObject(dc, pen);
    MoveToEx(dc, x1, y, null_mut());
    LineTo(dc, x2, y);
    SelectObject(dc, previous);
    DeleteObject(pen);
}

unsafe fn background(hwnd: H, dc: H, app: &App) {
    let mut client = Rect { left: 0, top: 0, right: 0, bottom: 0 };
    GetClientRect(hwnd, &mut client);
    FillRect(dc, &client, app.bg_brush);
    let v = |n: i32| (n as f32 * app.scale).round() as i32;
    let m = v(12);
    rule(dc, m, v(58), client.right - m);
    for y in [103, 183] {
        rounded(dc, &Rect {
            left: m, top: v(y), right: client.right - m, bottom: v(y + 44),
        }, FIELD, FIELD, v(14));
    }
    rule(dc, m, v(346), client.right - m);
    if app.progress_visible {
        let track = Rect {
            left: m, top: client.bottom - v(20),
            right: client.right - m, bottom: client.bottom - v(13),
        };
        rounded(dc, &track, TRACK, TRACK, v(7));
        let length = track.right - track.left;
        let progress = ((length as f32) * app.progress_display.clamp(0.0, 1.0)).round() as i32;
        if progress > 0 {
            rounded(dc, &Rect {
                right: (track.left + progress).min(track.right),
                ..track
            }, ACCENT, ACCENT, v(7));
        }
    }
}

unsafe fn draw_button(item: &DrawItem, app: &App) {
    let id = item.control_id;
    let browse = id == 102 || id == 104;
    FillRect(item.dc, &item.rect,
        if browse { app.field_brush } else { app.bg_brush });
    let disabled = item.item_state & 0x4 != 0;
    let down = item.item_state & 0x1 != 0;
    let focused = item.item_state & 0x10 != 0;
    let check = id == 110;
    let primary = id == 106 && !app.busy;
    let foreground = if disabled { rgb(114, 111, 121) } else { TEXT };

    if browse {
        let x = (item.rect.left + item.rect.right) / 2;
        let y = (item.rect.top + item.rect.bottom) / 2;
        let z = app.scale;
        let p = |n: i32| (n as f32 * z).round() as i32;
        let pen = CreatePen(0, p(2).max(1),
            if disabled { rgb(113, 110, 122) } else if down { TEXT } else { MUTED });
        let previous = SelectObject(item.dc, pen);
        MoveToEx(item.dc, x - p(9), y - p(3), null_mut());
        LineTo(item.dc, x - p(9), y - p(6));
        LineTo(item.dc, x - p(2), y - p(6));
        LineTo(item.dc, x + p(1), y - p(3));
        LineTo(item.dc, x + p(9), y - p(3));
        LineTo(item.dc, x + p(9), y + p(7));
        LineTo(item.dc, x - p(9), y + p(7));
        LineTo(item.dc, x - p(9), y - p(3));
        SelectObject(item.dc, previous);
        DeleteObject(pen);
        if focused {
            rounded(item.dc, &item.rect, FIELD, ACCENT, p(10));
        }
        return;
    }

    if check {
        let side = (16.0 * app.scale).round() as i32;
        let left = item.rect.left + (2.0 * app.scale).round() as i32;
        let center = (item.rect.top + item.rect.bottom) / 2;
        let check_rect = Rect {
            left, top: center - side / 2,
            right: left + side, bottom: center + side / 2,
        };
        rounded(item.dc, &check_rect,
            if app.skip_missing { ACCENT } else { BG },
            if app.skip_missing { ACCENT } else { MUTED },
            (5.0 * app.scale).round() as i32);
        if app.skip_missing {
            SetBkMode(item.dc, 1);
            SetTextColor(item.dc, TEXT);
            let tick = wide("✓");
            let mut r = check_rect;
            DrawTextW(item.dc, tick.as_ptr(), (tick.len() - 1) as i32,
                &mut r, 0x1 | 0x4 | 0x20);
        }
        SetBkMode(item.dc, 1);
        SetTextColor(item.dc, if disabled { MUTED } else { TEXT });
        let old = SelectObject(item.dc, app.font);
        let label = wide(&get(item.hwnd));
        let mut r = item.rect;
        r.left += (27.0 * app.scale).round() as i32;
        DrawTextW(item.dc, label.as_ptr(), (label.len() - 1) as i32,
            &mut r, 0x20 | 0x4 | 0x8000);
        SelectObject(item.dc, old);
        return;
    }

    let (fill, stroke) = if primary && !disabled {
        (if down { rgb(100, 76, 207) } else { ACCENT }, ACCENT)
    } else if id == 109 {
        (rgb(34, 31, 46), if focused { ACCENT } else { BORDER })
    } else {
        (if down { FIELD } else { BG },
         if focused { ACCENT } else { BORDER })
    };
    let radius = (15.0 * app.scale).round() as i32;
    rounded(item.dc, &item.rect, if disabled { BG } else { fill },
        if disabled { BORDER } else { stroke }, radius);
    SetBkMode(item.dc, 1);
    SetTextColor(item.dc, foreground);
    let old = SelectObject(item.dc, app.font);
    let label = wide(&get(item.hwnd));
    let mut text_rect = item.rect;
    DrawTextW(item.dc, label.as_ptr(), (label.len() - 1) as i32,
        &mut text_rect, 0x20 | 0x4 | 0x8000 | 0x1);
    SelectObject(item.dc, old);
}

fn count_event(message: &str, prefix: &str) -> Option<(usize, usize)> {
    let counts = message.strip_prefix(prefix)?;
    let (done, total) = counts.split_once('/')?;
    let done = done.trim().parse().ok()?;
    let total = total.trim().parse().ok()?;
    Some((done, total))
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
    field_brush: H,
    skip_missing: bool,
    open_ready: bool,
    rx: Option<Receiver<Event>>,
    stop: Arc<AtomicBool>,
    busy: bool,
    inspecting: bool,
    cancel_requested: bool,
    progress_visible: bool,
    progress_target: f32,
    progress_display: f32,
    last_output: PathBuf,
    scale: f32,
}
impl App {
    unsafe fn create(&mut self) {
        self.bg_brush = CreateSolidBrush(BG);
        self.field_brush = CreateSolidBrush(FIELD);
        self.font = CreateFontW(
            -(15.0 * self.scale) as i32, 0, 0, 0, 500,
            0, 0, 0, 1, 0, 0, 5, 0, wide("Segoe UI").as_ptr()
        );
        self.titlefont = CreateFontW(
            -(18.0 * self.scale) as i32, 0, 0, 0, 600,
            0, 0, 0, 1, 0, 0, 5, 0, wide("Segoe UI").as_ptr()
        );
        // Preserve control indices to keep the Win32 event routing stable.
        // 11 is deliberately unused: cancellation lives in the primary button.
        let defs = [
            ("STATIC", "Notion → Obsidian", 0u32, 0),
            ("STATIC", "Локально · без установки", 0x2, 0),
            ("STATIC", "Экспорт Notion", 0, 0),
            ("EDIT", "", 0x80 | 0x10000, 101),
            ("BUTTON", "Выбрать ZIP", 0x10000, 102),
            ("STATIC", "Папка хранилища Obsidian", 0, 0),
            ("EDIT", "", 0x80 | 0x10000, 103),
            ("BUTTON", "Выбрать папку", 0x10000, 104),
            ("STATIC", "", 0, 0),
            ("BUTTON", "Проверить ZIP", 0x10000, 105),
            ("BUTTON", "Конвертировать", 0x10000, 106),
            ("STATIC", "", 0, 0),
            ("STATIC", "Готово к конвертации", 0, 0),
            ("STATIC", "Исходные файлы останутся без изменений", 0, 0),
            ("BUTTON", "Открыть папку", 0x10000, 109),
            ("BUTTON", "Пропускать отсутствующие страницы", 0x10000, 110),
        ];
        for (class, text, style, id) in defs {
            let style = if class == "BUTTON" { (style & !0x0f) | 0xB } else { style };
            let h = CreateWindowExW(
                0, wide(class).as_ptr(), wide(text).as_ptr(),
                0x40000000 | 0x10000000 | style,
                0, 0, 0, 0, self.hwnd, id,
                GetModuleHandleW(null()), null_mut()
            );
            SendMessageW(h, 0x30,
                if self.controls.is_empty() { self.titlefont as usize }
                else { self.font as usize }, 1);
            self.controls.push(h);
        }
        let default = std::env::var_os("USERPROFILE")
            .map(PathBuf::from).unwrap_or(PathBuf::from("."))
            .join("Documents").join("Notion_Obsidian_v0.5.5");
        set(self.controls[6], &fresh_path(default).to_string_lossy());
        ShowWindow(self.controls[8], 0);
        ShowWindow(self.controls[11], 0);
        ShowWindow(self.controls[14], 0);
        for i in [3usize, 6] {
            SendMessageW(self.controls[i], 0x00D3, 3, (12 << 16) | 12);
            SetWindowTheme(self.controls[i], wide("DarkMode_Explorer").as_ptr(), null());
        }
        DragAcceptFiles(self.hwnd, 1);
        SetTimer(self.hwnd, 1, 33, 0);
        self.layout();
    }

    unsafe fn layout(&self) {
        let mut client = Rect { left: 0, top: 0, right: 0, bottom: 0 };
        GetClientRect(self.hwnd, &mut client);
        let w = client.right;
        let v = |n: i32| (n as f32 * self.scale).round() as i32;
        let m = v(12);
        let roww = w - 2 * m;
        let action_gap = v(10);
        let check_w = ((roww - action_gap) * 34 / 100).max(v(180));
        let input_w = (roww - v(68)).max(v(180));
        let status_w = (roww - v(188)).max(v(280));
        let rows = [
            (m, v(14), roww * 60 / 100, v(28)),  // App title
            (w - m - v(245), v(17), v(245), v(24)), // Privacy note
            (m, v(75), roww, v(22)),             // ZIP label
            (m + v(8), v(112), input_w, v(24)),  // ZIP edit
            (w - m - v(47), v(110), v(38), v(30)), // ZIP chooser
            (m, v(154), roww, v(22)),             // Output label
            (m + v(8), v(192), input_w, v(24)),  // Output edit
            (w - m - v(47), v(190), v(38), v(30)), // Output chooser
            (0, 0, 0, 0),                         // unused
            (m, v(281), check_w, v(49)),         // Secondary action
            (m + check_w + action_gap, v(281), roww - check_w - action_gap, v(49)),
            (0, 0, 0, 0),                         // unused
            (m + v(23), v(365), status_w - v(23), v(23)), // Status
            (m + v(23), v(390), status_w - v(23), v(22)), // Details
            (w - m - v(173), v(365), v(173), v(38)), // Open folder
            (m, v(239), roww, v(32)),           // Skip missing
        ];
        for (i, (x, y, width, height)) in rows.iter().enumerate() {
            if *width > 0 {
                MoveWindow(self.controls[i], *x, *y, *width, *height, 1);
            }
        }
        InvalidateRect(self.hwnd, null(), 0);
    }

    unsafe fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
        for i in [3, 4, 6, 7, 9, 15] {
            EnableWindow(self.controls[i], (!busy) as i32);
        }
        self.cancel_requested = false;
        EnableWindow(self.controls[10], 1);
        set(self.controls[10], if busy { "Отменить" } else { "Конвертировать" });
        InvalidateRect(self.controls[10], null(), 1);
    }

    unsafe fn start(&mut self, inspect: bool) {
        if self.busy { return; }
        let archive = PathBuf::from(get(self.controls[3]).trim());
        let output = PathBuf::from(get(self.controls[6]).trim());
        let skip_missing = self.skip_missing;
        if !archive.is_file() {
            dialog(self.hwnd, "Выберите существующий ZIP-файл.");
            return;
        }
        if !inspect && (get(self.controls[6]).trim().is_empty() || output.exists()) {
            dialog(self.hwnd,
                "Укажите новую папку, которой ещё нет. Существующие файлы не перезаписываются.");
            return;
        }
        self.open_ready = false;
        self.inspecting = inspect;
        self.progress_visible = true;
        self.progress_target = 0.0;
        self.progress_display = 0.0;
        ShowWindow(self.controls[14], 0);
        set(self.controls[12], if inspect { "Проверяем ZIP…" } else { "Подготавливаем конвертацию…" });
        set(self.controls[13], "Проверяем исходные файлы");
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
                    move |s| { let _ = progress.send(Event::Message(s.into())); }, stop,
                ).skip_missing_pages(skip_missing);
                if inspect { c.inspect(&archive) } else { c.run(&archive, &output) }
            })).unwrap_or_else(|_|
                Err("Внутренняя ошибка. Итоговая папка не опубликована.".into()));
            let _ = tx.send(Event::Done(result, inspect));
        });
        InvalidateRect(self.hwnd, null(), 0);
    }

    unsafe fn on_message(&mut self, s: &str) {
        if self.cancel_requested { return; }
        if let Some((done, total)) = count_event(s, "Сопоставлено баз: ") {
            let fraction = if total == 0 { 1.0 } else { (done as f32 / total as f32).min(1.0) };
            self.progress_target = if self.inspecting { 0.25 + 0.70 * fraction } else { 0.18 + 0.09 * fraction };
            set(self.controls[12], "Сопоставляем таблицы");
            set(self.controls[13], &format!("{done} из {total} баз"));
        } else if let Some((done, total)) = count_event(s, "Обработано страниц: ") {
            let fraction = if total == 0 { 1.0 } else { (done as f32 / total as f32).min(1.0) };
            self.progress_target = 0.28 + 0.54 * fraction;
            set(self.controls[12], "Переносим страницы");
            set(self.controls[13], &format!("{done} из {total} страниц"));
        } else if let Some((done, total)) = count_event(s, "Сохранено файлов: ") {
            let fraction = if total == 0 { 1.0 } else { (done as f32 / total as f32).min(1.0) };
            self.progress_target = 0.90 + 0.09 * fraction;
            set(self.controls[12], "Сохраняем хранилище");
            set(self.controls[13], &format!("{done} из {total} файлов"));
        } else if s.starts_with("Чтение ZIP") {
            self.progress_target = 0.06;
            set(self.controls[12], "Читаем архив");
            set(self.controls[13], "Проверяем ZIP и вложения");
        } else if s.starts_with("Сопоставление CSV") {
            self.progress_target = if self.inspecting { 0.25 } else { 0.18 };
            set(self.controls[12], "Сопоставляем таблицы");
            set(self.controls[13], "Находим записи и базы данных");
        } else if s.starts_with("Преобразование страниц") {
            self.progress_target = 0.28;
            set(self.controls[12], "Переносим страницы");
            set(self.controls[13], "Обрабатываем Markdown и ссылки");
        } else if s.starts_with("Проверка ссылок") {
            self.progress_target = 0.84;
            set(self.controls[12], "Проверяем ссылки и вложения");
            set(self.controls[13], "Сверяем структуру и контрольные суммы");
        } else if s.starts_with("Сохранение хранилища") {
            self.progress_target = 0.90;
            set(self.controls[12], "Сохраняем хранилище");
            set(self.controls[13], "Записываем готовые файлы");
        } else if s.starts_with("Готово.") {
            self.progress_target = 0.99;
            set(self.controls[12], "Завершаем перенос…");
        }
    }

    unsafe fn poll(&mut self) {
        let events = self.rx.as_ref()
            .map(|r| r.try_iter().collect::<Vec<_>>()).unwrap_or_default();
        for event in events {
            match event {
                Event::Message(s) => self.on_message(&s),
                Event::Done(result, inspect) => {
                    let cancelled = self.cancel_requested;
                    self.set_busy(false);
                    self.rx = None;
                    match result {
                        Ok(r) if !cancelled => {
                            self.progress_visible = true;
                            self.progress_target = 1.0;
                            self.progress_display = 1.0;
                            set(self.controls[12],
                                if inspect { "Проверка завершена" } else { "Хранилище готово" });
                            let pages = r.stats.get("source_pages").copied().unwrap_or(0);
                            let dbs = r.stats.get("source_databases").copied().unwrap_or(0);
                            let rows = r.stats.get("source_rows").copied().unwrap_or(0);
                            let warnings = r.warnings.len();
                            set(self.controls[13], &format!(
                                "{pages} страниц · {dbs} баз · {rows} записей · {warnings} предупреждений"));
                            if !inspect {
                                self.open_ready = true;
                                ShowWindow(self.controls[14], 1);
                                EnableWindow(self.controls[14], 1);
                            }
                        }
                        Ok(_) => {
                            // A cancellation after publication cannot remove a completed Vault.
                            // Report actual success rather than pretending it was rolled back.
                            self.progress_target = 1.0;
                            self.progress_display = 1.0;
                            self.open_ready = !inspect;
                            set(self.controls[12], if inspect { "Проверка завершена" } else { "Хранилище готово" });
                            set(self.controls[13], "Операция завершилась до обработки запроса отмены");
                            if !inspect { ShowWindow(self.controls[14], 1); }
                        }
                        Err(e) => {
                            self.progress_visible = false;
                            self.progress_target = 0.0;
                            self.progress_display = 0.0;
                            if cancelled || e.contains("отменено") {
                                set(self.controls[12], "Операция отменена");
                                set(self.controls[13], "Итоговая папка не создана");
                            } else {
                                set(self.controls[12], if inspect { "Ошибка проверки ZIP" } else { "Ошибка конвертации" });
                                set(self.controls[13], "Подробности в сообщении об ошибке");
                                dialog(self.hwnd, &e);
                            }
                        }
                    }
                    InvalidateRect(self.hwnd, null(), 0);
                }
            }
        }
    }

    unsafe fn tick(&mut self) {
        self.poll();
        let delta = self.progress_target - self.progress_display;
        if self.progress_visible && delta.abs() > 0.001 {
            self.progress_display += delta * 0.22;
            if (self.progress_target - self.progress_display).abs() < 0.002 {
                self.progress_display = self.progress_target;
            }
            InvalidateRect(self.hwnd, null(), 0);
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
                        .join("Notion_Obsidian_v0.5.4"),
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
    if ptr.is_null() { return DefWindowProcW(hwnd, msg, w, l); }
    let app = &mut *ptr;
    match msg {
        0x14 => 1, // WM_ERASEBKGND: WM_PAINT handles all drawing without flicker
        0x0F => { // WM_PAINT
            let mut paint: Paint = std::mem::zeroed();
            let screen = BeginPaint(hwnd, &mut paint);
            if screen != 0 {
                let mut client = Rect { left: 0, top: 0, right: 0, bottom: 0 };
                GetClientRect(hwnd, &mut client);
                if client.right > 0 && client.bottom > 0 {
                    let memory = CreateCompatibleDC(screen);
                    let bitmap = CreateCompatibleBitmap(screen, client.right, client.bottom);
                    if memory != 0 && bitmap != 0 {
                        let previous = SelectObject(memory, bitmap);
                        background(hwnd, memory, app);
                        BitBlt(screen, 0, 0, client.right, client.bottom, memory, 0, 0, 0x00CC0020);
                        SelectObject(memory, previous);
                    }
                    if bitmap != 0 { DeleteObject(bitmap); }
                    if memory != 0 { DeleteDC(memory); }
                }
                EndPaint(hwnd, &paint);
            }
            0
        }
        0x133 | 0x138 => { // WM_CTLCOLOREDIT / WM_CTLCOLORSTATIC
            let dc = w as H;
            let index = app.controls.iter().position(|&h| h == l as H);
            let edit = index == Some(3) || index == Some(6);
            SetBkColor(dc, if edit { FIELD } else { BG });
            SetBkMode(dc, if edit { 2 } else { 1 });
            SetTextColor(dc, if edit || matches!(index, Some(0 | 2 | 5 | 12)) { TEXT } else { MUTED });
            if edit { app.field_brush } else { app.bg_brush }
        }
        0x2b => { // WM_DRAWITEM
            if l != 0 {
                let item = &*(l as *const DrawItem);
                if item.control_type == 4 {
                    draw_button(item, app);
                    return 1;
                }
            }
            DefWindowProcW(hwnd, msg, w, l)
        }
        0x24 => { // WM_GETMINMAXINFO
            let points = l as *mut Point;
            (*points.add(3)).x = (760.0 * app.scale).round() as i32;
            (*points.add(3)).y = (485.0 * app.scale).round() as i32;
            0
        }
        5 => { // WM_SIZE
            if !app.controls.is_empty() { app.layout(); }
            0
        }
        0x111 => { // WM_COMMAND
            match w & 0xffff {
                102 => if !app.busy { app.choose_zip() },
                104 => if !app.busy { app.choose_output() },
                105 => app.start(true),
                106 => {
                    if app.busy {
                        if !app.cancel_requested {
                            app.cancel_requested = true;
                            app.stop.store(true, Ordering::Relaxed);
                            set(app.controls[10], "Отменяем…");
                            EnableWindow(app.controls[10], 0);
                            set(app.controls[12], "Отменяем операцию…");
                            set(app.controls[13], "Очищаем временные файлы");
                        }
                    } else {
                        app.start(false);
                    }
                }
                110 if !app.busy => {
                    app.skip_missing = !app.skip_missing;
                    InvalidateRect(app.controls[15], null(), 1);
                }
                109 if app.open_ready => {
                    ShellExecuteW(
                        hwnd, wide("open").as_ptr(),
                        wide(&app.last_output.to_string_lossy()).as_ptr(),
                        null(), null(), 1,
                    );
                }
                _ => {}
            }
            0
        }
        0x113 => { app.tick(); 0 } // WM_TIMER
        0x233 => { // WM_DROPFILES
            if !app.busy {
                let mut buf = vec![0u16; 32768];
                let len = DragQueryFileW(w as H, 0, buf.as_mut_ptr(), buf.len() as u32);
                if len > 0 {
                    set(app.controls[3], &String::from_utf16_lossy(&buf[..len as usize]));
                }
            }
            DragFinish(w as H);
            0
        }
        0x10 => { // WM_CLOSE
            if app.busy {
                app.stop.store(true, Ordering::Relaxed);
                app.cancel_requested = true;
                set(app.controls[10], "Отменяем…");
                EnableWindow(app.controls[10], 0);
                set(app.controls[12], "Отменяем операцию…");
                set(app.controls[13], "Дождитесь завершения отмены");
                0
            } else {
                DefWindowProcW(hwnd, msg, w, l)
            }
        }
        2 => {
            KillTimer(hwnd, 1);
            DeleteObject(app.font);
            DeleteObject(app.titlefont);
            for brush in [app.bg_brush, app.field_brush] { DeleteObject(brush); }
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
            field_brush: 0,
            skip_missing: true,
            open_ready: false,
            rx: None,
            stop: Arc::new(AtomicBool::new(false)),
            busy: false,
            inspecting: false,
            cancel_requested: false,
            progress_visible: false,
            progress_target: 0.0,
            progress_display: 0.0,
            last_output: PathBuf::new(),
            scale,
        });
        let ptr = Box::into_raw(app);
        let width = (790.0 * scale) as i32;
        let height = (490.0 * scale) as i32;
        let x = ((GetSystemMetrics(0) - width) / 2).max(0);
        let y = ((GetSystemMetrics(1) - height) / 2).max(0);
        let hwnd = CreateWindowExW(
            0,
            name.as_ptr(),
            wide("Notion → Obsidian").as_ptr(),
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
