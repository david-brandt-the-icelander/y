//! Derive templates (language 17, question 15): what `#[derive(T)]` promises is a template, expanded for each type that
//! derives it into ordinary `ensure` clauses, one per field. A type subject names its derives (`"derive": ["Clone"]`)
//! and has `fields` (or `variants`, whose fields count); the macro subject `std::clone::Clone!` carries `derive Clone`.
use serde_json::{json, Value};

/// The templates: name, the clause name and the clause text for one field `{f}`.
/// `self` is the deriving value, `other` another of the type, `result` what the trait's method returns.
pub const TEMPLATES: &[(&str, &str, &str)] = &[
    ("Clone", "clone_{f}", "ensure clone_{f} : result.{f} == self.{f}"),
    ("Copy", "copy_{f}", "ensure copy_{f} : result.{f} == self.{f}"),
    ("PartialEq", "eq_{f}", "ensure eq_{f} : self.{f} != other.{f} implies not result"),
    ("Eq", "refl_{f}", "law refl_{f} : all x : Self : x.{f} == x.{f}"),
    ("Hash", "hash_{f}", "ensure hash_{f} : self.{f} == other.{f} implies 'std::hash::Hash::hash'(self.{f}) == 'std::hash::Hash::hash'(other.{f})"),
    ("Default", "default_{f}", "ensure default_{f} : result.{f} == 'std::default::Default::default'()"),
];

pub fn is_template(name: &str) -> bool {
    TEMPLATES.iter().any(|t| t.0 == name)
}

/// The names of the fields of a type subject, of a struct or of any variant of an enum, in order and without repeats.
pub fn field_names(sub: &Value) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let lists = sub["fields"].as_array().into_iter().chain(sub["variants"].as_array().into_iter().flatten().filter_map(|v| v["fields"].as_array()));
    for f in lists.flatten().filter_map(|f| f["name"].as_str()) {
        if !out.iter().any(|o| o == f) {
            out.push(f.to_string());
        }
    }
    out
}

/// The clauses a subject's `derive` list expands to: for each template, for each field, one `ensure` clause.
pub fn expand(sub: &Value) -> Vec<Value> {
    let fields = field_names(sub);
    let mut out = vec![];
    for d in sub["derive"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        let Some((_, _, text)) = TEMPLATES.iter().find(|t| t.0 == d) else { continue };
        for f in &fields {
            let text = text.replace("{f}", f);
            let kind = text.split_whitespace().next().unwrap_or("ensure");
            out.push(json!({ "kind": kind, "text": text, "source": format!("derive template {d}"), "status": { "assumed": "hand-written" } }));
        }
    }
    out
}

/// The expansion of every deriving subject of a library file, as text: `subject`, then its clauses.
pub fn show(file: &Value) -> String {
    let mut s = String::new();
    for sub in file["subjects"].as_array().into_iter().flatten().filter(|s| s["derive"].is_array()) {
        s.push_str(&format!("{} {}: derive {}\n", sub["kind"].as_str().unwrap_or(""), sub["name"].as_str().unwrap_or(""), sub["derive"]));
        for c in expand(sub) {
            s.push_str(&format!("  {}\n", c["text"].as_str().unwrap_or("")));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_expands_per_field() {
        let t = json!({"kind":"type","name":"c::Cache","derive":["Clone","Eq"],"fields":[{"name":"entries"},{"name":"hits"}]});
        let texts: Vec<String> = expand(&t).iter().map(|c| c["text"].as_str().unwrap().to_string()).collect();
        assert_eq!(texts.len(), 4);
        assert_eq!(texts[0], "ensure clone_entries : result.entries == self.entries");
        assert!(texts[3].starts_with("law refl_hits"));
        assert!(show(&json!({"subjects":[t]})).contains("type c::Cache: derive [\"Clone\",\"Eq\"]"));
    }
}
