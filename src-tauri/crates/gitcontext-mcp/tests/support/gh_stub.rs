use std::{env, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => println!("gh test stub"),
        Some("api") if args.get(1).map(String::as_str) == Some("user") => {
            println!("fictional");
        }
        Some("repo") if args.get(1).map(String::as_str) == Some("view") => {
            if let Ok(branch) = env::var("GITCONTEXT_TEST_GH_DEFAULT_BRANCH") {
                println!("{branch}");
            } else {
                return ExitCode::FAILURE;
            }
        }
        Some("pr") if args.get(1).map(String::as_str) == Some("list") => println!("[]"),
        Some("pr") if args.get(1).map(String::as_str) == Some("view") => {
            println!("{{\"number\":1,\"url\":\"https://example.com/pr/1\",\"title\":\"Fixture PR\",\"state\":\"OPEN\",\"isDraft\":false,\"baseRefName\":\"main\",\"headRefName\":\"work\",\"headRefOid\":\"abcdef1234567890\",\"mergeable\":\"MERGEABLE\",\"mergeStateStatus\":\"CLEAN\"}}");
        }
        _ => return ExitCode::FAILURE,
    }
    ExitCode::SUCCESS
}
