//! Running cargo on a woven crate and reading its JSON diagnostics.
use crate::R;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A compiler or clippy diagnostic, located in a file of the crate (relative path) by byte offsets.
#[derive(Clone, Debug)]
pub struct Diag {
    pub level: String,
    pub code: Option<String>,
    pub message: String,
    /// Relative to the crate directory; empty when the diagnostic has no span in the crate.
    pub file: String,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub col: usize,
    pub rendered: String,
    /// Suggested replacements: (file, start, end, replacement, applicability).
    pub fixes: Vec<Fix>,
    /// The other spans of the diagnostic that lie in the crate: (file, start, end, label).
    pub secondary: Vec<(String, usize, usize, String)>,
}

#[derive(Clone, Debug)]
pub struct Fix {
    pub file: String,
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    pub applicability: String,
}

pub struct Run {
    pub diags: Vec<Diag>,
    pub success: bool,
    /// Cargo's own stderr, without the compiler's diagnostics: why a build did not start.
    pub stderr: String,
}

impl Diag {
    pub fn is_error(&self) -> bool {
        self.level == "error" || self.level.starts_with("error:")
    }

    /// The key that identifies a diagnostic across targets and across the original and woven crate.
    pub fn key(&self) -> String {
        format!("{}|{}|{}", self.code.as_deref().unwrap_or(""), self.message, self.file)
    }
}

/// Where cargo puts its build output. One shared directory keeps dependencies built once.
pub fn target_dir(base: &Path) -> PathBuf {
    std::env::var_os("BARE_RUST_TARGET").map(PathBuf::from).unwrap_or_else(|| base.join("target/bare-rust-cargo"))
}

fn command(dir: &Path, target: &Path, args: &[&str]) -> Command {
    let mut c = Command::new("cargo");
    c.args(args).current_dir(dir).env("CARGO_TARGET_DIR", target).env("CARGO_TERM_COLOR", "never");
    // strsim's benches need `#![feature(test)]`: compile them on stable the same way for the original and the woven.
    c.env("RUSTC_BOOTSTRAP", "1");
    c
}

fn local(span: &Value, dir: &Path) -> Option<Value> {
    let mut s = span.clone();
    for _ in 0..16 {
        let f = s["file_name"].as_str()?;
        let p = Path::new(f);
        if p.is_relative() && dir.join(p).is_file() {
            return Some(s);
        }
        s = s["expansion"]["span"].clone();
        if s.is_null() {
            return None;
        }
    }
    None
}

fn fixes(m: &Value, dir: &Path, out: &mut Vec<Fix>) {
    for sp in m["spans"].as_array().into_iter().flatten() {
        if let (Some(r), Some(f)) = (sp["suggested_replacement"].as_str(), sp["file_name"].as_str()) {
            if Path::new(f).is_relative() && dir.join(f).is_file() {
                out.push(Fix {
                    file: f.to_string(),
                    start: sp["byte_start"].as_u64().unwrap_or(0) as usize,
                    end: sp["byte_end"].as_u64().unwrap_or(0) as usize,
                    replacement: r.to_string(),
                    applicability: sp["suggestion_applicability"].as_str().unwrap_or("").to_string(),
                });
            }
        }
    }
    for c in m["children"].as_array().into_iter().flatten() {
        fixes(c, dir, out);
    }
}

fn parse(stdout: &str, dir: &Path) -> Vec<Diag> {
    let mut out: Vec<Diag> = vec![];
    for line in stdout.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if v["reason"] != "compiler-message" {
            continue;
        }
        let m = &v["message"];
        let message = m["message"].as_str().unwrap_or("").to_string();
        let spans = m["spans"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        if spans.is_empty() && (message.starts_with("aborting due to") || message.contains("emitted") || message.starts_with("could not compile")) {
            continue;
        }
        let primary = spans.iter().find(|s| s["is_primary"] == true).or(spans.first());
        let loc = primary.and_then(|p| local(p, dir));
        let mut fx = vec![];
        fixes(m, dir, &mut fx);
        let d = Diag {
            level: m["level"].as_str().unwrap_or("").to_string(),
            code: m["code"]["code"].as_str().map(str::to_string),
            message,
            file: loc.as_ref().and_then(|s| s["file_name"].as_str()).unwrap_or("").to_string(),
            start: loc.as_ref().and_then(|s| s["byte_start"].as_u64()).unwrap_or(0) as usize,
            end: loc.as_ref().and_then(|s| s["byte_end"].as_u64()).unwrap_or(0) as usize,
            line: loc.as_ref().and_then(|s| s["line_start"].as_u64()).unwrap_or(0) as usize,
            col: loc.as_ref().and_then(|s| s["column_start"].as_u64()).unwrap_or(0) as usize,
            rendered: m["rendered"].as_str().unwrap_or("").to_string(),
            fixes: fx,
            secondary: spans
                .iter()
                .filter(|s| s["is_primary"] != true)
                .filter_map(|s| local(s, dir))
                .map(|s| (s["file_name"].as_str().unwrap_or("").to_string(), s["byte_start"].as_u64().unwrap_or(0) as usize, s["byte_end"].as_u64().unwrap_or(0) as usize, s["label"].as_str().unwrap_or("").to_string()))
                .collect(),
        };
        let dup = out.iter().any(|o| o.key() == d.key() && o.start == d.start && o.end == d.end);
        if !dup {
            out.push(d);
        }
    }
    out
}

/// `cargo check` or `cargo clippy` with `--all-targets`, reading the JSON diagnostics.
pub fn diagnostics(dir: &Path, target: &Path, sub: &str) -> R<Run> {
    let out = command(dir, target, &[sub, "--all-targets", "--message-format=json"])
        .output()
        .map_err(|e| format!("cargo {sub}: {e}"))?;
    Ok(Run {
        diags: parse(&String::from_utf8_lossy(&out.stdout), dir),
        success: out.status.success(),
        stderr: String::from_utf8_lossy(&out.stderr).lines().filter(|l| l.starts_with("error:") && !l.contains("could not compile")).collect::<Vec<_>>().join("\n"),
    })
}

/// `cargo test`: whether it passed, and the summary lines.
pub fn test(dir: &Path, target: &Path) -> R<(bool, String)> {
    let out = command(dir, target, &["test"]).output().map_err(|e| format!("cargo test: {e}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let summary: Vec<&str> = text.lines().filter(|l| l.starts_with("test result:") || l.contains("FAILED") || l.starts_with("error")).collect();
    Ok((out.status.success(), summary.join("\n")))
}
