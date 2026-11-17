#![allow(non_snake_case)]
use notion_to_obsidian::{Converter, Report, options::ConversionOptions};
#[path = "settings.rs"]
mod settings;
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
// TRACKMOUSEEVENT, used by native Win32 folder buttons (x64 alignment).
#[repr(C)]
struct TrackMouse {
    size: u32,
    flags: u32,
    hwnd: H,
    hover_time: u32,
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
// TOOLINFOW / comctl32 tooltip ABI on Windows x64.
#[repr(C)]
struct ToolInfo {
    size: u32,
    flags: u32,
    owner: H,
    id: usize,
    rect: Rect,
    instance: H,
    text: *mut u16,
    param: isize,
    reserved: *mut c_void,
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
// Keep the dark field background independent from the smaller, vertically
// centered native EDIT control. Win32 one-line EDIT does not center its text
// within a tall window, even when the height matches the painted field.
const FIELD_TOPS: [i32; 2] = [103, 183];
const FIELD_HEIGHT: i32 = 44;
const EDIT_HEIGHT: i32 = 24;
const BROWSE_HEIGHT: i32 = 34;
const IDLE_WINDOW_HEIGHT: i32 = 515;
const STATUS_WINDOW_HEIGHT: i32 = 615;
const ZIP_HELP: &str = "В Notion выберите экспорт Markdown & CSV.\nВключите Include subpages и Create folders for subpages.\nСкачайте ZIP-архив, затем укажите его в этом поле.\nИсходный архив не изменяется. Результат создаётся в новой папке.";
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
// Native Explorer-style folder picker (IFileDialog with FOS_PICKFOLDERS).
// The vtables below follow the Windows SDK COM method order for x64.
#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}
const CLSID_FILE_OPEN_DIALOG: Guid = Guid {
    data1: 0xdc1c5a9c, data2: 0xe88a, data3: 0x4dde,
    data4: [0xa5, 0xa1, 0x60, 0xf8, 0x2a, 0x20, 0xae, 0xf7],
};
const IID_IFILE_DIALOG: Guid = Guid {
    data1: 0x42f85136, data2: 0xdb7e, data3: 0x439c,
    data4: [0x85, 0xf1, 0xe4, 0x07, 0x5d, 0x13, 0x5f, 0xc8],
};
#[repr(C)]
struct FileDialog {
    vtable: *const FileDialogVTable,
}
#[repr(C)]
struct FileDialogVTable {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(*mut FileDialog) -> u32,
    show: unsafe extern "system" fn(*mut FileDialog, H) -> i32,
    set_file_types: usize,
    set_file_type_index: usize,
    get_file_type_index: usize,
    advise: usize,
    unadvise: usize,
    set_options: unsafe extern "system" fn(*mut FileDialog, u32) -> i32,
    get_options: unsafe extern "system" fn(*mut FileDialog, *mut u32) -> i32,
    set_default_folder: usize,
    set_folder: usize,
    get_folder: usize,
    get_current_selection: usize,
    set_file_name: usize,
    get_file_name: usize,
    set_title: unsafe extern "system" fn(*mut FileDialog, *const u16) -> i32,
    set_ok_button_label: unsafe extern "system" fn(*mut FileDialog, *const u16) -> i32,
    set_file_name_label: usize,
    get_result: unsafe extern "system" fn(*mut FileDialog, *mut *mut ShellItem) -> i32,
}
#[repr(C)]
struct ShellItem {
    vtable: *const ShellItemVTable,
}
#[repr(C)]
struct ShellItemVTable {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(*mut ShellItem) -> u32,
    bind_to_handler: usize,
    get_parent: usize,
    get_display_name: unsafe extern "system" fn(*mut ShellItem, u32, *mut *mut u16) -> i32,
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
    fn CallWindowProcW(proc: isize, hwnd: H, msg: u32, w: usize, l: isize) -> isize;
    fn GetParent(hwnd: H) -> H;
    fn TrackMouseEvent(event: *mut TrackMouse) -> i32;
    fn SetCursor(cursor: H) -> H;
    fn SetFocus(hwnd: H) -> H;
    fn SendMessageW(hwnd: H, msg: u32, w: usize, l: isize) -> isize;
    fn SetWindowTextW(hwnd: H, text: *const u16) -> i32;
    fn GetWindowTextLengthW(hwnd: H) -> i32;
    fn GetWindowTextW(hwnd: H, text: *mut u16, max: i32) -> i32;
    fn IsWindowEnabled(hwnd: H) -> i32;
    fn EnableWindow(hwnd: H, enable: i32) -> i32;
    fn MoveWindow(hwnd: H, x: i32, y: i32, w: i32, h: i32, repaint: i32) -> i32;
    fn GetClientRect(hwnd: H, rect: *mut Rect) -> i32;
    fn GetWindowRect(hwnd: H, rect: *mut Rect) -> i32;
    fn SetWindowPos(hwnd: H, after: H, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
    fn DestroyWindow(hwnd: H) -> i32;
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
#[link(name = "comctl32")]
extern "system" {
    fn InitCommonControls();
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
    fn CoCreateInstance(clsid: *const Guid, outer: *mut c_void, context: u32,
        iid: *const Guid, object: *mut *mut c_void) -> i32;
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
    for y in FIELD_TOPS {
        rounded(dc, &Rect {
            left: m, top: v(y), right: client.right - m,
            bottom: v(y + FIELD_HEIGHT),
        }, FIELD, FIELD, v(14));
    }
    if app.status_visible {
        rule(dc, m, v(386), client.right - m);
    }
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
    let hovered = app.hover_button == Some(id);
    let help = id == 112;
    FillRect(item.dc, &item.rect,
        if browse { app.field_brush } else { app.bg_brush });
    let disabled = item.item_state & 0x4 != 0;
    let down = item.item_state & 0x1 != 0;
    let focused = item.item_state & 0x10 != 0;
    let check = id == 110 || id == 111;
    let primary = id == 106 && !app.busy;
    let foreground = if disabled { rgb(114, 111, 121) } else { TEXT };

    if help {
        let p = |n: i32| (n as f32 * app.scale).round() as i32;
        let mut circle = item.rect;
        circle.left += p(1);
        circle.top += p(1);
        circle.right -= p(1);
        circle.bottom -= p(1);
        rounded(item.dc, &circle,
            if hovered || focused { rgb(52, 46, 71) } else { BG },
            if hovered || focused { ACCENT } else { MUTED }, p(20));
        SetBkMode(item.dc, 1);
        SetTextColor(item.dc, if hovered || focused { TEXT } else { MUTED });
        let old = SelectObject(item.dc, app.font);
        let question = wide("?");
        let mut r = item.rect;
        DrawTextW(item.dc, question.as_ptr(), 1, &mut r, 0x20 | 0x4 | 0x1);
        SelectObject(item.dc, old);
        return;
    }

    if browse {
        let x = item.rect.left + (18.0 * app.scale).round() as i32;
        let y = (item.rect.top + item.rect.bottom) / 2;
        let z = app.scale;
        let p = |n: i32| (n as f32 * z).round() as i32;
        // Draw hover/focus surface FIRST: drawing it after the icon erased the icon.
        let fill = if disabled { FIELD } else if down {
            rgb(65, 58, 91)
        } else if hovered {
            rgb(55, 50, 73)
        } else {
            FIELD
        };
        let outline = if disabled { FIELD } else if focused {
            ACCENT
        } else if hovered {
            rgb(90, 78, 144)
        } else {
            FIELD
        };
        rounded(item.dc, &item.rect, fill, outline, p(10));
        let color = if disabled { rgb(113, 110, 122) } else if hovered || down {
            TEXT
        } else {
            MUTED
        };
        let pen = CreatePen(0, p(2).max(1), color);
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
        SetBkMode(item.dc, 1);
        SetTextColor(item.dc, if disabled { MUTED } else { TEXT });
        let old = SelectObject(item.dc, app.font);
        let caption = wide("Обзор");
        let mut label_rect = item.rect;
        label_rect.left += p(33);
        DrawTextW(item.dc, caption.as_ptr(), (caption.len() - 1) as i32,
            &mut label_rect, 0x20 | 0x4 | 0x8000);
        SelectObject(item.dc, old);
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
        let checked = if id == 110 { app.skip_missing } else { app.write_report };
        rounded(item.dc, &check_rect,
            if checked { ACCENT } else { BG },
            if checked { ACCENT } else { MUTED },
            (5.0 * app.scale).round() as i32);
        if checked {
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
        (if down { rgb(100, 76, 207) } else if hovered {
            rgb(137, 111, 245)
        } else { ACCENT }, ACCENT)
    } else if id == 109 {
        (if hovered { rgb(55, 48, 79) } else { rgb(34, 31, 46) },
         if focused || hovered { ACCENT } else { BORDER })
    } else {
        (if down { FIELD } else if hovered { rgb(43, 40, 53) } else { BG },
         if focused { ACCENT } else if hovered { rgb(94, 81, 156) } else { BORDER })
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
    Done(std::result::Result<Report, String>),
}
struct App {
    hwnd: H,
    controls: Vec<H>,
    font: H,
    titlefont: H,
    bg_brush: H,
    field_brush: H,
    button_procs: [isize; 6],
    hover_button: Option<u32>,
    tooltip: H,
    help_text: Vec<u16>,
    status_visible: bool,
    skip_missing: bool,
    write_report: bool,
    options: ConversionOptions,
    open_ready: bool,
    rx: Option<Receiver<Event>>,
    stop: Arc<AtomicBool>,
    busy: bool,
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
        // Keep original control indices; append help control after checkboxes.
        let defs = [
            ("STATIC", "Notion → Obsidian", 0u32, 0),
            ("STATIC", "", 0, 0), // reserved index; no subtitle
            ("STATIC", "ZIP-экспорт Notion", 0, 0),
            ("EDIT", "", 0x80 | 0x10000, 101),
            ("BUTTON", "Выбрать ZIP", 0x10000, 102),
            ("STATIC", "Новое хранилище Obsidian", 0, 0),
            ("EDIT", "", 0x80 | 0x10000, 103),
            ("BUTTON", "Выбрать папку", 0x10000, 104),
            ("STATIC", "", 0, 0),
            ("STATIC", "", 0, 0), // reserved index; no ZIP-check button
            ("BUTTON", "Конвертировать", 0x10000, 106),
            ("BUTTON", "Создавать отчёт о переносе", 0x10000, 111),
            ("STATIC", "", 0, 0), // status is populated only during conversion
            ("STATIC", "", 0, 0), // no idle explanation in the footer
            ("BUTTON", "Открыть папку", 0x10000, 109),
            ("BUTTON", "Пропускать отсутствующие страницы", 0x10000, 110),
            ("BUTTON", "Справка по ZIP", 0x10000, 112),
            ("STATIC", "Профиль экспорта", 0, 0),
            ("COMBOBOX", "", 0x10000 | 0x00200000 | 0x3, 118),
            ("BUTTON", "Настроить…", 0x10000, 119),
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
        for label in ["Универсальный", "База знаний", "Пользовательский"] {
            SendMessageW(self.controls[18], 0x0143, 0, wide(label).as_ptr() as isize);
        }
        SendMessageW(self.controls[18], 0x014E, 1, 0);
        SetWindowTheme(self.controls[18], wide("DarkMode_Explorer").as_ptr(), null());
        let default = std::env::var_os("USERPROFILE")
            .map(PathBuf::from).unwrap_or(PathBuf::from("."))
            .join("Documents").join("Notion_Obsidian");
        set(self.controls[6], &fresh_path(default).to_string_lossy());
        // EM_SETCUEBANNER: a visible hint when the ZIP path is empty.
        SendMessageW(self.controls[3], 0x1501, 0,
            wide("Нажмите «Обзор» или перетащите ZIP").as_ptr() as isize);
        ShowWindow(self.controls[1], 0);
        ShowWindow(self.controls[8], 0);
        ShowWindow(self.controls[9], 0);
        ShowWindow(self.controls[12], 0);
        ShowWindow(self.controls[13], 0);
        ShowWindow(self.controls[14], 0);
        for i in [3usize, 6] {
            // EM_SETMARGINS is horizontal only. Vertical alignment comes from
            // centering a single-line EDIT in the larger painted field.
            let margin = (8.0 * self.scale).round() as usize;
            SendMessageW(self.controls[i], 0x00D3, 3,
                ((margin << 16) | margin) as isize);
            SetWindowTheme(self.controls[i], wide("DarkMode_Explorer").as_ptr(), null());
        }
        // Native Windows tooltip: TTF_SUBCLASS handles mouse hover on the help button.
        self.tooltip = CreateWindowExW(
            0x00000008, wide("tooltips_class32").as_ptr(), null(),
            0x80000000 | 0x1 | 0x2, 0, 0, 0, 0,
            self.hwnd, 0, GetModuleHandleW(null()), null_mut(),
        );
        if self.tooltip != 0 {
            let mut info = ToolInfo {
                size: std::mem::size_of::<ToolInfo>() as u32,
                flags: 0x1 | 0x10, // TTF_IDISHWND | TTF_SUBCLASS
                owner: self.hwnd,
                id: self.controls[16] as usize,
                rect: Rect { left: 0, top: 0, right: 0, bottom: 0 },
                instance: 0,
                text: self.help_text.as_mut_ptr(),
                param: 0,
                reserved: null_mut(),
            };
            SendMessageW(self.tooltip, 0x0432, 0, &mut info as *mut ToolInfo as isize); // TTM_ADDTOOLW
            SendMessageW(self.tooltip, 0x0418, 0, (360.0 * self.scale) as isize); // TTM_SETMAXTIPWIDTH
        }
        // Hover feedback and a pointing-hand cursor for action and help buttons.
        for (slot, index) in [4usize, 7, 10, 14, 16, 19].iter().enumerate() {
            self.button_procs[slot] =
                SetWindowLongPtrW(self.controls[*index], -4, button_proc as isize);
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
        let browse_w = v(88);
        let browse_x = w - m - browse_w - v(6);
        let edit_x = m + v(6);
        let edit_w = (browse_x - v(12) - edit_x).max(v(180));
        let status_w = (roww - v(188)).max(v(280));
        let field_h = v(FIELD_HEIGHT);
        let edit_h = v(EDIT_HEIGHT);
        let browse_h = v(BROWSE_HEIGHT);
        let zip_top = v(FIELD_TOPS[0]);
        let output_top = v(FIELD_TOPS[1]);
        // Both controls share the same field center at every display scale.
        let centered_y = |top: i32, child_height: i32| {
            top + (field_h - child_height) / 2
        };
        let rows = [
            (m, v(14), roww, v(28)),             // App title
            (0, 0, 0, 0),                         // No subtitle
            (m, v(75), roww, v(22)),             // ZIP label
            (edit_x, centered_y(zip_top, edit_h), edit_w, edit_h), // ZIP text
            (browse_x, centered_y(zip_top, browse_h), browse_w, browse_h), // ZIP chooser
            (m, v(154), roww, v(22)),             // Output label
            (edit_x, centered_y(output_top, edit_h), edit_w, edit_h), // Output text
            (browse_x, centered_y(output_top, browse_h), browse_w, browse_h), // Output chooser
            (0, 0, 0, 0),                         // unused
            (0, 0, 0, 0),                         // removed inspect action
            (m, v(385), roww, v(49)),              // Full-width primary action
            (m, v(339), roww, v(32)),              // Optional report checkbox
            (m + v(23), v(478), status_w - v(23), v(23)), // Status
            (m + v(23), v(503), status_w - v(23), v(22)), // Details
            (w - m - v(173), v(478), v(173), v(38)), // Open folder
            (m, v(305), roww, v(32)),              // Skip missing checkbox
            (m + v(155), v(74), v(22), v(22)),     // ZIP help tooltip trigger
            (m, v(236), roww, v(22)),              // Profile label
            (m, v(261), roww - v(155), v(215)),   // Native profile dropdown
            (w - m - v(141), v(261), v(141), v(32)), // Configure profile
        ];
        for (i, (x, y, width, height)) in rows.iter().enumerate() {
            if *width > 0 {
                MoveWindow(self.controls[i], *x, *y, *width, *height, 1);
            }
        }
        InvalidateRect(self.hwnd, null(), 0);
    }

    // Keep the initial window compact; reveal a dedicated status section
    // only during processing and after completion (or an error).
    unsafe fn set_status_visible(&mut self, visible: bool) {
        if self.status_visible == visible { return; }
        self.status_visible = visible;
        ShowWindow(self.controls[12], visible as i32);
        ShowWindow(self.controls[13], visible as i32);
        if !visible {
            self.progress_visible = false;
            self.progress_target = 0.0;
            self.progress_display = 0.0;
            self.open_ready = false;
            ShowWindow(self.controls[14], 0);
        }
        let mut bounds = Rect { left: 0, top: 0, right: 0, bottom: 0 };
        GetWindowRect(self.hwnd, &mut bounds);
        let height = if visible { STATUS_WINDOW_HEIGHT } else { IDLE_WINDOW_HEIGHT };
        SetWindowPos(self.hwnd, 0, 0, 0, bounds.right - bounds.left,
            (height as f32 * self.scale).round() as i32,
            0x0002 | 0x0004 | 0x0010); // SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE
        InvalidateRect(self.hwnd, null(), 0);
    }

    unsafe fn invalidate_progress(&self) {
        let mut client = Rect { left: 0, top: 0, right: 0, bottom: 0 };
        GetClientRect(self.hwnd, &mut client);
        let v = |n: i32| (n as f32 * self.scale).round() as i32;
        let track = Rect {
            left: v(10), top: client.bottom - v(23),
            right: client.right - v(10), bottom: client.bottom - v(10),
        };
        // Only redraw the progress strip: painting the whole window on every
        // animation tick used to cover child controls and cause visible flashes.
        InvalidateRect(self.hwnd, &track, 0);
    }

    unsafe fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
        self.hover_button = None;
        for i in [4usize, 7, 10, 14] {
            InvalidateRect(self.controls[i], null(), 0);
        }
        for i in [3, 4, 6, 7, 11, 15] {
            EnableWindow(self.controls[i], (!busy) as i32);
        }
        self.cancel_requested = false;
        EnableWindow(self.controls[10], 1);
        set(self.controls[10], if busy { "Отменить" } else { "Конвертировать" });
        InvalidateRect(self.controls[10], null(), 1);
    }

    unsafe fn start(&mut self) {
        if self.busy { return; }
        let archive = PathBuf::from(get(self.controls[3]).trim());
        let output = PathBuf::from(get(self.controls[6]).trim());
        let skip_missing = self.skip_missing;
        let write_report = self.write_report;
        if !archive.is_file() {
            dialog(self.hwnd, "Выберите существующий ZIP-файл.");
            return;
        }
        if get(self.controls[6]).trim().is_empty() || output.exists() {
            dialog(self.hwnd,
                "Укажите новую папку, которой ещё нет. Существующие файлы не перезаписываются.");
            return;
        }
        self.open_ready = false;
        self.set_status_visible(true);
        self.progress_visible = true;
        self.progress_target = 0.0;
        self.progress_display = 0.0;
        ShowWindow(self.controls[14], 0);
        set(self.controls[12], "Подготавливаем конвертацию…");
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
                ).skip_missing_pages(skip_missing).write_report(write_report);
                c.run(&archive, &output)
            })).unwrap_or_else(|_|
                Err("Внутренняя ошибка. Итоговая папка не опубликована.".into()));
            let _ = tx.send(Event::Done(result));
        });
        InvalidateRect(self.hwnd, null(), 0);
    }

    unsafe fn on_message(&mut self, s: &str) {
        if self.cancel_requested { return; }
        if let Some((done, total)) = count_event(s, "Сопоставлено баз: ") {
            let fraction = if total == 0 { 1.0 } else { (done as f32 / total as f32).min(1.0) };
            self.progress_target = 0.18 + 0.09 * fraction;
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
            self.progress_target = 0.18;
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
                Event::Done(result) => {
                    let cancelled = self.cancel_requested;
                    self.set_busy(false);
                    self.rx = None;
                    match result {
                        Ok(r) if !cancelled => {
                            self.progress_visible = true;
                            self.progress_target = 1.0;
                            self.progress_display = 1.0;
                            set(self.controls[12], "Хранилище готово");
                            let pages = r.stats.get("source_pages").copied().unwrap_or(0);
                            let dbs = r.stats.get("source_databases").copied().unwrap_or(0);
                            let rows = r.stats.get("source_rows").copied().unwrap_or(0);
                            let warnings = r.warnings.len();
                            set(self.controls[13], &format!(
                                "{pages} страниц · {dbs} баз · {rows} записей · {warnings} предупреждений"));
                            self.open_ready = true;
                            ShowWindow(self.controls[14], 1);
                            EnableWindow(self.controls[14], 1);
                        }
                        Ok(_) => {
                            // A cancellation after publication cannot remove a completed Vault.
                            // Report actual success rather than pretending it was rolled back.
                            self.progress_target = 1.0;
                            self.progress_display = 1.0;
                            self.open_ready = true;
                            set(self.controls[12], "Хранилище готово");
                            set(self.controls[13], "Операция завершилась до обработки запроса отмены");
                            ShowWindow(self.controls[14], 1);
                        }
                        Err(e) => {
                            self.progress_visible = false;
                            self.progress_target = 0.0;
                            self.progress_display = 0.0;
                            if cancelled || e.contains("отменено") {
                                set(self.controls[12], "Операция отменена");
                                set(self.controls[13], "Итоговая папка не создана");
                            } else {
                                set(self.controls[12], "Ошибка конвертации");
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
            self.invalidate_progress();
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
            // EN_CHANGE resets any previous conversion result and collapses status.
        }
        SetFocus(self.hwnd);
    }
    unsafe fn choose_output(&self) {
        // The legacy SHBrowseForFolder tree did not behave like File Explorer.
        // IFileDialog with FOS_PICKFOLDERS uses the current Windows Explorer UI.
        let mut object: *mut c_void = null_mut();
        if CoCreateInstance(&CLSID_FILE_OPEN_DIALOG, null_mut(), 1,
            &IID_IFILE_DIALOG, &mut object) < 0 || object.is_null()
        {
            dialog(self.hwnd, "Не удалось открыть системный выбор папки.");
            return;
        }
        let picker = object as *mut FileDialog;
        let methods = &*(*picker).vtable;
        let mut options = 0u32;
        // FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST
        if (methods.get_options)(picker, &mut options) < 0 ||
            (methods.set_options)(picker, options | 0x20 | 0x40 | 0x800) < 0
        {
            dialog(self.hwnd, "Не удалось настроить выбор папки.");
        } else {
            (methods.set_title)(picker, wide("Выберите папку для нового хранилища").as_ptr());
            (methods.set_ok_button_label)(picker, wide("Выбрать").as_ptr());
            if (methods.show)(picker, self.hwnd) >= 0 {
                let mut item: *mut ShellItem = null_mut();
                if (methods.get_result)(picker, &mut item) >= 0 && !item.is_null() {
                    let item_methods = &*(*item).vtable;
                    let mut path: *mut u16 = null_mut();
                    // SIGDN_FILESYSPATH: produce a real file-system path.
                    if (item_methods.get_display_name)(item, 0x80058000, &mut path) >= 0
                        && !path.is_null()
                    {
                        let mut len = 0;
                        while len < 32768 && *path.add(len) != 0 { len += 1; }
                        let folder = PathBuf::from(String::from_utf16_lossy(
                            std::slice::from_raw_parts(path, len),
                        ));
                        let target = fresh_path(folder.join("Notion_Obsidian"));
                        set(self.controls[6], &target.to_string_lossy());
                        // EN_CHANGE clears any stale completion status.
                        CoTaskMemFree(path as *mut c_void);
                    }
                    (item_methods.release)(item);
                }
            }
        }
        (methods.release)(picker);
        SetFocus(self.hwnd);
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
// Subclass action buttons only; keep edit fields and the checkbox native.
unsafe extern "system" fn button_proc(hwnd: H, msg: u32, w: usize, l: isize) -> isize {
    let parent = GetParent(hwnd);
    let ptr = GetWindowLongPtrW(parent, -21) as *mut App;
    if ptr.is_null() { return DefWindowProcW(hwnd, msg, w, l); }
    let app = &mut *ptr;
    let (slot, id) = if hwnd == app.controls[4] { (0, 102) }
        else if hwnd == app.controls[7] { (1, 104) }
        else if hwnd == app.controls[10] { (2, 106) }
        else if hwnd == app.controls[14] { (3, 109) }
        else if hwnd == app.controls[16] { (4, 112) }
        else { return DefWindowProcW(hwnd, msg, w, l); };
    match msg {
        0x0200 if IsWindowEnabled(hwnd) != 0 => { // WM_MOUSEMOVE
            if app.hover_button != Some(id) {
                app.hover_button = Some(id);
                let mut track = TrackMouse {
                    size: std::mem::size_of::<TrackMouse>() as u32,
                    flags: 0x2, // TME_LEAVE
                    hwnd,
                    hover_time: 0,
                };
                TrackMouseEvent(&mut track);
                InvalidateRect(hwnd, null(), 0);
            }
        }
        0x02A3 => { // WM_MOUSELEAVE
            if app.hover_button == Some(id) {
                app.hover_button = None;
                InvalidateRect(hwnd, null(), 0);
            }
        }
        0x0020 if IsWindowEnabled(hwnd) != 0 => { // WM_SETCURSOR
            SetCursor(LoadCursorW(0, 32649usize as *const u16)); // IDC_HAND
            return 1;
        }
        _ => {}
    }
    let previous = app.button_procs[slot];
    if previous == 0 { DefWindowProcW(hwnd, msg, w, l) }
    else { CallWindowProcW(previous, hwnd, msg, w, l) }
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
            let min_height = if app.status_visible { 535.0 } else { 435.0 };
            (*points.add(3)).y = (min_height * app.scale).round() as i32;
            0
        }
        5 => { // WM_SIZE
            if !app.controls.is_empty() { app.layout(); }
            0
        }
        0x111 => { // WM_COMMAND
            // EN_CHANGE on a path field invalidates any previous conversion summary.
            if ((w & 0xffff) == 101 || (w & 0xffff) == 103)
                && ((w >> 16) & 0xffff) == 0x0300 && !app.busy {
                app.set_status_visible(false);
            }
            match w & 0xffff {
                102 => if !app.busy { app.choose_zip() },
                104 => if !app.busy { app.choose_output() },
                112 => dialog(hwnd, ZIP_HELP),
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
                        app.start();
                    }
                }
                110 if !app.busy => {
                    app.skip_missing = !app.skip_missing;
                    InvalidateRect(app.controls[15], null(), 1);
                }
                111 if !app.busy => {
                    app.write_report = !app.write_report;
                    InvalidateRect(app.controls[11], null(), 1);
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
            if app.tooltip != 0 { DestroyWindow(app.tooltip); }
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
        InitCommonControls();
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
            button_procs: [0; 5],
            hover_button: None,
            tooltip: 0,
            help_text: wide(ZIP_HELP),
            status_visible: false,
            skip_missing: true,
            write_report: false,
            open_ready: false,
            rx: None,
            stop: Arc::new(AtomicBool::new(false)),
            busy: false,
            cancel_requested: false,
            progress_visible: false,
            progress_target: 0.0,
            progress_display: 0.0,
            last_output: PathBuf::new(),
            scale,
        });
        let ptr = Box::into_raw(app);
        let width = (790.0 * scale) as i32;
        let height = (IDLE_WINDOW_HEIGHT as f32 * scale) as i32;
        let x = ((GetSystemMetrics(0) - width) / 2).max(0);
        let y = ((GetSystemMetrics(1) - height) / 2).max(0);
        let hwnd = CreateWindowExW(
            0,
            name.as_ptr(),
            wide("Notion → Obsidian").as_ptr(),
            0x00cf0000 | 0x02000000, // WS_CLIPCHILDREN: do not paint over controls
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
const _: [(); 16] = [(); std::mem::size_of::<Guid>()];
const _: [(); 64] = [(); std::mem::size_of::<DrawItem>()];
const _: [(); 72] = [(); std::mem::size_of::<ToolInfo>()];
