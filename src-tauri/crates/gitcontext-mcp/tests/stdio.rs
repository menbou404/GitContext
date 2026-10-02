use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "gitcontext-mcp-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Rpc {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Rpc {
    fn start(data_dir: &Path) -> Self {
        Self::start_tier(data_dir, "read")
    }

    fn start_tier(data_dir: &Path, tier: &str) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_gitcontext-mcp"))
            .args(["--max-tier", tier])
            .env("GITCONTEXT_DATA_DIR", data_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, request: Value) {
        writeln!(self.stdin, "{request}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, id: u32, method: &str, params: Value) -> Value {
        let response = self.request_any(id, method, params);
        assert!(response.get("error").is_none(), "RPC error: {response}");
        response["result"].clone()
    }

    fn request_any(&mut self, id: u32, method: &str, params: Value) -> Value {
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        assert!(!line.is_empty(), "server closed stdout");
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["id"], id);
        response
    }

    fn call(&mut self, id: u32, name: &str, arguments: Value) -> Value {
        self.request(
            id,
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )
    }
}

fn initialize(rpc: &mut Rpc) {
    rpc.request(
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-03-26", "capabilities": {},
            "clientInfo": { "name": "fictional-test", "version": "1" }
        }),
    );
    rpc.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
}

fn local_fixture(fixture: &Fixture) -> (PathBuf, PathBuf) {
    let repo = fixture.root.join("fictional-repo");
    let data_dir = fixture.root.join("data");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(&data_dir).unwrap();
    git(&repo, &["init", "-b", "main"]);
    fs::write(
        data_dir.join("state.json"),
        json!({
            "version": 2,
            "profiles": [{
                "id": "fictional", "label": "Fictional", "accent": "#112233",
                "gitName": "Sample Person", "gitEmail": "sample@example.com",
                "githubUsername": null, "sshKeyPath": null, "ghConfigDir": null
            }],
            "repositories": [{
                "id": "repo-1", "name": "fictional-repo", "path": repo,
                "remoteUrl": null, "branch": "main", "profileId": null,
                "lastAppliedAt": null
            }]
        })
        .to_string(),
    )
    .unwrap();
    (repo, data_dir)
}

impl Drop for Rpc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn read_tools_over_stdio() {
    let fixture = Fixture::new();
    let repo = fixture.root.join("fictional-repo");
    let subdir = repo.join("src");
    let data_dir = fixture.root.join("data");
    fs::create_dir_all(&subdir).unwrap();
    fs::create_dir_all(&data_dir).unwrap();
    git(&repo, &["init", "-b", "main"]);
    git(
        &repo,
        &[
            "config",
            "--local",
            "remote.origin.url",
            "git@github.com:fictional/repo.git",
        ],
    );
    fs::write(
        data_dir.join("state.json"),
        json!({
            "version": 2,
            "profiles": [{
                "id": "fictional", "label": "Fictional", "accent": "#112233",
                "gitName": "Sample Person", "gitEmail": "sample@example.com",
                "githubUsername": "FICTIONAL", "sshKeyPath": null, "ghConfigDir": null
            }],
            "repositories": [{
                "id": "repo-1", "name": "fictional-repo", "path": repo,
                "remoteUrl": "git@github.com:fictional/repo.git", "branch": "main",
                "profileId": null, "lastAppliedAt": null
            }]
        })
        .to_string(),
    )
    .unwrap();

    let mut rpc = Rpc::start(&data_dir);
    let init = rpc.request(
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-03-26", "capabilities": {},
            "clientInfo": { "name": "fictional-test", "version": "1" }
        }),
    );
    assert!(init["instructions"]
        .as_str()
        .unwrap()
        .contains("untrusted external data"));
    rpc.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));

    let tools = rpc.request(2, "tools/list", json!({}));
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 11);
    for name in [
        "list_profiles",
        "list_repositories",
        "find_repository",
        "get_repository_status",
        "suggest_profile",
        "preview_assignment",
        "preview_commit",
        "preview_push",
        "preview_sync",
        "preview_pull_request",
        "list_pull_requests",
    ] {
        assert!(names.contains(&name), "missing {name}");
    }
    for tool in tools["tools"].as_array().unwrap() {
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(tool["annotations"]["destructiveHint"], false);
        if [
            "list_profiles",
            "preview_sync",
            "preview_pull_request",
            "list_pull_requests",
        ]
        .contains(&tool["name"].as_str().unwrap())
        {
            assert_eq!(tool["annotations"]["openWorldHint"], true);
        }
    }

    let profiles = rpc.call(3, "list_profiles", json!({}));
    assert_eq!(
        profiles["structuredContent"]["profiles"][0]["id"],
        "fictional"
    );
    assert!(profiles["structuredContent"]["profiles"][0]
        .get("github")
        .is_none());
    let repositories = rpc.call(4, "list_repositories", json!({}));
    assert_eq!(
        repositories["structuredContent"]["repositories"][0]["id"],
        "repo-1"
    );
    let found = rpc.call(5, "find_repository", json!({ "path": subdir }));
    assert_eq!(found["structuredContent"]["found"], true);
    assert_eq!(found["structuredContent"]["repository"]["id"], "repo-1");
    let preview = rpc.call(
        6,
        "preview_assignment",
        json!({ "repositoryId": "repo-1", "profileId": "fictional" }),
    );
    assert_eq!(preview["isError"], false);
    assert_eq!(preview["structuredContent"]["profile"]["id"], "fictional");
    let suggestion = rpc.call(7, "suggest_profile", json!({ "repositoryId": "repo-1" }));
    assert_eq!(suggestion["structuredContent"]["owner"], "fictional");
    assert_eq!(
        suggestion["structuredContent"]["candidates"][0]["id"],
        "fictional"
    );
    let error = rpc.call(8, "preview_commit", json!({ "repositoryId": "repo-1" }));
    assert_eq!(error["isError"], true);
    assert!(error["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Assign a Profile"));
}

#[test]
fn unavailable_tier_exits_two() {
    let output = Command::new(env!("CARGO_BIN_EXE_gitcontext-mcp"))
        .args(["--max-tier", "remote"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("read"));
    assert!(output.stdout.is_empty());
}

#[test]
fn read_tier_hides_and_rejects_local_tools() {
    let fixture = Fixture::new();
    let (_, data_dir) = local_fixture(&fixture);
    let mut rpc = Rpc::start_tier(&data_dir, "read");
    initialize(&mut rpc);
    let tools = rpc.request(2, "tools/list", json!({}));
    let names: Vec<_> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["name"].as_str().unwrap())
        .collect();
    for name in [
        "add_repository",
        "apply_profile",
        "create_branch",
        "commit",
        "pull",
    ] {
        assert!(!names.contains(&name));
        let response = rpc.request_any(3, "tools/call", json!({"name": name, "arguments": {}}));
        assert!(response.get("error").is_some(), "{name} was callable");
    }
    let preview = rpc.call(
        4,
        "preview_assignment",
        json!({"repositoryId": "repo-1", "profileId": "fictional"}),
    );
    assert!(preview["structuredContent"].get("previewId").is_none());
}

#[test]
fn local_preview_execution_and_audit() {
    let fixture = Fixture::new();
    let (repo, data_dir) = local_fixture(&fixture);
    let mut rpc = Rpc::start_tier(&data_dir, "local");
    initialize(&mut rpc);
    let tools = rpc.request(2, "tools/list", json!({}));
    let names: Vec<_> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 16);
    for tool in tools["tools"].as_array().unwrap().iter().filter(|item| {
        [
            "add_repository",
            "apply_profile",
            "create_branch",
            "commit",
            "pull",
        ]
        .contains(&item["name"].as_str().unwrap())
    }) {
        assert_eq!(tool["annotations"]["readOnlyHint"], false);
        assert_eq!(tool["annotations"]["destructiveHint"], false);
        assert_eq!(tool["annotations"]["openWorldHint"], tool["name"] == "pull");
    }
    let preview = rpc.call(
        3,
        "preview_assignment",
        json!({"repositoryId": "repo-1", "profileId": "fictional"}),
    );
    let id = preview["structuredContent"]["previewId"].as_str().unwrap();
    assert_eq!(uuid_version(id), Some('4'));
    let applied = rpc.call(4, "apply_profile", json!({"previewId": id}));
    assert_eq!(applied["isError"], false, "{applied}");
    assert_eq!(applied["structuredContent"]["profileId"], "fictional");
    let reused = rpc.call(5, "apply_profile", json!({"previewId": id}));
    assert_eq!(reused["isError"], true);

    fs::write(repo.join("first.txt"), "PRIVATE_CONTENT_MARKER").unwrap();
    let preview = rpc.call(6, "preview_commit", json!({"repositoryId": "repo-1"}));
    let id = preview["structuredContent"]["previewId"].as_str().unwrap();
    fs::write(repo.join("later.txt"), "later content").unwrap();
    let rejected = rpc.call(
        7,
        "commit",
        json!({"previewId": id, "message": "SECRET_MESSAGE_MARKER"}),
    );
    assert_eq!(rejected["isError"], true);
    assert!(rejected["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("repository changed"));
    fs::remove_file(repo.join("later.txt")).unwrap();
    let reused = rpc.call(70, "commit", json!({"previewId": id, "message": "Retry"}));
    assert_eq!(reused["isError"], true);
    assert!(reused["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("already used"));
    let preview = rpc.call(8, "preview_commit", json!({"repositoryId": "repo-1"}));
    let id = preview["structuredContent"]["previewId"].as_str().unwrap();
    let committed = rpc.call(
        9,
        "commit",
        json!({"previewId": id, "message": "SECRET_MESSAGE_MARKER"}),
    );
    assert_eq!(committed["isError"], false, "{committed}");
    assert_eq!(committed["structuredContent"]["branch"], "main");
    assert!(committed["structuredContent"]["commitId"]
        .as_str()
        .is_some_and(|id| !id.is_empty()));
    let show = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["show", "--pretty=format:", "--name-only", "HEAD"])
        .output()
        .unwrap();
    let names = String::from_utf8_lossy(&show.stdout);
    assert!(names.contains("first.txt"));
    assert!(!names.contains("later.txt"));
    let audit = fs::read_to_string(data_dir.join("mcp-audit.jsonl")).unwrap();
    let records: Vec<Value> = audit
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), 5);
    assert!(records.iter().any(|record| record["outcome"] == "success"));
    assert!(records.iter().any(|record| record["outcome"] == "rejected"));
    assert!(records
        .iter()
        .all(|record| record["client"] == "fictional-test"));
    assert!(!audit.contains("PRIVATE_CONTENT_MARKER"));
    assert!(!audit.contains("SECRET_MESSAGE_MARKER"));
    assert!(!audit.contains("first.txt"));
    assert!(!audit.contains(repo.to_string_lossy().as_ref()));
}

fn uuid_version(id: &str) -> Option<char> {
    id.chars().nth(14)
}

#[test]
fn add_repository_accepts_only_root() {
    let fixture = Fixture::new();
    let (repo, data_dir) = local_fixture(&fixture);
    let subdir = repo.join("subdir");
    fs::create_dir_all(&subdir).unwrap();
    let mut rpc = Rpc::start_tier(&data_dir, "local");
    initialize(&mut rpc);
    let accepted = rpc.call(2, "add_repository", json!({"path": repo}));
    assert_eq!(accepted["isError"], false);
    assert!(accepted["structuredContent"]["repository"]["id"]
        .as_str()
        .is_some_and(|id| !id.is_empty()));
    let rejected = rpc.call(3, "add_repository", json!({"path": subdir}));
    assert_eq!(rejected["isError"], true);
    assert!(!rejected
        .to_string()
        .contains(subdir.to_string_lossy().as_ref()));
}

#[test]
fn audit_failure_warns_after_successful_action() {
    let fixture = Fixture::new();
    let (_, data_dir) = local_fixture(&fixture);
    fs::create_dir(data_dir.join("mcp-audit.jsonl")).unwrap();
    let mut rpc = Rpc::start_tier(&data_dir, "local");
    initialize(&mut rpc);
    let preview = rpc.call(
        2,
        "preview_assignment",
        json!({"repositoryId": "repo-1", "profileId": "fictional"}),
    );
    let id = preview["structuredContent"]["previewId"].as_str().unwrap();
    let applied = rpc.call(3, "apply_profile", json!({"previewId": id}));
    assert_eq!(applied["isError"], false);
    assert_eq!(applied["structuredContent"]["profileId"], "fictional");
    assert_eq!(
        applied["structuredContent"]["warning"],
        "Audit log could not be written."
    );
}
