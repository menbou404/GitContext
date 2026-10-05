//! Explicit, reviewable registration of the MCP server with local AI clients.
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use toml_edit::{value, Array, DocumentMut, Item, Table};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Client {
    ClaudeCode,
    Codex,
    ClaudeDesktop,
}
impl Client {
    fn slug(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
            Self::ClaudeDesktop => "claude-desktop",
        }
    }
    fn confirmation(self) -> &'static str {
        match self {
            Self::ClaudeCode => "mixed",
            Self::Codex => "unstable",
            Self::ClaudeDesktop => "unsupported",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Read,
    Local,
    Remote,
}
impl Tier {
    fn arg(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Connect,
    Change,
    Repair,
    Disconnect,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub path: PathBuf,
    pub version: Option<String>,
    pub development: bool,
    pub built: bool,
    pub registration_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub client: Client,
    pub state: String,
    pub tier: Tier,
    pub trust: bool,
    pub confirmation: String,
    pub config_path: PathBuf,
    pub registration_name: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub cli_available: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub server: ServerInfo,
    pub clients: Vec<ClientInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub client: Client,
    pub action: Action,
    pub tier: Tier,
    pub trust: bool,
    pub config_path: PathBuf,
    pub before: String,
    pub after: String,
    pub command_line: Option<String>,
    pub file_hash: Option<String>,
    pub registration_name: String,
    pub manual: bool,
}

#[derive(Clone, Debug)]
pub struct ClientPaths {
    pub claude_code: PathBuf,
    pub codex: PathBuf,
    pub claude_desktop: PathBuf,
    pub claude_cli: Option<PathBuf>,
    pub server: ServerInfo,
}

pub fn resolve_server(gui_exe: &Path, development: bool) -> ServerInfo {
    let parent = gui_exe.parent().unwrap_or(Path::new("."));
    let file = if cfg!(windows) {
        "gitcontext-mcp.exe"
    } else {
        "gitcontext-mcp"
    };
    let path = if development {
        parent
            .ancestors()
            .find(|p| p.join("Cargo.toml").exists() && p.join("crates").exists())
            .unwrap_or(parent)
            .join("target")
            .join("debug")
            .join(file)
    } else {
        parent.join(file)
    };
    let built = path.is_file();
    let version = if built {
        Command::new(&path).arg("--version").output().ok().map(|o| {
            let output = if o.stderr.is_empty() {
                &o.stdout
            } else {
                &o.stderr
            };
            String::from_utf8_lossy(output).trim().to_owned()
        })
    } else {
        None
    };
    ServerInfo {
        path,
        version,
        development,
        built,
        registration_name: if development {
            "gitcontext-dev"
        } else {
            "gitcontext"
        }
        .into(),
    }
}

pub fn default_paths() -> Result<ClientPaths, String> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or("Home directory not found")?;
    let home = PathBuf::from(home);
    let appdata = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("AppData/Roaming"));
    let local = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("AppData/Local"));
    let desktop = desktop_config_path(&local, &appdata);
    let cli = find_on_path("claude").or_else(|| {
        let path = home.join(".local/bin/claude.exe");
        path.is_file().then_some(path)
    });
    let gui = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(ClientPaths {
        claude_code: home.join(".claude.json"),
        codex: home.join(".codex/config.toml"),
        claude_desktop: desktop,
        claude_cli: cli,
        server: resolve_server(&gui, cfg!(debug_assertions)),
    })
}

fn desktop_config_path(local: &Path, appdata: &Path) -> PathBuf {
    let fallback = appdata.join("Claude/claude_desktop_config.json");
    let package_root = local.join("Packages");
    let packaged = fs::read_dir(package_root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("Claude_"))
        })
        .map(|path| path.join("LocalCache/Roaming/Claude/claude_desktop_config.json"))
        .find(|path| path.is_file());
    packaged.unwrap_or(fallback)
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let names = if cfg!(windows) {
        vec![format!("{name}.exe"), format!("{name}.cmd")]
    } else {
        vec![name.to_owned()]
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|p| p.is_file())
}

fn config_path(paths: &ClientPaths, client: Client) -> &Path {
    match client {
        Client::ClaudeCode => &paths.claude_code,
        Client::Codex => &paths.codex,
        Client::ClaudeDesktop => &paths.claude_desktop,
    }
}
fn contents(path: &Path) -> Result<Option<String>, String> {
    if path.exists() {
        fs::read_to_string(path)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display()))
    } else {
        Ok(None)
    }
}
fn content_hash(text: Option<&str>) -> Option<String> {
    text.map(|text| {
        let mut hash = DefaultHasher::new();
        text.hash(&mut hash);
        format!("{:016x}", hash.finish())
    })
}
fn parse_json(text: Option<&str>) -> Result<Value, String> {
    text.map_or_else(
        || Ok(json!({})),
        |s| serde_json::from_str(s).map_err(|e| e.to_string()),
    )
}
fn parse_toml(text: Option<&str>) -> Result<DocumentMut, String> {
    text.unwrap_or("")
        .parse::<DocumentMut>()
        .map_err(|e| e.to_string())
}
fn entry(
    paths: &ClientPaths,
    client: Client,
    text: Option<&str>,
) -> Result<Option<(String, Vec<String>, String)>, String> {
    let name = &paths.server.registration_name;
    if client == Client::Codex {
        let doc = parse_toml(text)?;
        let Some(servers) = doc.get("mcp_servers") else {
            return Ok(None);
        };
        let servers = servers.as_table().ok_or("mcp_servers must be a table")?;
        let Some(item) = servers.get(name) else {
            return Ok(None);
        };
        if !item.is_table() {
            return Err(format!("mcp_servers.{name} must be a table"));
        }
        let command = item["command"].as_str().unwrap_or("").to_owned();
        let args = item["args"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Some((
            command,
            args,
            format!(
                "[mcp_servers.{name}]
{}",
                item.to_string().trim_start()
            ),
        )))
    } else {
        let data = parse_json(text)?;
        let Some(item) = data.get("mcpServers").and_then(|v| v.get(name)) else {
            return Ok(None);
        };
        let command = item
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let args = item
            .get("args")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Some((
            command,
            args,
            serde_json::to_string_pretty(item).map_err(|e| e.to_string())?,
        )))
    }
}
fn tier_from_args(args: &[String]) -> Tier {
    args.windows(2)
        .find(|w| w[0] == "--max-tier")
        .map(|w| match w[1].as_str() {
            "remote" => Tier::Remote,
            "local" => Tier::Local,
            _ => Tier::Read,
        })
        .unwrap_or(Tier::Read)
}
fn same_exe(a: &str, b: &Path) -> bool {
    if cfg!(windows) {
        a.replace('/', "\\")
            .eq_ignore_ascii_case(&b.to_string_lossy().replace('/', "\\"))
    } else {
        a == b.to_string_lossy()
    }
}

pub fn list(paths: &ClientPaths) -> Result<Inventory, String> {
    let mut clients = Vec::new();
    for client in [Client::ClaudeCode, Client::Codex, Client::ClaudeDesktop] {
        let path = config_path(paths, client);
        let text = contents(path)?;
        let registered = entry(paths, client, text.as_deref())?;
        let installed = text.is_some()
            || match client {
                Client::ClaudeCode => paths.claude_cli.is_some(),
                Client::Codex => find_on_path("codex").is_some(),
                Client::ClaudeDesktop => false,
            };
        let (state, command, args) = match registered {
            Some((command, args, _)) => {
                let state = if paths.server.built
                    && Path::new(&command).is_file()
                    && same_exe(&command, &paths.server.path)
                {
                    "connected"
                } else {
                    "repair"
                };
                (state, Some(command), args)
            }
            None => (
                if installed {
                    "disconnected"
                } else {
                    "not_found"
                },
                None,
                Vec::new(),
            ),
        };
        clients.push(ClientInfo {
            client,
            state: state.into(),
            tier: tier_from_args(&args),
            trust: args.iter().any(|a| a == "--trust-client-approval"),
            confirmation: client.confirmation().into(),
            config_path: path.to_path_buf(),
            registration_name: paths.server.registration_name.clone(),
            command,
            args,
            cli_available: paths.claude_cli.is_some(),
        });
    }
    Ok(Inventory {
        server: paths.server.clone(),
        clients,
    })
}

fn args_for(tier: Tier, trust: bool) -> Vec<String> {
    let mut args = vec!["--max-tier".into(), tier.arg().into()];
    if trust {
        args.push("--trust-client-approval".into());
    }
    args
}
fn changed_text(
    paths: &ClientPaths,
    client: Client,
    source: Option<&str>,
    action: Action,
    tier: Tier,
    trust: bool,
) -> Result<String, String> {
    let name = &paths.server.registration_name;
    if client == Client::Codex {
        let mut doc = parse_toml(source)?;
        if doc.get("mcp_servers").is_some_and(|item| !item.is_table()) {
            return Err("mcp_servers must be a table".into());
        }
        if action == Action::Disconnect {
            if let Some(table) = doc.get_mut("mcp_servers").and_then(Item::as_table_mut) {
                table.remove(name);
            }
        } else {
            if doc.get("mcp_servers").is_none() {
                doc["mcp_servers"] = Item::Table(Table::new());
            }
            if doc["mcp_servers"]
                .as_table()
                .is_some_and(|table| table.get(name).is_some_and(|item| !item.is_table()))
            {
                return Err(format!("mcp_servers.{name} must be a table"));
            }
            if doc["mcp_servers"]
                .as_table()
                .is_some_and(|table| table.get(name).is_none())
            {
                doc["mcp_servers"][name] = Item::Table(Table::new());
            }
            doc["mcp_servers"][name]["command"] =
                value(paths.server.path.to_string_lossy().as_ref());
            let mut args = Array::new();
            for arg in args_for(tier, trust) {
                args.push(arg);
            }
            doc["mcp_servers"][name]["args"] = value(args);
        }
        Ok(doc.to_string())
    } else {
        let mut data = parse_json(source)?;
        if !data.is_object() {
            return Err("Root JSON value must be an object".into());
        }
        if action == Action::Disconnect {
            if let Some(servers) = data.get_mut("mcpServers").and_then(Value::as_object_mut) {
                servers.remove(name);
            }
        } else {
            if data.get("mcpServers").is_none() {
                data["mcpServers"] = json!({});
            }
            let servers = data["mcpServers"]
                .as_object_mut()
                .ok_or("mcpServers must be an object")?;
            servers.insert(name.clone(), json!({"command": paths.server.path.to_string_lossy(), "args": args_for(tier, trust)}));
        }
        let mut result = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"  ");
        let mut serializer = serde_json::Serializer::with_formatter(&mut result, formatter);
        data.serialize(&mut serializer).map_err(|e| e.to_string())?;
        result.push(b'\n');
        String::from_utf8(result).map_err(|e| e.to_string())
    }
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}
fn cli_command(paths: &ClientPaths, action: Action, tier: Tier, trust: bool) -> String {
    let name = &paths.server.registration_name;
    if action == Action::Disconnect {
        format!("claude mcp remove --scope user {name}")
    } else {
        let add = format!(
            "claude mcp add --scope user {name} -- {} {}",
            quote(&paths.server.path.to_string_lossy()),
            args_for(tier, trust).join(" ")
        );
        if matches!(action, Action::Change | Action::Repair) {
            format!("claude mcp remove --scope user {name}\n{add}")
        } else {
            add
        }
    }
}

pub fn plan(
    paths: &ClientPaths,
    client: Client,
    action: Action,
    tier: Tier,
    trust: bool,
) -> Result<Plan, String> {
    if trust
        && (tier != Tier::Remote
            || !matches!(
                client,
                Client::ClaudeCode | Client::Codex | Client::ClaudeDesktop
            ))
    {
        return Err(
            "Trust is only available for remote access on unsupported or unstable clients".into(),
        );
    }
    if action != Action::Disconnect && !paths.server.built {
        return Err("MCP server is not built. Run cargo build -p gitcontext-mcp.".into());
    }
    let path = config_path(paths, client);
    let source = contents(path)?;
    let before = entry(paths, client, source.as_deref())?
        .map(|v| v.2)
        .unwrap_or_default();
    let after = if client == Client::ClaudeCode {
        if action == Action::Disconnect {
            String::new()
        } else {
            serde_json::to_string_pretty(
                &json!({"command": paths.server.path, "args": args_for(tier, trust)}),
            )
            .map_err(|e| e.to_string())?
        }
    } else {
        let changed = changed_text(paths, client, source.as_deref(), action, tier, trust)?;
        entry(paths, client, Some(&changed))?
            .map(|v| v.2)
            .unwrap_or_default()
    };
    Ok(Plan {
        client,
        action,
        tier,
        trust,
        config_path: path.to_path_buf(),
        before,
        after,
        command_line: (client == Client::ClaudeCode)
            .then(|| cli_command(paths, action, tier, trust)),
        file_hash: content_hash(source.as_deref()),
        registration_name: paths.server.registration_name.clone(),
        manual: client == Client::ClaudeCode && paths.claude_cli.is_none(),
    })
}

pub fn apply(
    paths: &ClientPaths,
    state_dir: &Path,
    requested: &Plan,
) -> Result<Option<PathBuf>, String> {
    let fresh = plan(
        paths,
        requested.client,
        requested.action,
        requested.tier,
        requested.trust,
    )?;
    if &fresh != requested {
        return Err(
            "The configuration changed after review. Review it again before applying.".into(),
        );
    }
    if fresh.manual {
        return Err("Claude CLI was not found. Run the displayed command manually.".into());
    }
    let backup = if fresh.config_path.is_file() {
        let dir = state_dir.join("backups/ai-clients");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let stamp = Local::now().format("%Y%m%d-%H%M%S-%f");
        let ext = fresh
            .config_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("json");
        let target = dir.join(format!("{}-{stamp}.{ext}", fresh.client.slug()));
        fs::copy(&fresh.config_path, &target).map_err(|e| e.to_string())?;
        Some(target)
    } else {
        None
    };
    if content_hash(contents(&fresh.config_path)?.as_deref()) != fresh.file_hash {
        return Err(
            "The configuration changed after review. Review it again before applying.".into(),
        );
    }
    if fresh.client == Client::ClaudeCode {
        let cli = paths
            .claude_cli
            .as_ref()
            .ok_or("Claude CLI was not found")?;
        let old = entry(
            paths,
            fresh.client,
            contents(&fresh.config_path)?.as_deref(),
        )?;
        let add = |command: &Path, args: &[String]| -> Result<(), String> {
            let output = Command::new(cli)
                .args([
                    "mcp",
                    "add",
                    "--scope",
                    "user",
                    &fresh.registration_name,
                    "--",
                ])
                .arg(command)
                .args(args)
                .output()
                .map_err(|e| e.to_string())?;
            if output.status.success() {
                Ok(())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        };
        if matches!(
            fresh.action,
            Action::Disconnect | Action::Change | Action::Repair
        ) {
            let output = Command::new(cli)
                .args(["mcp", "remove", "--scope", "user", &fresh.registration_name])
                .output()
                .map_err(|e| e.to_string())?;
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).to_string());
            }
        }
        if fresh.action != Action::Disconnect {
            if let Err(error) = add(&paths.server.path, &args_for(fresh.tier, fresh.trust)) {
                if let Some((command, args, _)) = old {
                    let restore = add(Path::new(&command), &args);
                    return Err(match restore { Ok(()) => format!("Claude CLI registration failed; previous registration restored: {error}"), Err(restore_error) => format!("Claude CLI registration failed: {error}; restoring previous registration also failed: {restore_error}") });
                }
                return Err(error);
            }
        }
    } else {
        let source = contents(&fresh.config_path)?;
        let next = changed_text(
            paths,
            fresh.client,
            source.as_deref(),
            fresh.action,
            fresh.tier,
            fresh.trust,
        )?;
        if let Some(parent) = fresh.config_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&fresh.config_path, next).map_err(|e| e.to_string())?;
    }
    let actual_entry = entry(
        paths,
        fresh.client,
        contents(&fresh.config_path)?.as_deref(),
    )?;
    let matched = if fresh.client == Client::ClaudeCode {
        if fresh.action == Action::Disconnect {
            actual_entry.is_none()
        } else {
            actual_entry.is_some_and(|(command, args, _)| {
                same_exe(&command, &paths.server.path) && args == args_for(fresh.tier, fresh.trust)
            })
        }
    } else {
        actual_entry.map(|v| v.2).unwrap_or_default() == fresh.after
    };
    if !matched {
        return Err("The registration did not match the reviewed plan.".into());
    }
    Ok(backup)
}

pub fn verify(paths: &ClientPaths, client: Client) -> Result<usize, String> {
    let registered = entry(
        paths,
        client,
        contents(config_path(paths, client))?.as_deref(),
    )?
    .ok_or("Client is not connected")?;
    let mut child = Command::new(&registered.0)
        .args(&registered.1)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let result = (|| {
        let mut input = child.stdin.take().ok_or("Missing server stdin")?;
        let stdout = child.stdout.take().ok_or("Missing server stdout")?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut lines = BufReader::new(stdout).lines();
            while let Some(Ok(line)) = lines.next() {
                if let Ok(value) = serde_json::from_str::<Value>(&line) {
                    if tx.send(value).is_err() {
                        break;
                    }
                }
            }
        });
        let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"gitcontext-gui","version":"0.1"}}});
        writeln!(input, "{init}").map_err(|e| e.to_string())?;
        let response = rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| "MCP initialize timed out")?;
        if response.get("id") != Some(&json!(1)) || response.get("error").is_some() {
            return Err(format!("MCP initialize failed: {response}"));
        }
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )
        .map_err(|e| e.to_string())?;
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}})
        )
        .map_err(|e| e.to_string())?;
        loop {
            let response = rx
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .map_err(|_| "MCP tools/list timed out")?;
            if response.get("id") == Some(&json!(2)) {
                return response
                    .pointer("/result/tools")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .ok_or_else(|| format!("MCP tools/list failed: {response}"));
            }
        }
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, ClientPaths) {
        let root = std::env::temp_dir().join(format!("gitcontext-ai-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let server_path = root.join("gitcontext-mcp.exe");
        fs::write(&server_path, "test").unwrap();
        let paths = ClientPaths {
            claude_code: root.join(".claude.json"),
            codex: root.join(".codex/config.toml"),
            claude_desktop: root.join("Claude/claude_desktop_config.json"),
            claude_cli: None,
            server: ServerInfo {
                path: server_path,
                version: Some("test".into()),
                development: false,
                built: true,
                registration_name: "gitcontext".into(),
            },
        };
        (root, paths)
    }
    fn put(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn detects_all_states_and_arguments_without_real_configs() {
        let (_, paths) = fixture();
        let first = list(&paths).unwrap();
        assert_eq!(first.clients[0].state, "not_found");
        put(&paths.claude_code, "{}");
        put(&paths.codex, "# current\n");
        put(&paths.claude_desktop, &format!("{{\"mcpServers\":{{\"gitcontext\":{{\"command\":\"{}\",\"args\":[\"--max-tier\",\"remote\",\"--trust-client-approval\"]}}}}}}", paths.server.path.display()).replace('\\', "\\\\"));
        let second = list(&paths).unwrap();
        assert_eq!(second.clients[0].state, "disconnected");
        assert_eq!(second.clients[1].state, "disconnected");
        assert_eq!(second.clients[2].state, "connected");
        assert_eq!(second.clients[2].tier, Tier::Remote);
        assert!(second.clients[2].trust);
        put(&paths.claude_code, &serde_json::to_string(&json!({"mcpServers":{"gitcontext":{"command":paths.server.path,"args":["--max-tier","local"]}}})).unwrap());
        put(
            &paths.codex,
            &changed_text(
                &paths,
                Client::Codex,
                Some("# current\n"),
                Action::Connect,
                Tier::Remote,
                true,
            )
            .unwrap(),
        );
        let third = list(&paths).unwrap();
        assert_eq!(third.clients[0].state, "connected");
        assert_eq!(third.clients[0].tier, Tier::Local);
        assert!(!third.clients[0].trust);
        assert_eq!(third.clients[1].state, "connected");
        assert_eq!(third.clients[1].tier, Tier::Remote);
        assert!(third.clients[1].trust);
        put(
            &paths.claude_desktop,
            r#"{"mcpServers":{"gitcontext":{"command":"missing.exe","args":[]}}}"#,
        );
        assert_eq!(list(&paths).unwrap().clients[2].state, "repair");
    }

    #[test]
    fn toml_add_change_remove_preserves_other_entries_and_comments() {
        let (root, paths) = fixture();
        let original = "# keep this comment\nmodel = 'sample'\n\n[mcp_servers.other]\ncommand = 'other.exe' # keep\n";
        put(&paths.codex, original);
        for tier in [Tier::Read, Tier::Local] {
            let action = if tier == Tier::Read {
                Action::Connect
            } else {
                Action::Change
            };
            let plan = plan(&paths, Client::Codex, action, tier, false).unwrap();
            apply(&paths, &root, &plan).unwrap();
            let text = fs::read_to_string(&paths.codex).unwrap();
            assert!(text.starts_with(original));
            assert_eq!(
                entry(&paths, Client::Codex, Some(&text))
                    .unwrap()
                    .map(|v| tier_from_args(&v.1)),
                Some(tier)
            );
        }
        let plan = plan(&paths, Client::Codex, Action::Disconnect, Tier::Read, false).unwrap();
        apply(&paths, &root, &plan).unwrap();
        let text = fs::read_to_string(&paths.codex).unwrap();
        assert!(text.starts_with(original));
        assert!(entry(&paths, Client::Codex, Some(&text)).unwrap().is_none());
    }

    #[test]
    fn json_preserves_key_order_and_creates_backups() {
        let (root, paths) = fixture();
        let original =
            r#"{"first":1,"mcpServers":{"other":{"command":"other.exe","args":[]}},"last":2}"#;
        put(&paths.claude_desktop, original);
        let plan = super::plan(
            &paths,
            Client::ClaudeDesktop,
            Action::Connect,
            Tier::Read,
            false,
        )
        .unwrap();
        let backup = apply(&paths, &root, &plan).unwrap().unwrap();
        assert_eq!(fs::read_to_string(backup).unwrap(), original);
        let text = fs::read_to_string(&paths.claude_desktop).unwrap();
        assert!(text.find("\"first\"").unwrap() < text.find("\"mcpServers\"").unwrap());
        assert!(text.find("\"mcpServers\"").unwrap() < text.find("\"last\"").unwrap());
        assert!(text.contains("\n  \"first\""));
        assert!(text.contains("\"other\""));
        let plan = super::plan(
            &paths,
            Client::ClaudeDesktop,
            Action::Change,
            Tier::Local,
            false,
        )
        .unwrap();
        apply(&paths, &root, &plan).unwrap();
        let text = fs::read_to_string(&paths.claude_desktop).unwrap();
        assert!(text.contains("\"other\""));
        assert_eq!(
            entry(&paths, Client::ClaudeDesktop, Some(&text))
                .unwrap()
                .map(|v| tier_from_args(&v.1)),
            Some(Tier::Local)
        );
        let plan = super::plan(
            &paths,
            Client::ClaudeDesktop,
            Action::Disconnect,
            Tier::Read,
            false,
        )
        .unwrap();
        apply(&paths, &root, &plan).unwrap();
        let text = fs::read_to_string(&paths.claude_desktop).unwrap();
        assert!(text.contains("\"other\""));
        assert!(text.find("\"first\"").unwrap() < text.find("\"last\"").unwrap());
        assert!(entry(&paths, Client::ClaudeDesktop, Some(&text))
            .unwrap()
            .is_none());
    }

    #[test]
    fn refuses_changed_file_and_invalid_trust() {
        let (root, paths) = fixture();
        put(&paths.codex, "# original\n");
        let plan = plan(&paths, Client::Codex, Action::Connect, Tier::Read, false).unwrap();
        put(&paths.codex, "# changed\n");
        assert!(apply(&paths, &root, &plan).is_err());
        assert_eq!(fs::read_to_string(&paths.codex).unwrap(), "# changed\n");
        assert!(plan_ai_trust_error(&paths));
    }
    fn plan_ai_trust_error(paths: &ClientPaths) -> bool {
        plan(paths, Client::Codex, Action::Connect, Tier::Local, true).is_err()
            && plan(
                paths,
                Client::ClaudeCode,
                Action::Connect,
                Tier::Remote,
                true,
            )
            .is_ok()
    }

    #[test]
    fn claude_code_uses_cli_command_without_writing_json() {
        let (_, paths) = fixture();
        let plan = plan(
            &paths,
            Client::ClaudeCode,
            Action::Connect,
            Tier::Remote,
            true,
        )
        .unwrap();
        assert!(plan.manual);
        assert_eq!(plan.command_line.as_deref(), Some(format!("claude mcp add --scope user gitcontext -- \"{}\" --max-tier remote --trust-client-approval", paths.server.path.display()).as_str()));
        assert!(!paths.claude_code.exists());
    }

    #[test]
    fn packaged_path_is_next_to_gui_and_development_uses_workspace_target() {
        let (root, _) = fixture();
        let release = resolve_server(&root.join("app/gitcontext.exe"), false);
        assert_eq!(release.path, root.join("app/gitcontext-mcp.exe"));
        put(&release.path, "sidecar");
        assert!(resolve_server(&root.join("app/gitcontext.exe"), false).built);
        fs::create_dir_all(root.join("src-tauri/crates")).unwrap();
        put(&root.join("src-tauri/Cargo.toml"), "[workspace]\n");
        let dev = resolve_server(&root.join("src-tauri/target/debug/gitcontext.exe"), true);
        assert_eq!(
            dev.path,
            root.join("src-tauri/target/debug/gitcontext-mcp.exe")
        );
    }

    #[test]
    fn packaged_claude_desktop_config_takes_priority() {
        let (root, _) = fixture();
        let local = root.join("Local");
        let roaming = root.join("Roaming");
        let fallback = roaming.join("Claude/claude_desktop_config.json");
        let packaged =
            local.join("Packages/Claude_demo/LocalCache/Roaming/Claude/claude_desktop_config.json");
        put(&fallback, "{}");
        assert_eq!(desktop_config_path(&local, &roaming), fallback);
        put(&packaged, "{}");
        assert_eq!(desktop_config_path(&local, &roaming), packaged);
    }

    #[cfg(windows)]
    #[test]
    fn verifies_stdio_handshake_and_tool_count_with_fake_server() {
        let (root, mut paths) = fixture();
        let script = root.join("fake-server.ps1");
        put(
            &script,
            r#"$null = [Console]::ReadLine()
[Console]::WriteLine('{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{},"serverInfo":{"name":"fake","version":"1"}}}')
$null = [Console]::ReadLine()
$null = [Console]::ReadLine()
[Console]::WriteLine('{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"one"},{"name":"two"}]}}')
"#,
        );
        let powershell = find_on_path("powershell").unwrap();
        paths.server.path = powershell.clone();
        put(&paths.claude_code, &serde_json::to_string(&json!({"mcpServers":{"gitcontext":{"command":powershell,"args":["-NoProfile","-File",script]}}})).unwrap());
        assert_eq!(verify(&paths, Client::ClaudeCode).unwrap(), 2);
    }
}
