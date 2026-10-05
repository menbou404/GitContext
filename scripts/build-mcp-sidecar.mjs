import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const tauri = path.join(root, "src-tauri");

try {
  const rustc = execFileSync("rustc", ["-vV"], { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
  const triple = rustc.match(/^host:\s*(\S+)\s*$/m)?.[1];
  if (!triple) throw new Error("rustc -vV did not report a host target triple.");
  execFileSync("cargo", ["build", "--release", "-p", "gitcontext-mcp", "--manifest-path", path.join(tauri, "Cargo.toml")], { cwd: root, stdio: "inherit" });
  const extension = triple.includes("windows") ? ".exe" : "";
  const source = path.join(tauri, "target", "release", `gitcontext-mcp${extension}`);
  const destination = path.join(tauri, "binaries", `gitcontext-mcp-${triple}${extension}`);
  mkdirSync(path.dirname(destination), { recursive: true });
  copyFileSync(source, destination);
  console.log(`MCP sidecar: ${destination}`);
} catch (error) {
  console.error(`Could not build MCP sidecar: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
