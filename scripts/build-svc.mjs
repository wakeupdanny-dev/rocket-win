// Builds the helper service and drops it next to the bundled sing-box so Tauri
// picks it up as a resource. Run from anywhere.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const svcDir = join(root, "rocket-service");
const out = join(svcDir, "target", "release", "rocket-svc.exe");
const dest = join(root, "src-tauri", "binaries", "rocket-svc.exe");

console.log("building rocket-svc …");
execFileSync("cargo", ["build", "--release"], { cwd: svcDir, stdio: "inherit" });

if (!existsSync(out)) throw new Error(`not found: ${out}`);
mkdirSync(dirname(dest), { recursive: true });
copyFileSync(out, dest);
console.log(`-> ${dest}`);
