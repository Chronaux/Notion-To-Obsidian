// Validate generated Bases against the actual exported records. No Obsidian runtime required.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const root = process.argv[2];
if (!root) throw new Error('Usage: node tests/bases.cjs RESULT_FOLDER');
const report = JSON.parse(fs.readFileSync(path.join(root, '.notion-to-obsidian', 'migration-report.json')));
const vault = root;
const expressions = new Map();
function evaluate(filter, file) {
  if (!filter) return true;
  if (typeof filter === 'string') {
    if (!expressions.has(filter)) expressions.set(filter, vm.runInNewContext('(file) => ('+filter+')'));
    return !!expressions.get(filter)(file);
  }
  if (filter.and) return filter.and.every(item => evaluate(item, file));
  if (filter.or) return filter.or.some(item => evaluate(item, file));
  if (filter.not) return !evaluate(filter.not, file);
  throw new Error('Unsupported filter: ' + JSON.stringify(filter));
}
let checks = 0;
for (const prefix of ['', 'Vault/', 'Копия/Vault/']) {
  const files = Object.values(report.mapping).map(name => ({path: prefix + name, ext:'md'}));
  files.push({path:prefix+'Сторонняя заметка.md', ext:'md'});
  for (const db of report.databases) {
    const base = JSON.parse(fs.readFileSync(path.join(vault, db.base)));
    const all = files.filter(file => evaluate(base.filters, file));
    const view = all.filter(file => evaluate(base.views[0].filters, file));
    assert.equal(all.length, db.rows, prefix + db.title + ': all rows');
    assert.equal(view.length, db.view_rows, prefix + db.title + ': current view');
    assert.ok(!base.views[1].filters, 'All-records view must not apply current-view restriction');
    assert.ok(base.properties['formula.Название'].displayName in db.columns);
    // Check the rank list against the exported membership without executing the Obsidian DSL.
    const rankPaths = JSON.parse(base.formulas['Порядок'].split('.reduce(')[0]);
    assert.equal(rankPaths.length, all.length);
    assert.ok(all.every(file => rankPaths.some(p => file.path === p || file.path.endsWith('/'+p))));
    checks++;
  }
}
console.log(`BASES PASS: ${checks} database/path combinations; all/current row counts match the export`);
