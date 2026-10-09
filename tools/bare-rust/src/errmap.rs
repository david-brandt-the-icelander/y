//! Map a compiler diagnostic on a woven file back to the bare file and the constraint file (proposal 11, stage 3).
//!
//! A diagnostic's primary span is a byte range of the woven `.rs`. The span map recorded by `weave` says which ranges
//! are not copies of the bare text; such a range was printed from one slot, hole entry or `mut`, so the error is
//! reported at the place the slot sits in the `.brs` file and at the JSON pointer of the slot. Any other range is a
//! copy of the bare body, and the error is reported at its position in the `.brs` file alone.
use crate::cargo::Diag;
use crate::crate_weave::{CrateWeave, HoleReport, WovenFile};
use crate::trace::{Kind, Origin};
use serde_json::{json, Value};

/// A constraint slot a diagnostic touches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Related {
    /// The label rustc put on the span (empty for the primary span).
    pub label: String,
    pub bare_line: usize,
    pub bare_col: usize,
    /// "slot", "hole" or "mut".
    pub origin: &'static str,
    /// JSON pointer into the constraint file, `None` for a `mut`.
    pub json_path: Option<String>,
}

#[derive(Clone, Debug)]
pub struct MappedError {
    pub level: String,
    pub code: String,
    pub message: String,
    pub woven_file: String,
    pub woven_line: usize,
    pub woven_col: usize,
    /// The `.brs` file (relative to the crate) and the position in it; `None` when the file is not woven.
    pub bare_file: Option<String>,
    pub bare_line: usize,
    pub bare_col: usize,
    /// The constraint file beside the bare file.
    pub constraints_file: Option<String>,
    /// "slot", "hole" or "mut" when the primary span lies in woven-in text; "body" when it lies in the bare body;
    /// "unwoven" for a file the weave does not own, "none" for a diagnostic without a span.
    pub origin: &'static str,
    /// The JSON pointer of the slot or hole entry that produced the primary span's text.
    pub json_path: Option<String>,
    /// Secondary spans of the diagnostic that lie in woven-in text ("expected due to this").
    pub related: Vec<Related>,
}

/// One JSON pointer token.
fn esc(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

/// `params[1].type` is `/params/1/type`.
fn slot_pointer(slot: &str) -> String {
    let mut out = String::new();
    let mut tok = String::new();
    let flush = |tok: &mut String, out: &mut String| {
        if !tok.is_empty() {
            out.push('/');
            out.push_str(&esc(tok));
            tok.clear();
        }
    };
    for c in slot.chars() {
        match c {
            '.' | '[' => flush(&mut tok, &mut out),
            ']' => flush(&mut tok, &mut out),
            c => tok.push(c),
        }
    }
    flush(&mut tok, &mut out);
    out
}

/// The JSON pointer, in the file's constraint JSON, of the entry an origin names.
pub fn json_path(json: &Value, o: &Origin) -> Option<String> {
    match o.kind {
        Kind::Mut => None,
        Kind::Slot => {
            let i = json["subjects"].as_array()?.iter().position(|s| s["name"] == o.subject.as_str())?;
            Some(format!("/subjects/{i}{}", slot_pointer(&o.slot)))
        }
        Kind::Hole => {
            let base = format!("/holes/{}", esc(&o.subject));
            let h = &json["holes"][o.subject.as_str()];
            if let Some(key) = o.slot.strip_prefix("locals[").and_then(|s| s.strip_suffix(']')) {
                return Some(format!("{base}/locals/{}", esc(key)));
            }
            let inner = o.slot.strip_prefix("hints[")?.strip_suffix(']')?;
            let (kind, n) = inner.split_once(" #")?;
            let n: u64 = n.parse().ok()?;
            let at = h["hints"].as_array()?.iter().position(|x| x["kind"] == kind && x["n"] == n)?;
            Some(format!("{base}/hints/{at}"))
        }
    }
}

fn origin_name(o: &Origin) -> &'static str {
    match o.kind {
        Kind::Slot => "slot",
        Kind::Hole => "hole",
        Kind::Mut => "mut",
    }
}

fn base(d: &Diag) -> MappedError {
    MappedError {
        level: d.level.clone(),
        code: d.code.clone().unwrap_or_default(),
        message: d.message.clone(),
        woven_file: d.file.clone(),
        woven_line: d.line,
        woven_col: d.col,
        bare_file: None,
        bare_line: 0,
        bare_col: 0,
        constraints_file: None,
        origin: "none",
        json_path: None,
        related: vec![],
    }
}

/// Map one diagnostic. `wf` is the woven file the diagnostic is in, if the weave owns it.
pub fn map_diag(wf: Option<&WovenFile>, d: &Diag) -> MappedError {
    let mut m = base(d);
    if d.file.is_empty() {
        return m;
    }
    let Some(wf) = wf else {
        m.origin = "unwoven";
        return m;
    };
    let map = &wf.woven.map;
    m.bare_file = Some(wf.bare_name());
    m.constraints_file = Some(wf.rel.with_extension("constraints.json").display().to_string());
    match map.touching(d.start, d.end).first() {
        Some(sp) => {
            let (l, c) = wf.woven.bare_at(sp.bare_start);
            (m.bare_line, m.bare_col) = (l, c);
            m.origin = origin_name(&sp.origin);
            m.json_path = json_path(&wf.json, &sp.origin);
        }
        None => {
            let (l, c) = wf.woven.bare_at(map.to_bare(d.start, false));
            (m.bare_line, m.bare_col) = (l, c);
            m.origin = "body";
        }
    }
    for (_, s, e, label) in d.secondary.iter().filter(|x| x.0 == d.file) {
        let (s, e, label) = (*s, *e, label.clone());
        if let Some(sp) = map.touching(s, e).first() {
            let (l, c) = wf.woven.bare_at(sp.bare_start);
            m.related.push(Related { label, bare_line: l, bare_col: c, origin: origin_name(&sp.origin), json_path: json_path(&wf.json, &sp.origin) });
        }
    }
    m
}

/// An undetermined hole, as a mapped error: the hole entry that would fill it.
pub fn map_hole(files: &[WovenFile], h: &HoleReport) -> MappedError {
    let wf = files.iter().find(|f| f.bare_name() == h.file);
    let json_path = wf.and_then(|wf| {
        let o = Origin {
            kind: Kind::Hole,
            subject: h.subject.clone(),
            slot: match &h.key {
                Some(k) => format!("locals[{k}]"),
                None => format!("hints[{} #{}]", h.kind, h.n),
            },
        };
        // The entry was dropped or never written: point at where it would go.
        json_path(&wf.json, &o).or_else(|| {
            let base = format!("/holes/{}", esc(&h.subject));
            Some(match &h.key {
                Some(k) => format!("{base}/locals/{}", esc(k)),
                None => format!("{base}/hints/-"),
            })
        })
    });
    MappedError {
        level: "error".into(),
        code: h.code.clone(),
        message: format!("hole undetermined: {} of `{}`; fill with: {}", h.key.as_deref().map_or(format!("{} #{}", h.kind, h.n), |k| format!("local `{k}`")), h.subject, h.clause),
        woven_file: h.file.replace(".brs", ".rs"),
        woven_line: 0,
        woven_col: 0,
        bare_file: Some(h.file.clone()),
        bare_line: h.line,
        bare_col: h.col,
        constraints_file: Some(h.file.replace(".brs", ".constraints.json")),
        origin: "hole",
        json_path,
        related: vec![],
    }
}

impl MappedError {
    /// `src/lib.brs:12:5 [constraints: /subjects/3/params/1/type] error[E0308]: mismatched types`
    pub fn show(&self) -> String {
        let at = match &self.bare_file {
            Some(f) => format!("{f}:{}:{}", self.bare_line, self.bare_col),
            None if self.woven_file.is_empty() => "(no source span)".to_string(),
            None => format!("{}:{}:{} (not a woven file)", self.woven_file, self.woven_line, self.woven_col),
        };
        let mut s = at;
        match (&self.json_path, self.origin) {
            (Some(p), _) => s.push_str(&format!(" [constraints: {p}]")),
            (None, "mut") => s.push_str(" [mut derived through rustc]"),
            _ => {}
        }
        let code = if self.code.is_empty() { String::new() } else { format!("[{}]", self.code) };
        s.push_str(&format!(" {}{code}: {}", self.level, self.message));
        for r in &self.related {
            if let Some(p) = &r.json_path {
                s.push_str(&format!("\n    related [constraints: {p}] at {}:{}{}", r.bare_line, r.bare_col, if r.label.is_empty() { String::new() } else { format!(": {}", r.label) }));
            }
        }
        s
    }

    pub fn to_json(&self) -> Value {
        json!({
            "level": self.level,
            "code": self.code,
            "message": self.message,
            "origin": self.origin,
            "woven": {"file": self.woven_file, "line": self.woven_line, "column": self.woven_col},
            "bare": self.bare_file.as_ref().map(|f| json!({"file": f, "line": self.bare_line, "column": self.bare_col})),
            "constraints": self.constraints_file.as_ref().map(|f| json!({"file": f, "path": self.json_path})),
            "related": self.related.iter().map(|r| json!({
                "label": r.label, "origin": r.origin, "bare": {"line": r.bare_line, "column": r.bare_col}, "path": r.json_path
            })).collect::<Vec<_>>(),
        })
    }
}

/// Every error of a woven crate, mapped: undetermined holes first, then the other errors, then those that follow
/// from a hole.
pub fn map_crate(cw: &CrateWeave) -> Vec<MappedError> {
    let mut v: Vec<MappedError> = cw.holes.iter().map(|h| map_hole(&cw.files, h)).collect();
    v.extend(cw.errors.iter().chain(&cw.follow_on).map(|e| e.mapped.clone()));
    v
}
