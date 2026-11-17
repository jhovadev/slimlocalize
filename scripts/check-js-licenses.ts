import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const root = process.cwd();
const allowed = new Set([
  "MIT",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "ISC",
  "Zlib",
  "Unlicense",
  "CC0-1.0",
  "CC-BY-4.0",
  "0BSD",
  "BlueOak-1.0.0",
]);

const forbidden = ["GPL", "AGPL", "LGPL", "SSPL", "BUSL", "Commons Clause"];

function collectPackageJsonFiles(directory: string): string[] {
  const result: string[] = [];
  for (const entry of readdirSync(directory)) {
    if (entry === "node_modules" || entry === ".git") continue;
    const path = join(directory, entry);
    if (statSync(path).isDirectory())
      result.push(...collectPackageJsonFiles(path));
    else if (entry === "package.json") result.push(path);
  }
  return result;
}

const files = collectPackageJsonFiles(root);
let failures = 0;
for (const file of files) {
  const pkg = JSON.parse(readFileSync(file, "utf8")) as {
    license?: unknown;
    name?: string;
  };
  const license = typeof pkg.license === "string" ? pkg.license : "";
  if (!license) continue;
  if (forbidden.some((item) => license.includes(item))) {
    console.error(`Forbidden license in ${file}: ${license}`);
    failures += 1;
  } else if (!allowed.has(license)) {
    console.warn(
      `Unreviewed license in ${file}: ${license} (${pkg.name ?? "unknown"})`,
    );
  }
}
if (failures > 0) process.exit(1);
console.log(`Checked ${files.length} package manifests.`);
