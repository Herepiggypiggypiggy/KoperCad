// Render check: IEC and NFPA artwork side by side as a PNG, for the PR.
// usage: cargo run -p novo-symbols --example gallery -- iec.html iec
//        cargo run -p novo-symbols --example gallery -- nfpa.html nfpa
//        NODE_PATH=<global node_modules> node sheet.js iec.html nfpa.html passive/resistor,... out
// (writes out.html and out.png; needs playwright with Chromium)
const { chromium } = require('playwright');
const fs = require('fs');
const paths = process.argv[4].split(',');
const grab = (file, prefix) => {
  const html = fs.readFileSync(file, 'utf8');
  const out = {};
  for (const m of html.matchAll(/<figure>([\s\S]*?)<\/figure>/g)) {
    const id = /<code>([^<]*)<\/code>/.exec(m[1]);
    if (id) out[id[1]] = m[1];
  }
  return out;
};
(async () => {
  const iec = grab(process.argv[2]), nfpa = grab(process.argv[3]);
  let rows = '';
  for (const p of paths) {
    rows += `<tr><td>${iec['iec/' + p] || ''}</td><td>${nfpa['nfpa/' + p] || '(missing)'}</td></tr>`;
  }
  const page = `<!doctype html><meta charset=utf-8><style>body{font-family:sans-serif;margin:8px;background:#fff;color:#000}table{border-collapse:collapse}td{border:1px solid #ccc;padding:4px;vertical-align:top}svg{display:block;margin:auto}figcaption{font-size:11px;text-align:center}</style><table><tr><th>IEC 60617</th><th>NFPA 79 / ANSI Y32.2</th></tr>${rows}</table>`;
  fs.writeFileSync(process.argv[5] + '.html', page);
  const b = await chromium.launch();
  const pg = await b.newPage({ viewport: { width: 900, height: 600 } });
  await pg.goto('file://' + process.argv[5] + '.html');
  await pg.screenshot({ path: process.argv[5] + '.png', fullPage: true });
  await b.close();
})();
