'use strict';
// Static verification for the icon resource, version metadata and native theme.
// Does not replace compiling and testing the application on Windows.
const fs = require('node:fs');
const assert = require('node:assert/strict');
const cargo = fs.readFileSync('Cargo.toml', 'utf8');
const lock = fs.readFileSync('Cargo.lock', 'utf8');
const rc = fs.readFileSync('app.rc', 'utf8');
const build = fs.readFileSync('build.rs', 'utf8');
const gui = fs.readFileSync('src/gui.rs', 'utf8');
const workflow = fs.readFileSync('.github/workflows/windows-release.yml', 'utf8');
const packaging = fs.readFileSync('scripts/package_release.py', 'utf8');
const core = fs.readFileSync('src/lib.rs', 'utf8');
// Derive the release version from Cargo.toml so metadata checks never
// accidentally pin the previous release when Cargo's version is bumped.
const version = cargo.match(/^version = "(\d+\.\d+\.\d+)"$/m)?.[1];
assert.ok(version, 'Cargo.toml must contain a semantic version');
const versionPattern = version.replaceAll('.', '\\.');
const rcVersion = version.replaceAll('.', ',') + ',0';
assert.match(lock, new RegExp('name = "notion-to-obsidian"\\s+version = "' + versionPattern + '"'));
assert.match(rc, new RegExp('FILEVERSION ' + rcVersion));
assert.match(rc, new RegExp('PRODUCTVERSION ' + rcVersion));
assert.match(rc, /^1 ICON "assets\/app\.ico"/m);
assert.match(build, /rerun-if-changed=assets\/app\.ico/);
assert.match(core, new RegExp('version: "' + versionPattern + '"'));
assert.match(gui, /join\("Notion_Obsidian"\)/);
assert.match(gui, /wide\("Notion → Obsidian"\)\.as_ptr\(\)/);
assert.doesNotMatch(gui, /wide\("Notion → Obsidian ·/);
assert.match(gui, /"Конвертировать"/);
assert.doesNotMatch(gui, /"Проверить ZIP"/);
assert.doesNotMatch(gui, /"Локально · без установки"/);
assert.doesNotMatch(gui, /c\.inspect\(&archive\)/);
assert.match(gui, /"Создавать отчёт о переносе"/);
assert.match(gui, /const ZIP_HELP: &str =/);
assert.match(gui, /"Справка по ZIP"/);
assert.match(gui, /wide\("tooltips_class32"\)/);
assert.match(gui, /TTM_ADDTOOLW/);
assert.match(gui, /TTF_IDISHWND \| TTF_SUBCLASS/);
assert.match(gui, /TTM_SETMAXTIPWIDTH/);
assert.match(gui, /112 => dialog\(hwnd, ZIP_HELP\)/);
assert.match(gui, /status_visible: false/);
assert.match(gui, /self\.set_status_visible\(true\)/);
assert.match(gui, /app\.set_status_visible\(false\)/);
assert.match(gui, /if app\.status_visible \{/);
assert.match(gui, /ShowWindow\(self\.controls\[12\], visible as i32\)/);
assert.match(gui, /const IDLE_WINDOW_HEIGHT: i32 = 515/);
assert.match(gui, /const STATUS_WINDOW_HEIGHT: i32 = 615/);
assert.match(gui, /InitCommonControls\(\)/);
assert.doesNotMatch(gui, /"Выберите ZIP-экспорт Notion"/);
assert.match(gui, /write_report: false/);
assert.match(gui, /app\.write_report = !app\.write_report/);
assert.match(gui, /\.write_report\(write_report\)/);
assert.match(core, /pub fn write_report\(mut self, enabled: bool\)/);
assert.match(core, /if self\.write_report \{/);
assert.match(gui, /\(m, v\(385\), roww, v\(49\)\)/);
assert.match(gui, /\(m, v\(339\), roww, v\(32\)\)/);
assert.match(gui, /"Отменить"/);
assert.match(gui, /"Отменяем…"/);
assert.match(gui, /"Обработано страниц: "/);
assert.match(gui, /progress_target/);
assert.match(gui, /CreateCompatibleBitmap/);
assert.match(core, /Сохранено файлов:/);
assert.match(gui, /BS_OWNERDRAW/);
assert.match(gui, /FillRect\(item\.dc, &item\.rect,/);
assert.match(gui, /Пропускать отсутствующие страницы/);
assert.match(gui, /open_ready: false/);
assert.doesNotMatch(gui, /"Создать хранилище"/);
assert.match(gui, /WM_CTLCOLOREDIT/);
assert.match(gui, /DwmSetWindowAttribute/);
assert.match(gui, /LoadIconW\(instance, 1usize as \*const u16\)/);
assert.match(gui, /skip_missing: true/);
assert.match(gui, /"Профиль экспорта"/);
assert.match(gui, /"Настроить…"/);
assert.match(gui, /"Предпросмотр"/);
assert.match(gui, /\.options\(options\)/);
const options = fs.readFileSync('src/options.rs','utf8');
const editor = fs.readFileSync('src/settings.rs','utf8');
assert.match(options, /pub struct ConversionOptions/);
assert.match(options, /pub fn universal\(/);
assert.match(options, /pub fn knowledge\(/);
assert.match(options, /pub fn validate\(/);
assert.match(editor, /pub unsafe fn show\(/);
assert.match(editor, /GetSaveFileNameW/);
assert.match(core, /pub fn preview\(/);
assert.match(core, /self\.options\.validate\(\)\?/);
const settingsUi = fs.readFileSync('src/settings.rs', 'utf8');
// Both combo boxes are owner-drawn and their frame/arrow are painted in the
// same dark palette; settings action buttons are owner-drawn as well.
assert.match(gui, /0x3 \| 0x10 \| 0x200/);
assert.match(gui, /draw_combo_item\(item, app\.font, app\.scale\)/);
// Regression: ODT_COMBOBOX is 3; using ODT_LISTBOX (2) left the entire
// popup blank and removed the selected profile text in v0.6.1.
assert.match(gui, /item\.control_type == 3 && item\.control_id == 118/);
assert.match(gui, /measure\.control_type == 3 && measure\.control_id == 118/);
assert.doesNotMatch(gui, /item\.control_type == 2 && item\.control_id == 118/);
assert.match(gui, /0x133 \| 0x134 \| 0x138/);
assert.match(gui, /unsafe extern "system" fn dark_combo_proc/);
assert.match(gui, /fn EnumChildWindows\(hwnd: H, callback: Option<unsafe extern "system" fn\(H, isize\) -> i32>, param: isize\) -> i32/);
assert.match(gui, /fn GetClassNameW\(hwnd: H, class_name: \*mut u16, max: i32\) -> i32/);
// CBS_DROPDOWNLIST has an internal EDIT/STATIC display child. Theme it too
// and remove its default 3D border so it cannot leave a white inset frame.
assert.match(gui, /unsafe extern "system" fn theme_combo_child/);
assert.match(gui, /eq_ignore_ascii_case\("EDIT"\) \|\| class_name\.eq_ignore_ascii_case\("STATIC"\)/);
assert.match(gui, /style & !0x00800000/);
assert.match(gui, /SetWindowTheme\(hwnd, wide\(""\)\.as_ptr\(\), wide\(""\)\.as_ptr\(\)\)/);
assert.match(gui, /ex_style & !\(0x00000200 \| 0x00020000\)/);
assert.match(gui, /SetWindowPos\(hwnd, 0, 0, 0, 0, 0, 0x37\)/);
assert.match(gui, /EnumChildWindows\(hwnd, Some\(theme_combo_child\), 0\)/);
assert.match(gui, /unsafe fn is_combo_descendant\(hwnd: H, combo: H\) -> bool/);
assert.match(gui, /is_combo_descendant\(l as H, combo\)/);
// Both windows use one shared setup: disable UxTheme on the ComboBox itself,
// strip child edges, then subclass and paint the whole surface with GDI.
assert.match(gui, /unsafe fn apply_dark_combo\(hwnd: H\)/);
assert.match(gui, /SetWindowTheme\(hwnd, no_theme\.as_ptr\(\), no_theme\.as_ptr\(\)\)/);
assert.match(gui, /theme_combo_children\(hwnd\)/);
assert.match(gui, /SetWindowLongPtrW\(hwnd, -4, dark_combo_proc as isize\)/);
assert.match(gui, /apply_dark_combo\(self\.controls\[18\]\)/);
assert.match(settingsUi, /apply_dark_combo\(h\)/);
assert.doesNotMatch(gui, /SetWindowTheme\(self\.controls\[18\], wide\("DarkMode_Explorer"/);
assert.doesNotMatch(settingsUi, /if class == "COMBOBOX"\s*\{[\s\S]{0,220}DarkMode_Explorer/);
assert.match(settingsUi, /is_combo_descendant\(l as H, combo\)/);
assert.match(gui, /draw_combo_chrome\(hwnd\)/);
// ComboBox paints a separate inset frame around its selected-value area.
// Repaint that client-area boundary (plus the outer edges) after WM_PAINT;
// a window DC / WM_NCPAINT overlay cannot reach this inner frame.
assert.match(gui, /unsafe fn draw_combo_frame\(dc: H, rect: &Rect, divider: i32, color: u32, thickness: i32\)/);
assert.match(gui, /Rect \{ left: 0, top: 0, right: rect\.right, bottom: t \}/);
assert.match(gui, /Rect \{ left: 0, top: rect\.bottom - t, right: rect\.right, bottom: rect\.bottom \}/);
assert.match(gui, /Rect \{ left: 0, top: 0, right: t, bottom: rect\.bottom \}/);
assert.match(gui, /Rect \{ left: rect\.right - t, top: 0, right: rect\.right, bottom: rect\.bottom \}/);
assert.match(gui, /Rect \{ left: t, top: t, right: split, bottom: \(2 \* t\)\.min\(rect\.bottom - t\) \}/);
assert.match(gui, /Rect \{ left: t, top: rect\.bottom - 2 \* t, right: split, bottom: rect\.bottom - t \}/);
assert.match(gui, /Rect \{ left: t, top: t, right: 2 \* t, bottom: rect\.bottom - t \}/);
assert.match(gui, /Rect \{ left: split, top: t, right: \(split \+ t\)\.min\(rect\.right\), bottom: rect\.bottom - t \}/);
assert.match(gui, /draw_combo_frame\(dc, &rect, arrow_left, border, thickness\)/);
assert.match(gui, /0x000F => draw_combo_chrome\(hwnd\)/);
assert.doesNotMatch(gui, /GetWindowDC|draw_combo_nonclient_frame/);
assert.match(gui, /GetDpiForWindow/);
assert.match(gui, /0x2c => \{ \/\/ WM_MEASUREITEM/);
assert.match(settingsUi, /0x00010000 \| 0xB/);
assert.match(settingsUi, /draw_settings_button/);
assert.match(settingsUi, /draw_combo_item\(item, state\.font, state\.scale\)/);
assert.match(settingsUi, /item\.control_type == 3 && matches!\(item\.control_id, 600\.\.=609\)/);
assert.match(settingsUi, /measure\.control_type == 3 && matches!\(measure\.control_id, 600\.\.=609\)/);
assert.doesNotMatch(settingsUi, /item\.control_type == 2 && matches!\(item\.control_id, 600\.\.=609\)/);
assert.match(settingsUi, /0x0133 \| 0x0135 \| 0x0138/);
// Themed auto-check buttons ignore SetTextColor. Keep the native checkbox
// for keyboard and accessibility; give it a dark STATIC text label instead.
assert.match(settingsUi, /self\.add\("BUTTON", text, 0x00010000 \| 0x3, id, x, y, 23, 27\)/);
assert.match(settingsUi, /self\.add\("STATIC", text, 0x100 \| 0x200, id \+ 100/);
assert.match(settingsUi, /710\.\.=713 =>/);
assert.match(settingsUi, /SetBkColor\(dc, if edit \|\| list \{FIELD\} else \{BG\}\)/);
assert.match(settingsUi, /SetTextColor\(dc, TEXT\)/);
assert.match(settingsUi, /0x002C => \{ \/\/ WM_MEASUREITEM/);

// Regressions reported in 0.5.5: redraw only the progress strip,
// keep folder icons visible on focus, and provide native hover feedback.
assert.match(gui, /WS_CLIPCHILDREN/);
assert.match(gui, /self\.invalidate_progress\(\)/);
assert.match(gui, /TrackMouseEvent\(&mut track\)/);
assert.match(gui, /CallWindowProcW/);
assert.match(gui, /hover_button/);
assert.match(gui, /SetCursor\(LoadCursorW\(0, 32649usize/);
assert.match(gui, /CoCreateInstance/);
assert.match(gui, /CLSID_FILE_OPEN_DIALOG/);
assert.match(gui, /IID_IFILE_DIALOG/);
assert.match(gui, /FOS_PICKFOLDERS/);
assert.match(gui, /SIGDN_FILESYSPATH/);
assert.match(gui, /0x80058000/);
assert.match(gui, /methods\.get_result/);
assert.match(gui, /CoTaskMemFree\(path as \*mut c_void\)/);
assert.doesNotMatch(gui, /fn SHBrowseForFolderW/);
assert.match(gui, /button_proc/);
assert.match(gui, /IsWindowEnabled/);
assert.match(gui, /wide\("Нажмите «Обзор» или перетащите ZIP"\)/);
assert.match(gui, /caption = wide\("Обзор"\)/);
assert.doesNotMatch(gui, /"Результат будет создан в новой папке/);
// A full-height native one-line EDIT top-aligns glyphs: keep a smaller
// control centered vertically within the independently painted rounded field.
assert.match(gui, /const FIELD_TOPS: \[i32; 2\] = \[103, 183\]/);
assert.match(gui, /const FIELD_HEIGHT: i32 = 44/);
assert.match(gui, /const EDIT_HEIGHT: i32 = 24/);
assert.match(gui, /let centered_y = \|top: i32, child_height: i32\|/);
assert.match(gui, /\(edit_x, centered_y\(zip_top, edit_h\), edit_w, edit_h\)/);
assert.match(gui, /\(edit_x, centered_y\(output_top, edit_h\), edit_w, edit_h\)/);
assert.match(gui, /\(browse_x, centered_y\(zip_top, browse_h\), browse_w, browse_h\)/);
assert.match(gui, /\(browse_x, centered_y\(output_top, browse_h\), browse_w, browse_h\)/);
assert.doesNotMatch(gui, /v\(105\), input_w/);
// Simulate layout rounding at common Windows DPI settings. Both EDIT controls
// and both browse buttons must share the field's center to within 0.5 px.
for (const scale of [1, 1.25, 1.5, 2]) {
  const v = n => Math.round(n * scale);
  for (const top of [103, 183]) {
    for (const childHeight of [24, 34]) {
      const fieldY = v(top);
      const h = v(childHeight);
      const childY = fieldY + Math.floor((v(44) - h) / 2);
      assert.ok(Math.abs(childY + h / 2 - (fieldY + v(44) / 2)) <= 0.5,
        'Control vertical center must match painted field at DPI scale ' + scale);
    }
  }
}
// The tested ZIP is staged as a draft release, bypassing Actions artifact quota.
assert.match(workflow, /gh release create/);
assert.match(workflow, /--draft/);
assert.match(workflow, /releases\/assets/);
assert.match(workflow, /Expand-Archive/);
assert.match(workflow, /--method PATCH/);
assert.doesNotMatch(workflow, /actions\/(?:upload|download)-artifact@/);
assert.match(packaging, /Leave WORK intact/);
assert.match(packaging, /Nested archive detected/);
const ico = fs.readFileSync('assets/app.ico');
assert.equal(ico.readUInt16LE(0), 0);
assert.equal(ico.readUInt16LE(2), 1);
const count = ico.readUInt16LE(4);
assert.ok(count >= 4);
const sizes = [];
for (let i = 0; i < count; i++) {
  const off = 6 + 16 * i;
  const w = ico[off] || 256;
  const h = ico[off + 1] || 256;
  const length = ico.readUInt32LE(off + 8);
  const position = ico.readUInt32LE(off + 12);
  assert.equal(w, h);
  assert.equal(ico.readUInt16LE(off + 6), 32);
  assert.ok(length > 40 && position + length <= ico.length);
  assert.equal(ico.readUInt32LE(position), 40); // BITMAPINFOHEADER
  assert.equal(ico.readInt32LE(position + 4), w);
  assert.equal(ico.readInt32LE(position + 8), h * 2);
  sizes.push(w);
}
for (const s of [16, 32, 48, 64, 128, 256]) assert.ok(sizes.includes(s));
console.log('UI RESOURCE PASS:', version, 'ICO sizes', sizes.join(', '));
