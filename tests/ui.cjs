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
const version = '0.5.7';
assert.match(cargo, /version = "0\.5\.6"/);
assert.match(lock, /name = "notion-to-obsidian"\s+version = "0\.5\.6"/);
assert.match(rc, /FILEVERSION 0,5,6,0/);
assert.match(rc, /PRODUCTVERSION 0,5,6,0/);
assert.match(rc, /^1 ICON "assets\/app\.ico"/m);
assert.match(build, /rerun-if-changed=assets\/app\.ico/);
assert.match(core, /version: "0\.5\.6"/);
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
assert.match(gui, /hover_browse/);
assert.match(gui, /SetCursor\(LoadCursorW\(0, 32649usize/);
assert.match(gui, /v\(105\), input_w - v\(4\), v\(40\)/);
// GitHub Actions artifact is itself a ZIP. Its content must be unpacked files.
assert.match(workflow, /path: dist\/bundle\//);
assert.match(workflow, /path: extracted/);
assert.match(workflow, /package_release\.py --repack/);
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
