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
assert.match(gui, /"Результат будет создан в новой папке/);
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
