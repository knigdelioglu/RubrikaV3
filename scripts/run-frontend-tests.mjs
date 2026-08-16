import { readdir } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { join, resolve } from "node:path";

async function discoverTestFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const discovered = [];

  for (const entry of entries) {
    const entryPath = join(directory, entry.name);
    if (entry.isDirectory()) {
      discovered.push(...(await discoverTestFiles(entryPath)));
    } else if (entry.isFile() && entry.name.endsWith(".test.ts")) {
      discovered.push(entryPath);
    }
  }

  return discovered;
}

const testFiles = (await discoverTestFiles(resolve("src"))).sort();
if (testFiles.length === 0) {
  console.error("[frontend-tests] No *.test.ts files found under src/.");
  process.exit(1);
}

console.log(`[frontend-tests] discovered ${testFiles.length} test files`);

const result = spawnSync(
  process.execPath,
  ["--test", "--experimental-strip-types", ...testFiles],
  { stdio: "inherit" },
);

if (result.error) {
  console.error("[frontend-tests] failed to start Node test runner", result.error);
  process.exit(1);
}

process.exit(result.status ?? 1);
