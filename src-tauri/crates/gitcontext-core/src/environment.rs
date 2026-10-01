use crate::{
    git_ops,
    models::{EnvironmentStatus, ToolStatus},
};
use std::process::Command;

pub fn environment_status() -> EnvironmentStatus {
    let git = probe_tool("git", &["--version"]);
    let gh = probe_tool("gh", &["--version"]);
    let ssh = probe_tool("ssh", &["-V"]);
    let ssh_directory = git_ops::home_directory()
        .map(|home| home.join(".ssh"))
        .filter(|path| path.is_dir())
        .map(|path| path.to_string_lossy().into_owned());
    EnvironmentStatus {
        git,
        gh,
        ssh,
        ssh_directory,
    }
}

fn probe_tool(program: &str, arguments: &[&str]) -> ToolStatus {
    match Command::new(program).args(arguments).output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let first_line = if stdout.is_empty() { stderr } else { stdout }
                .lines()
                .next()
                .unwrap_or_default()
                .to_string();
            ToolStatus {
                available: true,
                version: (!first_line.is_empty()).then_some(first_line),
                detail: None,
            }
        }
        Ok(output) => ToolStatus {
            available: false,
            version: None,
            detail: Some(format!("{program} exited with {}", output.status)),
        },
        Err(error) => ToolStatus {
            available: false,
            version: None,
            detail: Some(format!("{program} was not found: {error}")),
        },
    }
}

pub fn command_available(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}
