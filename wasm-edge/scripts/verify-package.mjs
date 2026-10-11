#!/usr/bin/env node
// Packaging guard for the `recached-edge` npm package.
//
// Every published version from 0.1.3 to 0.3.0 was impossible to import: the
// wasm-bindgen glue starts with
//
//   import { openRecachedDb, ... } from './snippets/<crate-hash>/inline0.js';
//
// and `snippets/` was in neither the published tarball nor the `files` array
// wasm-pack generates. `npm install recached-edge` then failed at module
// resolution before any application code ran. Nothing caught it because the
// package was never installed from a tarball — only ever used from the working
// tree, where `snippets/` is right there on disk.
//
// The second trap is subtler: wasm-pack writes `pkg/.gitignore` containing `*`,
// and npm honours a nested .gitignore even for a path listed in `files`. So
// publishing the SDK directory with `"files": ["pkg/"]` silently ships an SDK
// with no WebAssembly in it at all.
//
// Both are invisible to typecheck, unit tests and `npm run build`. Only packing
// a tarball and importing it from outside the tree finds them, which is what
// this script does.
//
//   node scripts/verify-package.mjs --pre   # working tree, runs from prepack
//   node scripts/verify-package.mjs         # pack a real tarball and import it
//
// Exits non-zero with a specific reason on the first failure.

import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const PKG_ROOT = path.resolve(import.meta.dirname, '..');
const PRE = process.argv.includes('--pre');

const problems = [];
const fail = (msg) => problems.push(msg);

/** The `./pkg/<name>.js` specifier sdk.js loads the wasm-bindgen glue from. */
function glueSpecifier(sdkSource) {
  const m = sdkSource.match(/import\(\s*['"](\.\/pkg\/[^'"]+)['"]\s*\)/);
  return m ? m[1] : null;
}

/** Every relative specifier a module imports at the top level. */
function staticImports(source) {
  return [...source.matchAll(/from\s*['"](\.[^'"]+)['"]/g)].map((m) => m[1]);
}

/**
 * Resolve the import graph starting at sdk.js and report anything missing.
 * `root` is a directory laid out like the published package.
 */
function checkTree(root, label) {
  const sdkPath = path.join(root, 'sdk.js');
  if (!existsSync(sdkPath)) {
    fail(`${label}: sdk.js is missing — the SDK entrypoint is what package.json "main" points at. Run \`npx tsc\`.`);
    return null;
  }

  const spec = glueSpecifier(readFileSync(sdkPath, 'utf8'));
  if (!spec) {
    fail(`${label}: sdk.js has no dynamic import of ./pkg/* — cannot locate the wasm glue to verify.`);
    return null;
  }

  const gluePath = path.join(root, spec);
  if (!existsSync(gluePath)) {
    const built = existsSync(path.join(root, 'pkg'))
      ? readdirSync(path.join(root, 'pkg')).filter((f) => f.endsWith('.js')).join(', ') || 'none'
      : 'no pkg/ directory at all';
    fail(
      `${label}: sdk.js imports "${spec}" but that file does not exist (pkg/ holds: ${built}). ` +
        `Build with \`--out-name recached_edge\` so the emitted name matches the import.`,
    );
    return null;
  }

  const glueSource = readFileSync(gluePath, 'utf8');
  for (const dep of staticImports(glueSource)) {
    const depPath = path.resolve(path.dirname(gluePath), dep);
    if (!existsSync(depPath)) {
      fail(
        `${label}: the wasm glue imports "${dep}" but it is not present. ` +
          `This is the 0.1.3–0.3.0 bug: wasm-pack's snippets/ directory must ship with the package.`,
      );
    }
  }

  const wasm = readdirSync(path.join(root, 'pkg')).filter((f) => f.endsWith('.wasm'));
  if (wasm.length === 0) fail(`${label}: pkg/ contains no .wasm file.`);

  return { sdkPath, gluePath };
}

/**
 * Call the public `Cache` API against the packaged wasm.
 *
 * Importing proves the files resolve; it does not prove a single method works.
 * `incr`/`decr` shipped broken for several releases because Rust's `i64` makes
 * wasm-bindgen emit `bigint` while the SDK passed a `number` — a `TypeError` on
 * every call, in every runtime, that no test could see: the Rust-side browser
 * tests never cross the JS boundary, and `sdk.js` has no unit tests.
 *
 * `initSync` takes the wasm bytes directly, so this needs no fetchable URL and
 * no browser. Persistence and pub/sub are skipped — they need IndexedDB and a
 * socket. Everything here is local-only, the mode with no external dependency.
 */
async function checkBehaviour(glue, sdk, pkgDir) {
  const wasm = readdirSync(pkgDir).find((f) => f.endsWith('.wasm'));
  let cache;
  try {
    glue.initSync({ module: readFileSync(path.join(pkgDir, wasm)) });
    cache = new sdk.Cache(new glue.RecachedCache());
  } catch (e) {
    fail(`tarball: could not instantiate the wasm — ${e.message.split('\n')[0]}`);
    return;
  }

  const check = (label, fn, expected) => {
    let actual;
    try {
      actual = fn();
    } catch (e) {
      fail(`tarball: ${label} threw ${e.constructor.name}: ${e.message.split('\n')[0]}`);
      return;
    }
    const got = JSON.stringify(actual);
    if (got !== JSON.stringify(expected)) {
      fail(`tarball: ${label} returned ${got}, expected ${JSON.stringify(expected)}`);
    }
  };

  // Change routing through the real wasm callback: the keys the engine reports
  // must reach the listeners of those keys and patterns, and no others.
  const seen = { all: [], key: 0, pattern: 0, other: 0 };
  const stops = [
    cache.onMutation((keys) => seen.all.push(...keys)),
    cache.onKeyChange('route:a', () => seen.key++),
    cache.onPatternChange('route:*', () => seen.pattern++),
    cache.onKeyChange('elsewhere', () => seen.other++),
  ];
  cache.set('route:a', '1');
  cache.incr('route:n');
  check('mutation keys', () => seen.all, ['route:a', 'route:n']);
  check('key listener', () => seen.key, 1);
  check('pattern listener', () => seen.pattern, 2);
  check('unrelated key listener', () => seen.other, 0);
  for (const stop of stops) stop();

  cache.set('k', 'v');
  check('get after set', () => cache.get('k'), 'v');
  check('exists', () => cache.exists('k'), true);
  check('get on a missing key', () => cache.get('nope'), null);
  check('ttl on a missing key', () => cache.ttl('nope'), -2);

  cache.setJSON('u', { id: 42 });
  check('getJSON round-trip', () => cache.getJSON('u').id, 42);

  cache.setEx('s', 'tok', 60);
  check('ttl after setEx', () => cache.ttl('s'), 60);

  // The regression that motivated this function.
  check('incr', () => cache.incr('n'), 1);
  check('incr by 5', () => cache.incr('n', 5), 6);
  check('decr by 2', () => cache.decr('n', 2), 4);
  check('incr returns a number', () => typeof cache.incr('n'), 'number');

  cache.jset('d', '$', { title: 'a', draft: true });
  cache.jmerge('d', { title: 'b', draft: null });
  check('jget after jmerge', () => cache.jget('d'), { title: 'b' });

  cache.set('p:1', 'x');
  check('getMatching', () => cache.getMatching('p:*'), [['p:1', 'x']]);

  cache.setBytes('b', new Uint8Array([1, 2, 3]));
  check('getBytes round-trip', () => Array.from(cache.getBytes('b')), [1, 2, 3]);

  check('del', () => cache.del('k'), true);
  check('get after del', () => cache.get('k'), null);
}

// ── working-tree checks (also run from prepack) ───────────────────────────────

if (existsSync(path.join(PKG_ROOT, 'pkg', '.gitignore'))) {
  fail(
    'pkg/.gitignore exists. wasm-pack writes it containing "*", and npm applies a nested ' +
      '.gitignore even to a directory listed in "files" — packing now would drop the whole ' +
      'pkg/ tree. `npm run build:wasm` removes it; delete it before publishing.',
  );
}

checkTree(PKG_ROOT, 'working tree');

// ── tarball checks ────────────────────────────────────────────────────────────
// The working tree can be complete while the tarball is not: `files`, nested
// .gitignore and .npmignore all decide what actually ships. So pack for real
// and import the result the way a consumer would.

let tmp;
if (!PRE && problems.length === 0) {
  tmp = mkdtempSync(path.join(tmpdir(), 'recached-pack-'));
  try {
    const out = execFileSync('npm', ['pack', '--json', '--pack-destination', tmp], {
      cwd: PKG_ROOT,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'inherit'],
    });
    // `npm pack` runs prepack, whose output lands on this stdout too, so the
    // JSON is not necessarily the whole of it.
    const json = out.slice(out.indexOf('['));
    const tarball = path.join(tmp, JSON.parse(json)[0].filename);
    execFileSync('tar', ['-xzf', tarball, '-C', tmp]);
    const extracted = path.join(tmp, 'package');

    const found = checkTree(extracted, 'tarball');

    if (found) {
      // Resolution is the thing that broke first, so import both modules for real.
      let glue;
      try {
        glue = await import(pathToFileURL(found.gluePath).href);
      } catch (e) {
        fail(`tarball: importing the wasm glue failed — ${e.code ?? ''} ${e.message.split('\n')[0]}`);
      }

      let sdk;
      try {
        sdk = await import(pathToFileURL(found.sdkPath).href);
        for (const name of ['createCache', 'init', 'Cache']) {
          if (!(name in sdk)) {
            fail(
              `tarball: the package does not export \`${name}\`. Every doc example imports it; ` +
                `0.1.0–0.3.0 published wasm-pack's pkg/ directory, which exports only RecachedCache.`,
            );
          }
        }
      } catch (e) {
        fail(`tarball: importing sdk.js failed — ${e.code ?? ''} ${e.message.split('\n')[0]}`);
      }

      if (glue && sdk) await checkBehaviour(glue, sdk, path.dirname(found.gluePath));
    }
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

// ── report ────────────────────────────────────────────────────────────────────

if (problems.length > 0) {
  console.error(`\nrecached-edge package verification failed (${problems.length}):\n`);
  for (const p of problems) console.error(`  ✗ ${p}\n`);
  process.exit(1);
}

// stderr in --pre: it runs inside `npm pack`, and anything on stdout there
// corrupts `npm pack --json` for whatever is parsing it.
const report = PRE ? console.error : console.log;
report(
  PRE
    ? 'recached-edge: working tree is publishable (glue + snippets resolve, no pkg/.gitignore).'
    : 'recached-edge: tarball verified — snippets ship, glue resolves, createCache is exported.',
);
