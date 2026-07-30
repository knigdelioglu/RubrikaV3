import { readFile, readdir } from 'node:fs/promises';
import { extname, join, relative } from 'node:path';

const root = new URL('..', import.meta.url);
const sourceRoot = new URL('../src/', import.meta.url);
const allowedExtensions = new Set(['.ts', '.tsx']);
const violations = [];

async function walk(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      if (entry.name !== 'design-reference') await walk(path);
      continue;
    }
    if (!allowedExtensions.has(extname(entry.name))) continue;
    const source = await readFile(path, 'utf8');
    if (/\b(?:from\s*|import\s*\()?['"][^'"]*design-reference\//.test(source)) {
      violations.push(relative(root.pathname, path));
    }
  }
}

await walk(sourceRoot.pathname);

if (violations.length > 0) {
  console.error('Production files may not import src/design-reference/**:');
  for (const violation of violations) console.error(`- ${violation}`);
  process.exitCode = 1;
} else {
  console.log('Production boundary OK: src/design-reference is not imported.');
}
