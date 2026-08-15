use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::permissions::Permissions;

pub async fn verify_reference_code(
    language: &str,
    source: &str,
    permissions: &Permissions,
) -> Value {
    let language = language.to_ascii_lowercase();
    if !permissions
        .code_languages
        .iter()
        .any(|item| item.eq_ignore_ascii_case(&language))
    {
        return json!({
            "result": "NOT_TESTED",
            "reason": format!("language `{language}` is not on the allowlist"),
        });
    }
    if source.trim().is_empty() {
        return json!({
            "result": "NOT_TESTED",
            "reason": "no reference implementation was supplied",
        });
    }
    if language != "python" {
        return json!({
            "result": "NOT_TESTED",
            "reason": "only python compile checks are available in v1",
        });
    }

    let python = which("python").or_else(|| which("python3"));
    let Some(python) = python else {
        return json!({
            "result": "NOT_TESTED",
            "reason": "python interpreter is not available on this host",
        });
    };

    let mut child = match Command::new(python)
        .arg("-I")
        .arg("-c")
        .arg("import sys, ast; ast.parse(sys.stdin.read()); print('SYNTAX_OK')")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return json!({
                "result": "NOT_TESTED",
                "reason": format!("could not spawn interpreter: {error}"),
            });
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(source.as_bytes()).await;
    }

    let timeout = Duration::from_secs(permissions.code_timeout_seconds.max(1));
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = truncate(&stdout, permissions.code_output_limit);
            let stderr = truncate(&stderr, permissions.code_output_limit);
            if output.status.success() && stdout.contains("SYNTAX_OK") {
                json!({
                    "result": "PASS",
                    "reason": "python ast.parse accepted the reference implementation",
                    "stdout": stdout,
                    "stderr": stderr,
                })
            } else {
                json!({
                    "result": "FAIL",
                    "reason": "interpreter rejected the reference implementation",
                    "stdout": stdout,
                    "stderr": stderr,
                })
            }
        }
        Ok(Err(error)) => json!({
            "result": "NOT_TESTED",
            "reason": format!("interpreter error: {error}"),
        }),
        Err(_) => {
            json!({
                "result": "FAIL",
                "reason": "code verifier timed out",
            })
        }
    }
}

fn truncate(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        text.to_owned()
    } else {
        format!("{}…", &text[..limit])
    }
}

fn which(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate.to_string_lossy().into_owned());
        }
        let exe = dir.join(format!("{name}.exe"));
        if exe.exists() {
            return Some(exe.to_string_lossy().into_owned());
        }
    }
    None
}
