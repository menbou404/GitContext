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
        let mut child = Command::new(env!("CARGO_BIN_EXE_gitcontext-mcp"))
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
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        assert!(!line.is_empty(), "server closed stdout");
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["id"], id);
        assert!(response.get("error").is_none(), "RPC error: {response}");
        response["result"].clone()
    }

    fn call(&mut self, id: u32, name: &str, arguments: Value) -> Value {
        self.request(
            id,
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )
    }
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
