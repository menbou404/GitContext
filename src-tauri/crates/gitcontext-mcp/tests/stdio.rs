#[cfg(all(windows, debug_assertions))]
use gitcontext_core::approval;
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::Duration,
};

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("gitcontext-mcp-test-{}", uuid::Uuid::new_v4()));
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
        "git {:?} failed: {}",
        args,
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
        Self::start_with(data_dir, tier, false, None)
    }

    fn start_with(data_dir: &Path, tier: &str, trust: bool, timeout_ms: Option<u64>) -> Self {
        Self::start_with_desktop(data_dir, tier, trust, timeout_ms, false)
    }

    fn start_with_desktop(
        data_dir: &Path,
        tier: &str,
        trust: bool,
        timeout_ms: Option<u64>,
        desktop: bool,
    ) -> Self {
        Self::start_with_desktop_and_gui(data_dir, tier, trust, timeout_ms, desktop, None)
    }

    fn start_with_desktop_and_gui(
        data_dir: &Path,
        tier: &str,
        trust: bool,
        timeout_ms: Option<u64>,
        desktop: bool,
        gui: Option<(&str, &Path)>,
    ) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gitcontext-mcp"));
        command.args(["--max-tier", tier]);
        command.env("USERPROFILE", data_dir.parent().unwrap());
        let stub = data_dir.parent().unwrap().join("bin");
        if stub.is_dir() {
            let mut paths = vec![stub];
            paths.extend(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ));
            command.env("PATH", std::env::join_paths(paths).unwrap());
            let default_branch = data_dir.join("gh-default-branch");
            if default_branch.is_file() {
                command.env(
                    "GITCONTEXT_TEST_GH_DEFAULT_BRANCH",
                    fs::read_to_string(default_branch).unwrap(),
                );
            } else {
                command.env_remove("GITCONTEXT_TEST_GH_DEFAULT_BRANCH");
            }
        }
        command.env_remove("CLAUDE_CODE_DESKTOP_APP_VERSION");
        if desktop {
            command.env("CLAUDE_CODE_DESKTOP_APP_VERSION", "2.1.281");
        }
        if tier == "remote" {
            command.env("GITCONTEXT_TEST_STOP_BEFORE_REMOTE_EXECUTION", "1");
        }
        if data_dir.join("skip-pr-fetch").is_file() {
            command.env("GITCONTEXT_TEST_SKIP_PULL_REQUEST_FETCH", "1");
        }
        if trust {
            command.arg("--trust-client-approval");
        }
        if let Some(timeout_ms) = timeout_ms {
            command.env(
                "GITCONTEXT_TEST_CONFIRMATION_TIMEOUT_MS",
                timeout_ms.to_string(),
            );
        }
        if let Some((pipe, executable)) = gui {
            command.env("GITCONTEXT_TEST_APPROVAL_PIPE", pipe);
            command.env("GITCONTEXT_TEST_APPROVAL_SERVER_EXE", executable);
        } else {
            // Never reach a GitContext GUI that may be running on the developer's machine.
            command.env(
                "GITCONTEXT_TEST_APPROVAL_PIPE",
                format!(r"\\.\pipe\gitcontext-test-none-{}", uuid::Uuid::new_v4()),
            );
            command.env_remove("GITCONTEXT_TEST_APPROVAL_SERVER_EXE");
        }
        let mut child = command
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
        let response = self.read_message();
        assert_eq!(response["id"], id);
        response
    }

    fn read_message(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        assert!(!line.is_empty(), "server closed stdout");
        serde_json::from_str(&line).unwrap()
    }

    fn call(&mut self, id: u32, name: &str, arguments: Value) -> Value {
        self.request(
            id,
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )
    }

    fn modern_request(
        &mut self,
        id: u32,
        method: &str,
        mut params: Value,
        capabilities: Value,
    ) -> Value {
        params["_meta"] = json!({
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientInfo": {"name": "modern-test", "version": "1"},
            "io.modelcontextprotocol/clientCapabilities": capabilities
        });
        self.request(id, method, params)
    }

    fn modern_call(&mut self, id: u32, name: &str, arguments: Value, capabilities: Value) -> Value {
        self.modern_request(
            id,
            "tools/call",
            json!({"name": name, "arguments": arguments}),
            capabilities,
        )
    }
}

fn initialize(rpc: &mut Rpc) {
    initialize_with_capabilities(rpc, json!({}));
}

fn initialize_with_capabilities(rpc: &mut Rpc, capabilities: Value) {
    rpc.request(
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-11-25", "capabilities": capabilities,
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
                "remoteUrl": null, "branch": "main", "profileId": "fictional",
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
    assert_eq!(
        repositories["structuredContent"]["repositories"][0]["autoApprove"]["pushWorkBranch"],
        false
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
fn invalid_tier_exits_two() {
    let output = Command::new(env!("CARGO_BIN_EXE_gitcontext-mcp"))
        .args(["--max-tier", "invalid"])
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
    assert_eq!(preview["structuredContent"]["decision"], "existing");
    assert_eq!(preview["structuredContent"]["inheritDefaults"], false);
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

fn push_fixture(fixture: &Fixture) -> (PathBuf, PathBuf) {
    let (repo, data_dir) = local_fixture(fixture);
    let ssh_dir = fixture.root.join(".ssh");
    fs::create_dir_all(&ssh_dir).unwrap();
    let key = ssh_dir.join("fictional-key");
    fs::write(&key, "test key placeholder").unwrap();
    let mut state: Value =
        serde_json::from_str(&fs::read_to_string(data_dir.join("state.json")).unwrap()).unwrap();
    state["profiles"][0]["sshKeyPath"] = json!(key);
    state["profiles"][0]["githubUsername"] = json!("fictional");
    fs::write(data_dir.join("state.json"), state.to_string()).unwrap();
    git(&repo, &["config", "user.name", "Sample Person"]);
    git(&repo, &["config", "user.email", "sample@example.com"]);
    fs::write(repo.join("readme.txt"), "fixture").unwrap();
    git(&repo, &["add", "readme.txt"]);
    git(&repo, &["commit", "-m", "Initial fixture"]);
    git(
        &repo,
        &[
            "config",
            "remote.origin.url",
            "git@github.com:fictional/repo.git",
        ],
    );
    (repo, data_dir)
}

fn set_auto_approve(data_dir: &Path, push: bool, pull_request: bool) {
    let path = data_dir.join("state.json");
    let mut state: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    state["repositories"][0]["autoApprove"] = json!({
        "pushWorkBranch": push, "createPullRequest": pull_request
    });
    fs::write(path, state.to_string()).unwrap();
}

fn set_repository_approval(data_dir: &Path, field: &str, enabled: bool) {
    let path = data_dir.join("state.json");
    let mut state: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    state["repositories"][0]["autoApprove"][field] = json!(enabled);
    fs::write(path, state.to_string()).unwrap();
}

fn set_repository_visibility(data_dir: &Path, visibility: &str) {
    let path = data_dir.join("state.json");
    let mut state: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    state["repositories"][0]["autoApprove"]["publishVisibility"] = json!(visibility);
    fs::write(path, state.to_string()).unwrap();
}

fn set_profile_clone_approval(data_dir: &Path, enabled: bool) {
    let path = data_dir.join("state.json");
    let mut state: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    state["profiles"][0]["autoApprove"] = json!({"cloneRepository": enabled});
    fs::write(path, state.to_string()).unwrap();
}

fn assert_remote_approval(result: &Value, approved: bool, data_dir: &Path) {
    if approved {
        assert_eq!(result["isError"], true, "{result}");
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Test stopped before"),
            "{result}"
        );
        assert_eq!(last_audit(data_dir)["confirmation"], "auto");
    } else {
        assert_eq!(result["resultType"], "input_required", "{result}");
    }
}

fn install_gh_stub(fixture: &Fixture, data_dir: &Path, default_branch: Option<&str>) {
    let bin = fixture.root.join("bin");
    let gh_dir = fixture.root.join("gh-config");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&gh_dir).unwrap();
    let executable = bin.join(if cfg!(windows) { "gh.exe" } else { "gh" });
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/gh_stub.rs");
    let output = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(source)
        .arg("-o")
        .arg(executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    if let Some(branch) = default_branch {
        fs::write(data_dir.join("gh-default-branch"), branch).unwrap();
    }
    let path = data_dir.join("state.json");
    let mut state: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    state["profiles"][0]["ghConfigDir"] = json!(gh_dir);
    fs::write(path, state.to_string()).unwrap();
}

fn applied_push_preview(rpc: &mut Rpc) -> String {
    let assignment = rpc.call(
        2,
        "preview_assignment",
        json!({"repositoryId":"repo-1","profileId":"fictional"}),
    );
    let assignment_id = assignment["structuredContent"]["previewId"]
        .as_str()
        .unwrap_or_else(|| panic!("{assignment}"));
    let applied = rpc.call(3, "apply_profile", json!({"previewId":assignment_id}));
    assert_eq!(applied["isError"], false, "{applied}");
    let preview = rpc.call(4, "preview_push", json!({"repositoryId":"repo-1"}));
    assert_eq!(preview["isError"], false, "{preview}");
    preview["structuredContent"]["previewId"]
        .as_str()
        .unwrap()
        .to_string()
}

fn last_audit(data_dir: &Path) -> Value {
    let audit = fs::read_to_string(data_dir.join("mcp-audit.jsonl")).unwrap();
    serde_json::from_str(audit.lines().last().unwrap()).unwrap()
}

#[test]
fn auto_push_work_branch_skips_confirmation_even_without_elicitation() {
    let fixture = Fixture::new();
    let (repo, data_dir) = push_fixture(&fixture);
    git(&repo, &["switch", "-c", "work"]);
    install_gh_stub(&fixture, &data_dir, Some("main"));
    set_auto_approve(&data_dir, true, false);
    let mut rpc = Rpc::start_tier(&data_dir, "remote");
    initialize(&mut rpc);
    let id = applied_push_preview(&mut rpc);
    let status = rpc.call(5, "get_repository_status", json!({"repositoryId":"repo-1"}));
    assert_eq!(
        status["structuredContent"]["autoApprove"]["pushWorkBranch"],
        true
    );
    let result = rpc.modern_call(6, "push", json!({"previewId":id}), json!({}));
    assert_eq!(result["isError"], true, "{result}");
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Test stopped before"));
    assert_eq!(last_audit(&data_dir)["confirmation"], "auto");
}

#[test]
fn folder_rule_assigns_new_repository_and_inherits_push_approval() {
    let fixture = Fixture::new();
    let (repo, data_dir) = push_fixture(&fixture);
    git(&repo, &["switch", "-c", "work"]);
    install_gh_stub(&fixture, &data_dir, Some("main"));
    let path = data_dir.join("state.json");
    let mut state: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    state["repositories"] = json!([]);
    state["profiles"][0]["autoApprove"] = json!({"newRepositoryFolders":[fs::canonicalize(&fixture.root).unwrap()],
        "newRepository":{"pushWorkBranch":true,"publishVisibility":"private"}});
    fs::write(&path, state.to_string()).unwrap();
    let mut rpc = Rpc::start_tier(&data_dir, "remote");
    initialize(&mut rpc);
    let added = rpc.call(2, "add_repository", json!({"path":repo}));
    let id = added["structuredContent"]["repository"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let suggested = rpc.call(3, "suggest_profile", json!({"repositoryId":id}));
    assert_eq!(
        suggested["structuredContent"]["ruleProfile"]["id"],
        "fictional"
    );
    assert_eq!(suggested["structuredContent"]["needsConfirmation"], false);
    let preview = rpc.call(4, "preview_assignment", json!({"repositoryId":id}));
    assert_eq!(preview["structuredContent"]["decision"], "automatic");
    assert_eq!(preview["structuredContent"]["inheritDefaults"], true);
    let preview_id = preview["structuredContent"]["previewId"].as_str().unwrap();
    let applied = rpc.call(5, "apply_profile", json!({"previewId":preview_id}));
    assert_eq!(applied["isError"], false, "{applied}");
    let saved: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        saved["repositories"][0]["autoApprove"]["pushWorkBranch"],
        true
    );
    assert_eq!(
        saved["repositories"][0]["autoApproveSource"]["profileId"],
        "fictional"
    );
    assert_eq!(
        last_audit(&data_dir)["summary"],
        "Profile applied; auto-approval defaults inherited"
    );
    let push_preview = rpc.call(6, "preview_push", json!({"repositoryId":id}));
    let push_id = push_preview["structuredContent"]["previewId"]
        .as_str()
        .unwrap();
    let pushed = rpc.modern_call(7, "push", json!({"previewId":push_id}), json!({}));
    assert!(
        pushed["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Test stopped before"),
        "{pushed}"
    );
    assert_eq!(last_audit(&data_dir)["confirmation"], "auto");
}

#[test]
fn new_repository_without_matching_rule_or_with_different_profile_is_rejected() {
    for case in ["outside", "different", "ambiguous"] {
        let fixture = Fixture::new();
        let (repo, data_dir) = local_fixture(&fixture);
        let path = data_dir.join("state.json");
        let mut state: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        state["repositories"][0]["profileId"] = Value::Null;
        state["repositories"][0]["path"] = json!(fs::canonicalize(&repo).unwrap());
        if case != "outside" {
            state["profiles"][0]["autoApprove"] =
                json!({"newRepositoryFolders":[fs::canonicalize(&fixture.root).unwrap()]});
            let mut other = state["profiles"][0].clone();
            other["id"] = json!("other");
            if case == "different" {
                other["autoApprove"] = json!({});
            }
            state["profiles"].as_array_mut().unwrap().push(other);
        }
        fs::write(&path, state.to_string()).unwrap();
        let mut rpc = Rpc::start_tier(&data_dir, "local");
        initialize(&mut rpc);
        let preview = rpc.call(2, "preview_assignment", json!({"repositoryId":"repo-1","profileId":if case == "different" { "other" } else { "fictional" }}));
        assert_eq!(
            preview["structuredContent"]["decision"],
            "needsConfirmation"
        );
        let id = preview["structuredContent"]["previewId"].as_str().unwrap();
        let result = rpc.call(3, "apply_profile", json!({"previewId":id}));
        assert_eq!(result["isError"], true, "{result}");
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Ask the user which Profile"));
        assert_eq!(last_audit(&data_dir)["outcome"], "rejected");
        let saved: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(saved["repositories"][0]["profileId"].is_null());
        let _ = repo;
    }
}

#[test]
fn assignment_preview_rejects_changed_defaults() {
    let fixture = Fixture::new();
    let (repo, data_dir) = local_fixture(&fixture);
    let path = data_dir.join("state.json");
    let mut state: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    state["repositories"][0]["profileId"] = Value::Null;
    state["repositories"][0]["path"] = json!(fs::canonicalize(&repo).unwrap());
    state["profiles"][0]["autoApprove"] =
        json!({"newRepositoryFolders":[fs::canonicalize(&fixture.root).unwrap()]});
    fs::write(&path, state.to_string()).unwrap();
    let mut rpc = Rpc::start_tier(&data_dir, "local");
    initialize(&mut rpc);
    let preview = rpc.call(2, "preview_assignment", json!({"repositoryId":"repo-1"}));
    assert_eq!(preview["structuredContent"]["decision"], "automatic");
    let id = preview["structuredContent"]["previewId"].as_str().unwrap();
    state["profiles"][0]["autoApprove"]["newRepository"]["pushWorkBranch"] = json!(true);
    fs::write(&path, state.to_string()).unwrap();
    let rejected = rpc.call(3, "apply_profile", json!({"previewId":id}));
    assert_eq!(rejected["isError"], true);
    assert!(rejected["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("changed after the preview"));
    assert_eq!(last_audit(&data_dir)["outcome"], "rejected");
}

#[test]
fn auto_push_default_branch_and_failed_lookup_require_confirmation() {
    for default_branch in [Some("MaIn"), None] {
        let fixture = Fixture::new();
        let (_, data_dir) = push_fixture(&fixture);
        install_gh_stub(&fixture, &data_dir, default_branch);
        set_auto_approve(&data_dir, true, false);
        let mut rpc = Rpc::start_tier(&data_dir, "remote");
        initialize(&mut rpc);
        let id = applied_push_preview(&mut rpc);
        let result = rpc.modern_call(
            5,
            "push",
            json!({"previewId":id}),
            json!({"elicitation":{"form":{}}}),
        );
        assert_eq!(result["resultType"], "input_required", "{result}");
    }
}

#[test]
fn push_approval_uses_only_the_matching_branch_setting() {
    for (working, work_enabled, default_enabled) in [
        (true, false, false),
        (true, true, false),
        (true, false, true),
        (true, true, true),
        (false, false, false),
        (false, true, false),
        (false, false, true),
        (false, true, true),
    ] {
        let fixture = Fixture::new();
        let (repo, data_dir) = push_fixture(&fixture);
        if working {
            git(&repo, &["switch", "-c", "work"]);
        }
        install_gh_stub(&fixture, &data_dir, Some("MaIn"));
        set_repository_approval(&data_dir, "pushWorkBranch", work_enabled);
        set_repository_approval(&data_dir, "pushDefaultBranch", default_enabled);
        let mut rpc = Rpc::start_tier(&data_dir, "remote");
        initialize(&mut rpc);
        let id = applied_push_preview(&mut rpc);
        let status = rpc.call(5, "get_repository_status", json!({"repositoryId":"repo-1"}));
        assert_eq!(
            status["structuredContent"]["autoApprove"]["pushDefaultBranch"],
            default_enabled
        );
        let approved = if working {
            work_enabled
        } else {
            default_enabled
        };
        let capabilities = if approved {
            json!({})
        } else {
            json!({"elicitation":{"form":{}}})
        };
        let result = rpc.modern_call(6, "push", json!({"previewId":id}), capabilities);
        assert_remote_approval(&result, approved, &data_dir);
    }
}

#[test]
fn failed_default_branch_lookup_never_auto_approves_push() {
    for working in [false, true] {
        let fixture = Fixture::new();
        let (repo, data_dir) = push_fixture(&fixture);
        if working {
            git(&repo, &["switch", "-c", "work"]);
        }
        install_gh_stub(&fixture, &data_dir, None);
        set_repository_approval(&data_dir, "pushWorkBranch", true);
        set_repository_approval(&data_dir, "pushDefaultBranch", true);
        let mut rpc = Rpc::start_tier(&data_dir, "remote");
        initialize(&mut rpc);
        let id = applied_push_preview(&mut rpc);
        let result = rpc.modern_call(
            5,
            "push",
            json!({"previewId":id}),
            json!({"elicitation":{"form":{}}}),
        );
        assert_remote_approval(&result, false, &data_dir);
    }
}

#[test]
fn clone_approval_comes_from_the_preview_profile_without_a_repository() {
    for approved in [false, true] {
        let fixture = Fixture::new();
        let (_, data_dir) = push_fixture(&fixture);
        set_profile_clone_approval(&data_dir, approved);
        let mut rpc = Rpc::start_tier(&data_dir, "remote");
        initialize(&mut rpc);
        let profiles = rpc.call(2, "list_profiles", json!({}));
        assert_eq!(
            profiles["structuredContent"]["profiles"][0]["autoApprove"]["cloneRepository"],
            approved,
            "{profiles}"
        );
        let preview = rpc.call(3, "preview_clone", json!({"profileId":"fictional","sshUrl":"git@github.com:fictional/new-repo.git","destinationParent":fixture.root}));
        assert_eq!(preview["isError"], false, "{preview}");
        let id = preview["structuredContent"]["previewId"].as_str().unwrap();
        let capabilities = if approved {
            json!({})
        } else {
            json!({"elicitation":{"form":{}}})
        };
        let result = rpc.modern_call(4, "clone_repository", json!({"previewId":id}), capabilities);
        assert_remote_approval(&result, approved, &data_dir);
    }
}

#[test]
fn merge_approval_is_independent_of_other_repository_settings() {
    for approved in [false, true] {
        let fixture = Fixture::new();
        let (_, data_dir) = push_fixture(&fixture);
        install_gh_stub(&fixture, &data_dir, Some("main"));
        set_repository_approval(&data_dir, "mergePullRequest", approved);
        let mut rpc = Rpc::start_tier(&data_dir, "remote");
        initialize(&mut rpc);
        let _ = applied_push_preview(&mut rpc);
        let preview = rpc.call(
            5,
            "preview_merge",
            json!({"repositoryId":"repo-1","number":1}),
        );
        assert_eq!(preview["isError"], false, "{preview}");
        let id = preview["structuredContent"]["previewId"].as_str().unwrap();
        let capabilities = if approved {
            json!({})
        } else {
            json!({"elicitation":{"form":{}}})
        };
        let result = rpc.modern_call(
            6,
            "merge_pull_request",
            json!({"previewId":id,"strategy":"merge"}),
            capabilities,
        );
        assert_remote_approval(&result, approved, &data_dir);
    }
}

#[test]
fn publish_approval_uses_registered_repository_setting() {
    for (approved, publish_visibility, visibility) in [
        (false, "private", "private"),
        (true, "private", "private"),
        (true, "private", "public"),
        (true, "any", "public"),
    ] {
        let fixture = Fixture::new();
        let (repo, data_dir) = local_fixture(&fixture);
        git(&repo, &["config", "user.name", "Sample Person"]);
        git(&repo, &["config", "user.email", "sample@example.com"]);
        fs::write(repo.join("readme.txt"), "fixture").unwrap();
        git(&repo, &["add", "readme.txt"]);
        git(&repo, &["commit", "-m", "Initial fixture"]);
        install_gh_stub(&fixture, &data_dir, Some("main"));
        set_repository_approval(&data_dir, "publishRepository", approved);
        set_repository_visibility(&data_dir, publish_visibility);
        let mut rpc = Rpc::start_tier(&data_dir, "remote");
        initialize(&mut rpc);
        let assignment = rpc.call(
            2,
            "preview_assignment",
            json!({"repositoryId":"repo-1","profileId":"fictional"}),
        );
        let assignment_id = assignment["structuredContent"]["previewId"]
            .as_str()
            .unwrap();
        assert_eq!(
            rpc.call(3, "apply_profile", json!({"previewId":assignment_id}))["isError"],
            false
        );
        let preview = rpc.call(
            4,
            "preview_publish",
            json!({"repositoryId":"repo-1","name":"published-repo","visibility":visibility}),
        );
        assert_eq!(preview["isError"], false, "{preview}");
        let id = preview["structuredContent"]["previewId"].as_str().unwrap();
        let expected_auto = approved && (visibility == "private" || publish_visibility == "any");
        let capabilities = if expected_auto {
            json!({})
        } else {
            json!({"elicitation":{"form":{}}})
        };
        let result = rpc.modern_call(
            5,
            "publish_repository",
            json!({"previewId":id}),
            capabilities,
        );
        assert_remote_approval(&result, expected_auto, &data_dir);
    }
}

#[test]
fn auto_push_setting_is_reloaded_after_preview() {
    let fixture = Fixture::new();
    let (repo, data_dir) = push_fixture(&fixture);
    git(&repo, &["switch", "-c", "work"]);
    install_gh_stub(&fixture, &data_dir, Some("main"));
    set_auto_approve(&data_dir, true, false);
    let mut rpc = Rpc::start_tier(&data_dir, "remote");
    initialize(&mut rpc);
    let id = applied_push_preview(&mut rpc);
    set_auto_approve(&data_dir, false, false);
    let result = rpc.modern_call(
        5,
        "push",
        json!({"previewId":id}),
        json!({"elicitation":{"form":{}}}),
    );
    assert_eq!(result["resultType"], "input_required", "{result}");
}

#[test]
fn auto_create_pull_request_skips_confirmation_but_merge_does_not() {
    let fixture = Fixture::new();
    let (repo, data_dir) = push_fixture(&fixture);
    git(&repo, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    fs::write(data_dir.join("skip-pr-fetch"), "1").unwrap();
    git(&repo, &["switch", "-c", "work"]);
    fs::write(repo.join("work.txt"), "fixture").unwrap();
    git(&repo, &["add", "work.txt"]);
    git(&repo, &["commit", "-m", "Work fixture"]);
    install_gh_stub(&fixture, &data_dir, Some("main"));
    set_auto_approve(&data_dir, true, true);
    let mut rpc = Rpc::start_tier(&data_dir, "remote");
    initialize(&mut rpc);
    let assignment = rpc.call(
        2,
        "preview_assignment",
        json!({"repositoryId":"repo-1","profileId":"fictional"}),
    );
    let assignment_id = assignment["structuredContent"]["previewId"]
        .as_str()
        .unwrap();
    assert_eq!(
        rpc.call(3, "apply_profile", json!({"previewId":assignment_id}))["isError"],
        false
    );
    let preview = rpc.call(4, "preview_pull_request", json!({"repositoryId":"repo-1"}));
    assert_eq!(preview["isError"], false, "{preview}");
    let id = preview["structuredContent"]["previewId"].as_str().unwrap();
    let result = rpc.modern_call(
        5,
        "create_pull_request",
        json!({"previewId":id,"title":"Fixture PR","body":"","draft":false}),
        json!({}),
    );
    assert_eq!(result["isError"], true, "{result}");
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Test stopped before"));
    assert_eq!(last_audit(&data_dir)["confirmation"], "auto");
    set_repository_approval(&data_dir, "createPullRequest", false);
    let second_preview = rpc.call(8, "preview_pull_request", json!({"repositoryId":"repo-1"}));
    assert_eq!(second_preview["isError"], false, "{second_preview}");
    let second_id = second_preview["structuredContent"]["previewId"]
        .as_str()
        .unwrap();
    let second_result = rpc.modern_call(
        9,
        "create_pull_request",
        json!({"previewId":second_id,"title":"Fixture PR","body":"","draft":false}),
        json!({"elicitation":{"form":{}}}),
    );
    assert_eq!(
        second_result["resultType"], "input_required",
        "{second_result}"
    );
    let merge = rpc.call(
        6,
        "preview_merge",
        json!({"repositoryId":"repo-1","number":1}),
    );
    assert_eq!(merge["isError"], false, "{merge}");
    let merge_id = merge["structuredContent"]["previewId"].as_str().unwrap();
    let result = rpc.modern_call(
        7,
        "merge_pull_request",
        json!({"previewId":merge_id,"strategy":"merge"}),
        json!({"elicitation":{"form":{}}}),
    );
    assert_eq!(result["resultType"], "input_required", "{result}");
}

#[test]
fn remote_tier_exposes_only_remote_operations() {
    let fixture = Fixture::new();
    let (_, data_dir) = local_fixture(&fixture);
    for (tier, expected) in [("local", false), ("remote", true)] {
        let mut rpc = Rpc::start_tier(&data_dir, tier);
        initialize(&mut rpc);
        let list = rpc.request(2, "tools/list", json!({}));
        let names: Vec<_> = list["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert!(!names.contains(&"set_repository_auto_approve"));
        assert!(!names.contains(&"set_profile_auto_approve"));
        assert!(!names.contains(&"set_new_repository_defaults"));
        assert!(!names.contains(&"add_new_repository_folder"));
        assert!(!names.contains(&"remove_new_repository_folder"));
        for name in [
            "preview_merge",
            "preview_clone",
            "preview_publish",
            "push",
            "create_pull_request",
            "merge_pull_request",
            "clone_repository",
            "publish_repository",
        ] {
            assert_eq!(names.contains(&name), expected, "{tier}: {name}");
        }
        if expected {
            for tool in list["tools"].as_array().unwrap().iter().filter(|tool| {
                [
                    "push",
                    "create_pull_request",
                    "merge_pull_request",
                    "clone_repository",
                    "publish_repository",
                ]
                .contains(&tool["name"].as_str().unwrap())
            }) {
                assert_eq!(tool["annotations"]["readOnlyHint"], false);
                assert_eq!(tool["annotations"]["destructiveHint"], true);
                assert_eq!(tool["annotations"]["openWorldHint"], true);
            }
        }
    }
}

#[test]
fn push_preview_id_is_remote_only() {
    for tier in ["local", "remote"] {
        let fixture = Fixture::new();
        let (_, data_dir) = push_fixture(&fixture);
        let mut rpc = Rpc::start_tier(&data_dir, tier);
        initialize(&mut rpc);
        let assignment = rpc.call(
            2,
            "preview_assignment",
            json!({"repositoryId":"repo-1","profileId":"fictional"}),
        );
        let id = assignment["structuredContent"]["previewId"]
            .as_str()
            .unwrap();
        assert_eq!(
            rpc.call(3, "apply_profile", json!({"previewId":id}))["isError"],
            false
        );
        let preview = rpc.call(4, "preview_push", json!({"repositoryId":"repo-1"}));
        assert_eq!(preview["isError"], false, "{preview}");
        assert_eq!(
            preview["structuredContent"].get("previewId").is_some(),
            tier == "remote"
        );
    }
}

#[test]
fn clone_preview_validates_home_and_destination_without_network() {
    let fixture = Fixture::new();
    let (_, data_dir) = push_fixture(&fixture);
    let mut rpc = Rpc::start_tier(&data_dir, "remote");
    initialize(&mut rpc);
    let preview = rpc.call(2, "preview_clone", json!({"profileId":"fictional","sshUrl":" git@github.com:fictional/new-repo.git ","destinationParent":fixture.root}));
    assert_eq!(preview["isError"], false, "{preview}");
    assert_eq!(
        preview["structuredContent"]["sshUrl"],
        "git@github.com:fictional/new-repo.git"
    );
    assert!(preview["structuredContent"]["previewId"].as_str().is_some());
    fs::create_dir(fixture.root.join("new-repo")).unwrap();
    let rejected = rpc.call(
        3,
        "clone_repository",
        json!({"previewId":preview["structuredContent"]["previewId"]}),
    );
    assert_eq!(rejected["isError"], true);
    assert!(rejected["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("repository changed"));
}

#[test]
fn remote_without_elicitation_is_rejected_and_audited() {
    let fixture = Fixture::new();
    let (_, data_dir) = push_fixture(&fixture);
    let mut rpc = Rpc::start_tier(&data_dir, "remote");
    initialize(&mut rpc);
    let id = applied_push_preview(&mut rpc);
    let result = rpc.call(5, "push", json!({"previewId":id}));
    assert_eq!(result["isError"], true);
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("cannot show GitContext's confirmation prompt"));
    let audit = fs::read_to_string(data_dir.join("mcp-audit.jsonl")).unwrap();
    let last: Value = serde_json::from_str(audit.lines().last().unwrap()).unwrap();
    assert_eq!(last["tool"], "push");
    assert_eq!(last["outcome"], "rejected");
    assert!(last["confirmation"].is_null());
}

#[cfg(all(windows, debug_assertions))]
#[test]
fn gui_confirmation_over_stdio_approves_declines_and_rejects_immediate_answers() {
    for (approved, pause, expected) in [
        (true, 1100, "Test stopped before"),
        (false, 1100, "Confirmation declined"),
        (true, 0, "too quickly"),
    ] {
        let fixture = Fixture::new();
        let (_, data_dir) = push_fixture(&fixture);
        let pipe = format!(r"\\.\pipe\gitcontext-mcp-test-{}", uuid::Uuid::new_v4());
        approval::start_server(pipe.clone(), move |_| {
            std::thread::sleep(Duration::from_millis(pause));
            approved
        })
        .unwrap();
        let executable = std::env::current_exe().unwrap();
        let mut rpc = Rpc::start_with_desktop_and_gui(
            &data_dir,
            "remote",
            false,
            None,
            false,
            Some((&pipe, &executable)),
        );
        initialize(&mut rpc);
        let id = applied_push_preview(&mut rpc);
        let response = rpc.call(5, "push", json!({"previewId":id}));
        assert!(
            response["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains(expected),
            "{response}"
        );
        assert_eq!(last_audit(&data_dir)["confirmation"], "gui");
    }
}

#[test]
fn modern_requests_use_per_request_elicitation_capability() {
    for (supports_form, trust) in [(true, false), (true, true), (false, false), (false, true)] {
        let fixture = Fixture::new();
        let (_, data_dir) = push_fixture(&fixture);
        let mut rpc = Rpc::start_with(&data_dir, "remote", trust, None);
        let capabilities = if supports_form {
            json!({"elicitation":{"form":{}}})
        } else {
            json!({})
        };
        let discovered = rpc.modern_request(1, "server/discover", json!({}), capabilities.clone());
        assert_eq!(
            discovered["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
            "rmcp"
        );
        let tools = rpc.modern_request(2, "tools/list", json!({}), capabilities.clone());
        assert!(tools["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "push"));
        let assignment = rpc.modern_call(
            3,
            "preview_assignment",
            json!({"repositoryId":"repo-1","profileId":"fictional"}),
            capabilities.clone(),
        );
        let assignment_id = assignment["structuredContent"]["previewId"]
            .as_str()
            .unwrap();
        let applied = rpc.modern_call(
            4,
            "apply_profile",
            json!({"previewId":assignment_id}),
            capabilities.clone(),
        );
        assert_eq!(applied["isError"], false, "{applied}");
        let preview = rpc.modern_call(
            5,
            "preview_push",
            json!({"repositoryId":"repo-1"}),
            capabilities.clone(),
        );
        let preview_id = preview["structuredContent"]["previewId"].as_str().unwrap();
        let first = rpc.modern_call(
            6,
            "push",
            json!({"previewId":preview_id}),
            capabilities.clone(),
        );
        if supports_form {
            assert_eq!(first["resultType"], "input_required", "{first}");
            assert_eq!(
                first["inputRequests"]["approval"]["method"],
                "elicitation/create"
            );
            assert_eq!(
                first["inputRequests"]["approval"]["params"]["requestedSchema"]["properties"]
                    ["approved"]["type"],
                "boolean"
            );
            assert!(
                first["inputRequests"]["approval"]["params"]["requestedSchema"]["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("approved"))
            );
            assert!(first["inputRequests"]["approval"]["params"]["message"]
                .as_str()
                .unwrap()
                .contains("Profile: Fictional"));
            let state = first["requestState"].as_str().unwrap();
            assert_eq!(uuid_version(state), Some('4'));
            let response = rpc.modern_request(7, "tools/call", json!({"name":"push","arguments":{"previewId":preview_id},"requestState":state,"inputResponses":{"approval":{"action":"decline"}}}), capabilities);
            assert_eq!(response["isError"], true, "{response}");
        } else if trust {
            assert!(first["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Test stopped before"));
        } else {
            assert!(first["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("cannot show GitContext's confirmation prompt"));
        }
        let audit = fs::read_to_string(data_dir.join("mcp-audit.jsonl")).unwrap();
        let last: Value = serde_json::from_str(audit.lines().last().unwrap()).unwrap();
        assert_eq!(last["client"], "modern-test");
        assert_eq!(
            last["confirmation"],
            if supports_form {
                json!("elicitation")
            } else if trust {
                json!("client")
            } else {
                json!(null)
            }
        );
    }
}

fn modern_push_round(timeout_ms: Option<u64>) -> (Fixture, Rpc, PathBuf, String, String) {
    let fixture = Fixture::new();
    let (_, data_dir) = push_fixture(&fixture);
    let mut rpc = Rpc::start_with(&data_dir, "remote", false, timeout_ms);
    let capabilities = json!({"elicitation":{"form":{},"url":{}}});
    let assignment = rpc.modern_call(
        1,
        "preview_assignment",
        json!({"repositoryId":"repo-1","profileId":"fictional"}),
        capabilities.clone(),
    );
    let id = assignment["structuredContent"]["previewId"]
        .as_str()
        .unwrap();
    let applied = rpc.modern_call(
        2,
        "apply_profile",
        json!({"previewId":id}),
        capabilities.clone(),
    );
    assert_eq!(applied["isError"], false, "{applied}");
    let preview = rpc.modern_call(
        3,
        "preview_push",
        json!({"repositoryId":"repo-1"}),
        capabilities.clone(),
    );
    let preview_id = preview["structuredContent"]["previewId"]
        .as_str()
        .unwrap()
        .to_string();
    let first = rpc.modern_call(4, "push", json!({"previewId":preview_id}), capabilities);
    assert_eq!(first["resultType"], "input_required", "{first}");
    let state = first["requestState"].as_str().unwrap().to_string();
    let audit = fs::read_to_string(data_dir.join("mcp-audit.jsonl")).unwrap();
    assert!(!audit
        .lines()
        .any(|line| serde_json::from_str::<Value>(line).unwrap()["tool"] == "push"));
    (fixture, rpc, data_dir, preview_id, state)
}

fn modern_push_retry(
    rpc: &mut Rpc,
    id: u32,
    preview_id: &str,
    state: &str,
    response: Value,
) -> Value {
    rpc.modern_request(
        id,
        "tools/call",
        json!({
            "name":"push", "arguments":{"previewId":preview_id},
            "requestState":state, "inputResponses":{"approval":response}
        }),
        json!({"elicitation":{"form":{},"url":{}}}),
    )
}

fn assert_push_audit(data_dir: &Path, expected_outcome: &str) {
    let audit = fs::read_to_string(data_dir.join("mcp-audit.jsonl")).unwrap();
    let last: Value = serde_json::from_str(audit.lines().last().unwrap()).unwrap();
    assert_eq!(last["tool"], "push");
    assert_eq!(last["outcome"], expected_outcome);
    assert_eq!(last["confirmation"], "elicitation");
}

#[test]
fn modern_mrtr_approval_and_denials() {
    for (response, expected, wait) in [
        (
            json!({"action":"accept","content":{"approved":true}}),
            "approved too quickly",
            false,
        ),
        (
            json!({"action":"accept","content":{"approved":true}}),
            "Test stopped before",
            true,
        ),
        (
            json!({"action":"decline"}),
            "declined without showing",
            false,
        ),
        (json!({"action":"decline"}), "Confirmation declined.", true),
        (
            json!({"action":"cancel"}),
            "declined without showing",
            false,
        ),
        (
            json!({"action":"accept","content":{"approved":false}}),
            "declined without showing",
            false,
        ),
        (
            json!({"action":"accept"}),
            "declined without showing",
            false,
        ),
        (
            json!({"action":"accept","content":{"approved":"yes"}}),
            "declined without showing",
            false,
        ),
    ] {
        let (_fixture, mut rpc, data_dir, preview_id, state) = modern_push_round(None);
        if wait {
            std::thread::sleep(Duration::from_millis(1050));
        }
        let result = modern_push_retry(&mut rpc, 5, &preview_id, &state, response);
        assert_eq!(result["isError"], true, "{result}");
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains(expected),
            "{result}"
        );
        assert_push_audit(
            &data_dir,
            if expected == "Test stopped before" {
                "failed"
            } else {
                "rejected"
            },
        );
        let audit = fs::read_to_string(data_dir.join("mcp-audit.jsonl")).unwrap();
        let last: Value = serde_json::from_str(audit.lines().last().unwrap()).unwrap();
        if !wait {
            assert_eq!(
                last["summary"],
                "Remote operation rejected: client answered without showing the prompt"
            );
        }
        let reused = modern_push_retry(
            &mut rpc,
            6,
            &preview_id,
            &state,
            json!({"action":"accept","content":{"approved":true}}),
        );
        assert_eq!(reused["isError"], true);
        assert_push_audit(&data_dir, "rejected");
    }
}

#[test]
fn modern_mrtr_invalid_retries_and_expiry() {
    let (_fixture, mut rpc, data_dir, preview_id, state) = modern_push_round(None);
    let tampered = modern_push_retry(
        &mut rpc,
        5,
        &preview_id,
        "tampered-state",
        json!({"action":"accept","content":{"approved":true}}),
    );
    assert_eq!(tampered["isError"], true);
    assert_push_audit(&data_dir, "rejected");
    let mismatched = modern_push_retry(
        &mut rpc,
        6,
        "other-preview",
        &state,
        json!({"action":"accept","content":{"approved":true}}),
    );
    assert_eq!(mismatched["isError"], true);
    assert_push_audit(&data_dir, "rejected");
    let reused = modern_push_retry(
        &mut rpc,
        7,
        &preview_id,
        &state,
        json!({"action":"accept","content":{"approved":true}}),
    );
    assert_eq!(reused["isError"], true);
    let missing_state = rpc.modern_request(8, "tools/call", json!({"name":"push","arguments":{"previewId":preview_id},"inputResponses":{"approval":{"action":"accept","content":{"approved":true}}}}), json!({"elicitation":{"form":{}}}));
    assert_eq!(missing_state["isError"], true);
    assert_push_audit(&data_dir, "rejected");

    let (_fixture, mut rpc, data_dir, preview_id, state) = modern_push_round(None);
    let missing_responses = rpc.modern_request(
        5,
        "tools/call",
        json!({"name":"push","arguments":{"previewId":preview_id},"requestState":state}),
        json!({"elicitation":{"form":{}}}),
    );
    assert_eq!(missing_responses["isError"], true);
    assert_push_audit(&data_dir, "rejected");
    let reused = modern_push_retry(
        &mut rpc,
        6,
        &preview_id,
        &state,
        json!({"action":"accept","content":{"approved":true}}),
    );
    assert_eq!(reused["isError"], true);

    let (_fixture, mut rpc, data_dir, preview_id, state) = modern_push_round(None);
    let missing_approval = rpc.modern_request(5, "tools/call", json!({"name":"push","arguments":{"previewId":preview_id},"requestState":state,"inputResponses":{}}), json!({"elicitation":{"form":{}}}));
    assert_eq!(missing_approval["isError"], true);
    assert_push_audit(&data_dir, "rejected");

    let (_fixture, mut rpc, data_dir, preview_id, state) = modern_push_round(Some(10));
    std::thread::sleep(std::time::Duration::from_millis(30));
    let expired = modern_push_retry(
        &mut rpc,
        5,
        &preview_id,
        &state,
        json!({"action":"accept","content":{"approved":true}}),
    );
    assert_eq!(expired["isError"], true);
    assert!(expired["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("timed out"));
    assert_push_audit(&data_dir, "rejected");
}

#[test]
fn modern_mrtr_rechecks_fingerprint_after_approval() {
    let (fixture, mut rpc, data_dir, preview_id, state) = modern_push_round(None);
    fs::write(
        fixture.root.join("fictional-repo").join("changed.txt"),
        "changed",
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(1050));
    let result = modern_push_retry(
        &mut rpc,
        5,
        &preview_id,
        &state,
        json!({"action":"accept","content":{"approved":true}}),
    );
    assert_eq!(result["isError"], true);
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("repository changed"));
    assert_push_audit(&data_dir, "rejected");
}

#[test]
fn modern_mrtr_state_cannot_be_reused_for_another_tool() {
    let (_fixture, mut rpc, data_dir, preview_id, state) = modern_push_round(None);
    let wrong_tool = rpc.modern_request(5, "tools/call", json!({
        "name":"clone_repository", "arguments":{"previewId":preview_id},
        "requestState":state, "inputResponses":{"approval":{"action":"accept","content":{"approved":true}}}
    }), json!({"elicitation":{"form":{}}}));
    assert_eq!(wrong_tool["isError"], true);
    let reused = modern_push_retry(
        &mut rpc,
        6,
        &preview_id,
        &state,
        json!({"action":"accept","content":{"approved":true}}),
    );
    assert_eq!(reused["isError"], true);
    assert_push_audit(&data_dir, "rejected");
}

#[test]
fn elicitation_decline_cancel_accept_and_timeout() {
    for (action, content, expected, wait) in [
        ("decline", json!(null), "declined without showing", false),
        ("decline", json!(null), "Confirmation declined.", true),
        ("cancel", json!(null), "declined without showing", false),
        (
            "accept",
            json!({"approved":false}),
            "declined without showing",
            false,
        ),
        ("accept", json!(null), "declined without showing", false),
        (
            "accept",
            json!({"approved":"yes"}),
            "declined without showing",
            false,
        ),
        (
            "accept",
            json!({"approved":true}),
            "approved too quickly",
            false,
        ),
        (
            "accept",
            json!({"approved":true}),
            "Test stopped before",
            true,
        ),
    ] {
        let fixture = Fixture::new();
        let (_, data_dir) = push_fixture(&fixture);
        let mut rpc = Rpc::start_tier(&data_dir, "remote");
        initialize_with_capabilities(&mut rpc, json!({"elicitation":{"form":{}}}));
        let id = applied_push_preview(&mut rpc);
        rpc.send(json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"push","arguments":{"previewId":id}}}));
        let request = rpc.read_message();
        assert_eq!(request["method"], "elicitation/create", "{request}");
        assert!(request["params"]["message"]
            .as_str()
            .unwrap()
            .contains("Profile: Fictional"));
        assert_eq!(
            request["params"]["requestedSchema"]["properties"]["approved"]["type"],
            "boolean"
        );
        if wait {
            std::thread::sleep(Duration::from_millis(1050));
        }
        rpc.send(json!({"jsonrpc":"2.0","id":request["id"],"result":{"action":action,"content":content}}));
        let response = rpc.read_message();
        assert_eq!(response["id"], 5, "{response}");
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains(expected),
            "{response}"
        );
        let audit = fs::read_to_string(data_dir.join("mcp-audit.jsonl")).unwrap();
        let last: Value = serde_json::from_str(audit.lines().last().unwrap()).unwrap();
        assert_eq!(last["confirmation"], "elicitation");
        if !wait {
            assert_eq!(
                last["summary"],
                "Remote operation rejected: client answered without showing the prompt"
            );
            assert_eq!(last["outcome"], "rejected");
        }
    }
    let fixture = Fixture::new();
    let (_, data_dir) = push_fixture(&fixture);
    let mut rpc = Rpc::start_with(&data_dir, "remote", false, Some(30));
    initialize_with_capabilities(&mut rpc, json!({"elicitation":{"form":{}}}));
    let id = applied_push_preview(&mut rpc);
    rpc.send(json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"push","arguments":{"previewId":id}}}));
    let request = rpc.read_message();
    assert_eq!(request["method"], "elicitation/create");
    let mut response = rpc.read_message();
    while response["id"] != 5 {
        response = rpc.read_message();
    }
    assert!(
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("timed out"),
        "{response}"
    );
}

#[test]
fn trusted_client_with_elicitation_still_receives_prompt() {
    let fixture = Fixture::new();
    let (_, data_dir) = push_fixture(&fixture);
    let mut rpc = Rpc::start_with(&data_dir, "remote", true, None);
    initialize_with_capabilities(&mut rpc, json!({"elicitation":{"form":{}}}));
    let id = applied_push_preview(&mut rpc);
    rpc.send(json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"push","arguments":{"previewId":id}}}));
    let request = rpc.read_message();
    assert_eq!(request["method"], "elicitation/create");
    rpc.send(json!({"jsonrpc":"2.0","id":request["id"],"result":{"action":"decline"}}));
    let response = rpc.read_message();
    assert_eq!(response["result"]["isError"], true);
    assert!(response["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("declined without showing"));
    let audit = fs::read_to_string(data_dir.join("mcp-audit.jsonl")).unwrap();
    let last: Value = serde_json::from_str(audit.lines().last().unwrap()).unwrap();
    assert_eq!(last["confirmation"], "elicitation");
}

#[test]
fn trusted_client_without_elicitation_uses_client_confirmation() {
    let fixture = Fixture::new();
    let (_, data_dir) = push_fixture(&fixture);
    let mut rpc = Rpc::start_with(&data_dir, "remote", true, None);
    initialize(&mut rpc);
    let id = applied_push_preview(&mut rpc);
    let response = rpc.call(5, "push", json!({"previewId":id}));
    assert!(response["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Test stopped before"));
    let audit = fs::read_to_string(data_dir.join("mcp-audit.jsonl")).unwrap();
    let last: Value = serde_json::from_str(audit.lines().last().unwrap()).unwrap();
    assert_eq!(last["confirmation"], "client");
}

#[test]
fn desktop_code_tab_rejects_before_elicitation_unless_trusted() {
    for modern in [false, true] {
        for trust in [false, true] {
            let fixture = Fixture::new();
            let (_, data_dir) = push_fixture(&fixture);
            let mut rpc = Rpc::start_with_desktop(&data_dir, "remote", trust, None, true);
            let capabilities = json!({"elicitation":{"form":{}}});
            let result = if modern {
                let assignment = rpc.modern_call(
                    1,
                    "preview_assignment",
                    json!({"repositoryId":"repo-1","profileId":"fictional"}),
                    capabilities.clone(),
                );
                let id = assignment["structuredContent"]["previewId"]
                    .as_str()
                    .unwrap();
                rpc.modern_call(
                    2,
                    "apply_profile",
                    json!({"previewId":id}),
                    capabilities.clone(),
                );
                let preview = rpc.modern_call(
                    3,
                    "preview_push",
                    json!({"repositoryId":"repo-1"}),
                    capabilities.clone(),
                );
                let id = preview["structuredContent"]["previewId"].as_str().unwrap();
                rpc.modern_call(4, "push", json!({"previewId":id}), capabilities)
            } else {
                initialize_with_capabilities(&mut rpc, capabilities);
                let id = applied_push_preview(&mut rpc);
                rpc.call(5, "push", json!({"previewId":id}))
            };
            assert_ne!(result["resultType"], "input_required", "{result}");
            let message = result["content"][0]["text"].as_str().unwrap();
            if trust {
                assert!(message.contains("Test stopped before"), "{result}");
            } else {
                assert!(
                    message.contains("cannot show GitContext's confirmation prompt"),
                    "{result}"
                );
            }
            let audit = fs::read_to_string(data_dir.join("mcp-audit.jsonl")).unwrap();
            let last: Value = serde_json::from_str(audit.lines().last().unwrap()).unwrap();
            assert_eq!(
                last["confirmation"],
                if trust { json!("client") } else { json!(null) }
            );
        }
    }
}

#[test]
fn acceptance_rechecks_the_fingerprint() {
    let fixture = Fixture::new();
    let (repo, data_dir) = push_fixture(&fixture);
    let mut rpc = Rpc::start_tier(&data_dir, "remote");
    initialize_with_capabilities(&mut rpc, json!({"elicitation":{"form":{}}}));
    let id = applied_push_preview(&mut rpc);
    rpc.send(json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"push","arguments":{"previewId":id}}}));
    let request = rpc.read_message();
    assert_eq!(request["method"], "elicitation/create");
    fs::write(repo.join("changed-after-preview.txt"), "change").unwrap();
    std::thread::sleep(Duration::from_millis(1050));
    rpc.send(json!({"jsonrpc":"2.0","id":request["id"],"result":{"action":"accept","content":{"approved":true}}}));
    let response = rpc.read_message();
    assert_eq!(response["id"], 5);
    assert!(
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("repository changed"),
        "{response}"
    );
    let audit = fs::read_to_string(data_dir.join("mcp-audit.jsonl")).unwrap();
    let last: Value = serde_json::from_str(audit.lines().last().unwrap()).unwrap();
    assert_eq!(last["outcome"], "rejected");
    assert_eq!(last["confirmation"], "elicitation");
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
