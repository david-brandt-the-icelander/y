//! The lowering of the declarations no example uses: enumerations, states and their transitions, traits and their
//! implementations, units, commands with their policies, native functions and types, and kinds of collection.
use crate::lower::{Form, Lowerer, Var};
use crate::model::{FnInfo, TypeDef};
use crate::ty::{pascal, Ty};
use y_parse::Node;

/// A duration literal of the policy, as Rust: `10ms` is `Duration::from_millis(10)`.
fn duration(text: &str) -> String {
    let digits: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();
    let n: u64 = digits.parse().unwrap_or(0);
    match &text[digits.len()..] {
        "ms" => format!("Duration::from_millis({n})"),
        "s" => format!("Duration::from_secs({n})"),
        "min" => format!("Duration::from_secs({})", n * 60),
        "h" => format!("Duration::from_secs({})", n * 3600),
        "d" => format!("Duration::from_secs({})", n * 86400),
        _ => format!("Duration::from_millis({n})"),
    }
}

impl<'a> Lowerer<'a> {
    /// `enumeration name [is type]:`: a Rust enum ordered as written, with `ALL`, the members in order, and `value()`.
    pub fn enumeration_item(&mut self, name: &str, vis: &str) {
        let Some(TypeDef::Enumeration { items, value }) = self.m.types.get(name).map(|t| t.def.clone()) else { return };
        let en = pascal(name);
        self.line("#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]");
        self.line(format!("{vis}enum {en} {{"));
        for (n, _) in &items {
            self.line(format!("    {},", pascal(n)));
        }
        self.line("}");
        self.line("");
        // the values: as given, or numbered from zero in the order written, in the smallest unsigned integer that holds them
        let given_text = matches!(value, Some(Ty::Str));
        let numbers: Vec<i128> = items.iter().enumerate().map(|(k, (_, v))| v.as_ref().and_then(|v| v.str("text")).and_then(|t| t.parse().ok()).unwrap_or(k as i128)).collect();
        let vt = if given_text {
            "&'static str".to_string()
        } else {
            match value.as_ref() {
                Some(t) if *t != Ty::Unknown => self.rust_type(t),
                _ => crate::ty::range_digits(0, *numbers.iter().max().unwrap_or(&0)).replace('i', "u"),
            }
        };
        self.line(format!("impl {en} {{"));
        self.line(format!("    /// Every member, in order: what `for … in {name}` goes over."));
        let all: Vec<String> = items.iter().map(|(n, _)| format!("{en}::{}", pascal(n))).collect();
        self.line(format!("    pub const ALL: [{en}; {}] = [{}];", items.len(), all.join(", ")));
        self.line("");
        self.line("    /// The member's value.");
        self.line(format!("    pub fn value(self) -> {vt} {{"));
        self.line("        match self {");
        for (k, (n, v)) in items.iter().enumerate() {
            let val = if given_text { format!("\"{}\"", v.as_ref().and_then(|v| v.str("value")).unwrap_or(n)) } else { numbers[k].to_string() };
            self.line(format!("            {en}::{} => {val},", pascal(n)));
        }
        self.line("        }");
        self.line("    }");
        self.line("}");
        self.line("");
        self.line(format!("impl std::fmt::Display for {en} {{"));
        self.line("    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {");
        self.line("        write!(f, \"{}\", self.value())");
        self.line("    }");
        self.line("}");
    }

    /// `state name:`: a structure of the shared fields, generic over its state, `S`; a module of the states, each a
    /// structure of its own fields; the state not known (`NameState`, the default), into which each converts; `becomes`,
    /// which moves a value to another state; and `known`, which recovers a value's state.
    pub fn state_item(&mut self, name: &str, vis: &str) {
        let Some(TypeDef::State { fields, cases }) = self.m.types.get(name).map(|t| t.def.clone()) else { return };
        let (en, any, known) = (pascal(name), format!("{}State", pascal(name)), format!("{}Known", pascal(name)));
        self.line("#[derive(Clone, Debug, PartialEq)]");
        self.line(format!("{vis}struct {en}<S = {any}> {{"));
        for (f, t) in &fields {
            let rt = self.rust_type(t);
            self.line(format!("    pub {f}: {rt},"));
        }
        self.line("    pub state: S,");
        self.line("}");
        self.line("");
        self.line(format!("/// The states of `{name}`, each with the fields only it has."));
        self.line(format!("{vis}mod {name} {{"));
        self.line("    use super::*;");
        for (c, fs) in &cases {
            self.line("");
            self.line("    #[derive(Clone, Debug, PartialEq)]");
            if fs.is_empty() {
                self.line(format!("    pub struct {};", pascal(c)));
            } else {
                let parts: Vec<String> = fs.iter().map(|(f, t)| format!("pub {f}: {}", self.module().rust_type(t))).collect();
                self.line(format!("    pub struct {} {{ {} }}", pascal(c), parts.join(", ")));
            }
        }
        self.line("}");
        self.line("");
        self.line(format!("/// A `{name}` whose state its type does not say: one of its states."));
        self.line("#[derive(Clone, Debug, PartialEq)]");
        self.line(format!("{vis}enum {any} {{"));
        for (c, _) in &cases {
            self.line(format!("    {}({name}::{}),", pascal(c), pascal(c)));
        }
        self.line("}");
        for (c, _) in &cases {
            self.line("");
            self.line(format!("impl From<{name}::{}> for {any} {{", pascal(c)));
            self.line(format!("    fn from(state: {name}::{}) -> {any} {{", pascal(c)));
            self.line(format!("        {any}::{}(state)", pascal(c)));
            self.line("    }");
            self.line("}");
        }
        self.line("");
        self.line(format!("/// A `{name}` whose state is recovered: what a `conditional` on its state narrows it to."));
        self.line(format!("{vis}enum {known} {{"));
        for (c, _) in &cases {
            self.line(format!("    {}({en}<{name}::{}>),", pascal(c), pascal(c)));
        }
        self.line("}");
        let names: Vec<&str> = fields.iter().map(|(f, _)| f.as_str()).collect();
        let carried = |state: &str| {
            let mut parts: Vec<String> = names.iter().map(|f| format!("{f}: self.{f}")).collect();
            parts.push(format!("state: {state}"));
            parts.join(", ")
        };
        self.line("");
        self.line(format!("impl<S> {en}<S> {{"));
        self.line("    /// Y's `becomes`: the value in another state, its shared fields kept. The value it was is moved, so it");
        self.line("    /// cannot be used afterwards (Y § 3.11).");
        self.line(format!("    pub fn becomes<T>(self, state: T) -> {en}<T> {{"));
        self.line(format!("        {en} {{ {} }}", carried("state")));
        self.line("    }");
        self.line("}");
        self.line("");
        self.line(format!("impl<S: Into<{any}>> {en}<S> {{"));
        self.line("    /// The value, its state no longer said by its type.");
        self.line(format!("    pub fn forget(self) -> {en}<{any}> {{"));
        self.line(format!("        {en} {{ {} }}", carried("self.state.into()")));
        self.line("    }");
        self.line("}");
        self.line("");
        self.line(format!("impl {en}<{any}> {{"));
        self.line("    /// The value, its state recovered.");
        self.line(format!("    pub fn known(self) -> {known} {{"));
        self.line("        match self.state {");
        for (c, _) in &cases {
            self.line(format!("            {any}::{}(state) => {known}::{}({en} {{ {} }}),", pascal(c), pascal(c), carried("state")));
        }
        self.line("        }");
        self.line("    }");
        self.line("}");
    }

    /// `command name:` and its `policy:`: a structure of its fields, and its `Command` implementation: the policy, and
    /// the key duplicates collapse on.
    pub fn command_item(&mut self, name: &str, vis: &str) {
        let Some(TypeDef::Command { fields, policy }) = self.m.types.get(name).map(|t| t.def.clone()) else { return };
        let en = pascal(name);
        self.line("#[derive(Clone, Debug, PartialEq)]");
        self.line(format!("{vis}struct {en} {{"));
        for (f, t) in &fields {
            let rt = self.rust_type(t);
            self.line(format!("    pub {f}: {rt},"));
        }
        self.line("}");
        let entry = |k: &str| policy.nodes("entries").into_iter().find(|e| e.str("name") == Some(k)).and_then(|e| e.node("value")).cloned();
        // every field of a policy is a decision someone has to make (Y § 3.10)
        for k in ["deduplicate_by", "idempotent", "retry", "deadline", "on_abandon"] {
            if entry(k).is_none() {
                self.error(format!("command `{name}`'s policy has no `{k}`: a policy is not optional, because every field of it is a decision (Y § 3.10)"));
            }
        }
        let dedupe: Vec<String> = match entry("deduplicate_by") {
            Some(n) if n.kind == "name" => vec![n.str("name").unwrap().to_string()],
            Some(n) if n.kind == "list" => n.nodes("items").iter().filter_map(|i| i.str("name").map(str::to_string)).collect(),
            _ => vec![],
        };
        for d in &dedupe {
            if !fields.iter().any(|(f, _)| f == d) {
                self.error(format!("command `{name}` deduplicates by `{d}`, which is not one of its fields"));
            }
        }
        let retry = entry("retry");
        let rarg = |k: &str| retry.as_ref().and_then(|r| r.nodes("args").into_iter().find(|a| a.str("name") == Some(k))).and_then(|a| a.node("value")).and_then(|v| v.str("text")).map(str::to_string);
        let first = rarg("first").map(|t| duration(&t)).unwrap_or_else(|| "Duration::from_millis(0)".into());
        let factor = rarg("factor").unwrap_or_else(|| "1".into());
        let attempts = rarg("attempts").unwrap_or_else(|| "1".into());
        let deadline = entry("deadline").and_then(|d| d.str("text").map(duration)).unwrap_or_else(|| "Duration::MAX".into());
        let idempotent = entry("idempotent").and_then(|i| i.str("text").map(str::to_string)).unwrap_or_else(|| "false".into());
        let abandon = match entry("on_abandon") {
            Some(n) if n.kind == "compensate" => "OnAbandon::Compensate",
            _ => "OnAbandon::Park",
        };
        self.line("");
        self.line(format!("/// `{name}`'s delivery policy, and the key its duplicates collapse on (Y § 3.10)."));
        self.line(format!("impl Command for {en} {{"));
        self.line("    fn policy() -> Policy {");
        self.line(format!("        Policy {{ attempts: {attempts}, first: {first}, factor: {factor}, deadline: {deadline}, idempotent: {idempotent}, on_abandon: {abandon} }}"));
        self.line("    }");
        self.line("");
        self.line("    fn key(&self) -> String {");
        let keys: Vec<String> = dedupe.iter().map(|d| format!("self.{d}.to_string()")).collect();
        self.line(format!("        [{}].join(\"/\")", keys.join(", ")));
        self.line("    }");
        self.line("}");
    }

    /// `type name native because "…"`: the type of the module's native implementation, by the same name.
    pub fn native_type_item(&mut self, name: &str, vis: &str) {
        let because = match self.m.types.get(name).map(|t| &t.def) { Some(TypeDef::Native { because }) => because.clone(), _ => String::new() };
        self.line(format!("{vis}use crate::{}::native::{}; // y: native because \"{because}\"", self.m.name, pascal(name)));
    }

    /// `trait name:`: a Rust trait of its methods' signatures, each taking `me` as `&self` (or `&mut self`).
    pub fn trait_item(&mut self, d: &Node, vis: &str) {
        let name = d.str("name").unwrap();
        self.line(format!("{vis}trait {} {{", pascal(name)));
        self.ind += 1;
        let sigs = self.m.traits.get(name).cloned().unwrap_or_default();
        for (k, (x, f)) in d.nodes("decls").iter().zip(&sigs).enumerate() {
            if k > 0 {
                self.line("");
            }
            for l in crate::emit::doc_lines_of(x, Some(f)) {
                self.line(l);
            }
            let mut f = f.clone();
            f.me = Some((String::new(), x.flag("mutable")));
            let sig = self.signature(&f, "");
            self.line(format!("{sig};"));
        }
        self.ind -= 1;
        self.line("}");
    }

    /// `implementation trait for type:`: `impl Trait for Type`, each method lowered as a function whose `me` is `self`.
    pub fn implementation_item(&mut self, d: &Node) {
        let (tr, ty) = (d.str("trait").unwrap(), d.str("type").unwrap());
        if !self.m.traits.contains_key(tr) {
            self.error(format!("`implementation {tr} for {ty}`: the module declares no trait `{tr}`"));
        }
        let sigs = self.m.traits.get(tr).cloned().unwrap_or_default();
        for f in d.nodes("decls") {
            let n = f.str("name").unwrap_or("");
            if !sigs.iter().any(|s| s.name == n) {
                self.error(format!("`implementation {tr} for {ty}`: `{n}` is not a method of `{tr}`"));
            }
        }
        for s in &sigs {
            if !d.nodes("decls").iter().any(|f| f.str("name") == Some(s.name.as_str())) {
                self.error(format!("`implementation {tr} for {ty}` does not implement `{}`", s.name));
            }
        }
        // for a structure that leaves types open, one implementation for every type it may hold, as its bounds allow
        let open = self.m.open.get(ty).cloned().unwrap_or_default();
        let params = self.generic_params(&open, true);
        let args = if open.is_empty() { String::new() } else { format!("<{}>", open.iter().map(|(g, _)| pascal(g)).collect::<Vec<_>>().join(", ")) };
        self.line(format!("impl{params} {} for {}{args} {{", pascal(tr), pascal(ty)));
        self.ind += 1;
        self.impl_generics = open;
        for (k, x) in d.nodes("decls").into_iter().enumerate() {
            if k > 0 {
                self.line("");
            }
            let Some(mut f) = self.m.methods.get(ty).and_then(|ms| ms.get(x.str("name").unwrap())).cloned() else { continue };
            // a method's own open types are the trait's: Rust takes an implementation's bounds only as the trait's
            if let Some(s) = sigs.iter().find(|s| s.name == f.name) {
                f.generics = s.generics.clone();
            }
            self.function_item(x, &f, "");
        }
        self.impl_generics = vec![];
        self.ind -= 1;
        self.line("}");
    }

    /// The name `me` in a method: `self`, as the method takes it.
    pub fn bind_me(&mut self, f: &FnInfo) {
        if let Some((t, mutable)) = &f.me {
            let form = if *mutable { Form::Mut } else { Form::Ref };
            self.scopes.last_mut().unwrap().insert("me".into(), Var { ty: Ty::Named(t.clone()), form, arm: None, alias: Some("self".into()) });
        }
    }
}

impl<'a> Lowerer<'a> {
    /// A workflow holds only what keeps it deterministic: stores, shared state, and where it submits and enqueues
    /// commands (Y § 3.10). Any other capability is a compile error.
    pub fn workflow_check(&mut self, f: &FnInfo) {
        for p in f.injected() {
            let ok = matches!(p.ty, Ty::Store(_) | Ty::Shared(_) | Ty::Submit(_));
            if !ok {
                self.error(format!("workflow `{}` holds `{}`: a workflow may hold `store`, `shared`, `submit` and `enqueue`, and no clock, random source or I/O capability (Y § 3.10)", f.name, p.name));
            }
        }
    }

    /// `old(e)` in an `ensures`: each `e` kept where the function starts, `let old_1 = e;`.
    pub fn bind_olds(&mut self, d: &Node) {
        self.olds.clear();
        let mut found: Vec<Node> = vec![];
        for w in d.nodes("ensures") {
            w.walk(&mut |n: &Node| {
                if n.kind == "old" {
                    let v = n.node("value").unwrap().clone();
                    if !found.iter().any(|f| crate::lower::y_text(f) == crate::lower::y_text(&v)) {
                        found.push(v);
                    }
                }
            });
        }
        for (k, v) in found.iter().enumerate() {
            let x = self.expr(v);
            let t = x.ty.clone();
            let code = self.owned(x, &t);
            let local = format!("old_{}", k + 1);
            self.line(format!("let {local} = {code}; // y: old({})", crate::lower::y_text(v)));
            self.olds.push((crate::lower::y_text(v), local, t));
        }
    }

    /// The local an `old(e)` reads.
    pub fn old_value(&mut self, e: &Node) -> crate::lower::Ex {
        let v = e.node("value").unwrap();
        let text = crate::lower::y_text(v);
        match self.olds.iter().find(|o| o.0 == text).cloned() {
            Some((_, local, t)) => crate::lower::Ex { code: local, ty: t, form: Form::Place, prec: 100 },
            None => {
                self.error(format!("`old({text})` is read in an `ensures`, and nowhere else (Y § 3.11)"));
                self.expr(v)
            }
        }
    }

    /// The statements no example uses: `assert`, `assume`, `allow`, `log`, `metric`, `enqueue`, and a `nested
    /// function`. `true` when the statement was one of them.
    pub fn kind_stmt(&mut self, st: &Node) -> bool {
        let module = self.ctx.module.clone();
        let here = format!("{module}::{}", self.ctx.name);
        match st.kind {
            "assert" => {
                let c = self.expr(st.node("condition").unwrap());
                self.use_("y_runtime::assertion");
                self.line(format!("assertion({}, \"{here}#{}\");", c.code, st.str("name").unwrap_or("")));
            }
            // a claim the prover may take as true: nothing runs, and the marker keeps it where it is
            "assume" => self.line(format!("// y: assume {} because \"{}\"", st.str("name").unwrap_or(""), st.str("because").unwrap_or(""))),
            "allow" => self.line(format!("// y: allow({}) because \"{}\"", st.str("code").unwrap_or(""), st.str("because").unwrap_or(""))),
            "log" => {
                let x = self.expr(st.node("value").unwrap());
                let msg = if x.ty.is_text() { self.as_str(x) } else { format!("&{}.to_string()", x.code) };
                self.line(format!("observe::log(\"{}\", \"{here}\", {msg});", st.str("level").unwrap_or("info")));
            }
            "metric" => {
                let x = self.expr(st.node("value").unwrap());
                self.line(format!("observe::metric(\"{here}\", \"{}\", {});", st.str("name").unwrap_or(""), x.code));
            }
            "enqueue" => self.enqueue(st),
            "nested" => {
                let f = st.node("function").unwrap().clone();
                self.local_function(&f, None);
            }
            _ => return false,
        }
        true
    }

    /// `enqueue c(…)` in an atomic region: the command added to the outbox of its type that the region holds, so that
    /// it is there if and only if the region commits (Y § 3.10).
    fn enqueue(&mut self, st: &Node) {
        let v = st.node("value").unwrap();
        let cmd = v.node("callee").and_then(|c| c.str("name")).unwrap_or("").to_string();
        if self.atomic_result.is_none() {
            self.error(format!("`enqueue {cmd}(…)` is outside an atomic region: a command is enqueued with the transaction it is in, and only there (Y § 3.10)"));
        }
        let outbox = self.scopes.iter().rev().flat_map(|s| s.iter()).find(|(_, var)| matches!(&var.ty, Ty::Outbox(c) if **c == Ty::Named(cmd.clone()))).map(|(_, var)| var.alias.clone().unwrap_or_default());
        let Some(outbox) = outbox else {
            self.error(format!("`enqueue {cmd}(…)`: no `enqueue<command_type={cmd}>` is in scope"));
            return;
        };
        let c = self.command_value(v);
        self.line(format!("{outbox}.add({c});"));
    }

    /// A command's value, `charge(order: o, amount: a)`: its structure.
    pub fn command_value(&mut self, v: &Node) -> String {
        let cmd = v.node("callee").and_then(|c| c.str("name")).unwrap_or("").to_string();
        let Some(TypeDef::Command { fields, .. }) = self.m.types.get(&cmd).or_else(|| self.lib.and_then(|l| l.types.get(&cmd))).map(|t| t.def.clone()) else {
            self.error(format!("`{cmd}` is not a command"));
            return String::new();
        };
        let args: Vec<(Option<&str>, &Node)> = v.nodes("args").into_iter().map(|a| (a.str("name"), a.node("value").unwrap())).collect();
        self.construct_value(&pascal(&cmd), &fields, &args)
    }

    /// `submit c(…)` in a workflow: the command delivered through the `submit<command_type=c>` in scope, and its
    /// outcome, `result<value_type=r, failure_type=undelivered<failure_type=e>>`.
    pub fn submit(&mut self, e: &Node) -> crate::lower::Ex {
        let v = e.node("value").unwrap();
        let cmd = v.node("callee").and_then(|c| c.str("name")).unwrap_or("").to_string();
        let found = self.scopes.iter().rev().flat_map(|s| s.iter()).find(|(_, var)| matches!(&var.ty, Ty::Submit(c) if **c == Ty::Named(cmd.clone()))).map(|(n, var)| (var.alias.clone().unwrap_or(n.clone()), var.clone()));
        let Some((name, _)) = found else {
            self.error(format!("`submit {cmd}(…)`: no `submit<command_type={cmd}>` is in scope"));
            return crate::lower::Ex { code: "()".into(), ty: Ty::Unknown, form: Form::Temp, prec: 100 };
        };
        let h = self.function(&cmd).cloned();
        let (r, f) = h.map(|h| (h.result.map(|r| r.1).unwrap_or(Ty::Unit), h.fails.unwrap_or(Ty::Unit))).unwrap_or((Ty::Unknown, Ty::Unknown));
        let c = self.command_value(v);
        self.use_("y_runtime::command::Undelivered");
        crate::lower::Ex { code: format!("{name}.submit({c})"), ty: Ty::Res(Box::new(r), Box::new(Ty::Undelivered(Box::new(f)))), form: Form::Temp, prec: 100 }
    }

    /// `submit.to(handler)`: the capability that delivers a command to its handler, the handler's own capabilities
    /// taken from the scope it is made in, with the compensation its policy's `on_abandon` names.
    pub fn submit_to(&mut self, handler: &str) -> crate::lower::Ex {
        let Some(h) = self.function(handler).cloned() else {
            self.error(format!("`submit.to({handler})`: no handler `{handler}`"));
            return crate::lower::Ex { code: "()".into(), ty: Ty::Unknown, form: Form::Temp, prec: 100 };
        };
        let ct = pascal(handler);
        let call = |l: &mut Self, f: &FnInfo, arg: &str| -> String {
            let mut parts = vec![arg.to_string()];
            for p in f.injected() {
                parts.push(match l.var(&p.name).map(|v| v.form) {
                    Some(Form::Ref | Form::Mut) => p.name.clone(),
                    _ if p.mode == crate::model::Mode::Mut => format!("&mut {}", p.name),
                    _ => format!("&{}", p.name),
                });
            }
            format!("{}({})", f.name, parts.join(", "))
        };
        let body = call(self, &h, "command");
        let body = if h.fails.is_some() { body } else { format!("Ok::<_, ()>({body})") };
        // `on_abandon: compensate c(field: field)`: the compensating command, built from this one's fields, delivered
        // to its handler
        let mut compensation = "None".to_string();
        if let Some(TypeDef::Command { fields, policy }) = self.m.types.get(handler).or_else(|| self.lib.and_then(|l| l.types.get(handler))).map(|t| t.def.clone()) {
            let comp = policy.nodes("entries").into_iter().find(|e| e.str("name") == Some("on_abandon")).and_then(|e| e.node("value")).filter(|v| v.kind == "compensate").and_then(|v| v.node("value")).cloned();
            if let Some(cv) = comp {
                let other = cv.node("callee").and_then(|c| c.str("name")).unwrap_or("").to_string();
                self.push_scope();
                for (f, t) in &fields {
                    let form = if self.is_copy_ty(t) { Form::Place } else { Form::Borrowed };
                    self.scopes.last_mut().unwrap().insert(f.clone(), Var { ty: t.clone(), form, arm: None, alias: Some(format!("command.{f}")) });
                }
                let value = self.command_value(&cv);
                self.pop_scope();
                match self.function(&other).cloned() {
                    Some(oh) => {
                        let inner = call(self, &oh, &format!("&{value}"));
                        compensation = format!("Some(Box::new(|command: &{ct}| {{ let _ = {inner}; }}))");
                    }
                    None => self.error(format!("command `{handler}` is compensated by `{other}`, which has no handler")),
                }
            }
        }
        self.use_("y_runtime::command::Submit");
        crate::lower::Ex { code: format!("Submit::to(|command: &{ct}| {body}, {compensation})"), ty: Ty::Submit(Box::new(Ty::Named(handler.to_string()))), form: Form::Temp, prec: 100 }
    }

    /// A lambda, `name = lambda name(…) captures …:`, or a nested function: a function of its own, written after the
    /// one it is in, whose parameters are its own and then what it captures, each listed (Y § 3.1); and, where it
    /// captures anything or is bound as a value, a closure of the same name that passes the captures.
    pub fn local_function(&mut self, f: &Node, bound: Option<&str>) {
        let name = f.str("name").unwrap().to_string();
        if self.function(&name).is_some() || self.locals_fns.contains_key(&name) {
            self.error(format!("`{name}`: a lambda or nested function is named as no other callable of its module is (Y § 3.1, no shadowing)"));
        }
        let mut info = self.module().function_of(f, None);
        let captures: Vec<(String, bool)> = f.nodes("captures").iter().map(|c| (c.str("name").unwrap().to_string(), c.flag("mutable"))).collect();
        // what it reads and is not its own or captured is an error: a capture not listed (Y § 3.1)
        let own: Vec<String> = info.params.iter().map(|p| p.name.clone()).chain(captures.iter().map(|c| c.0.clone())).collect();
        let mut stray = vec![];
        for b in f.nodes("body") {
            b.walk(&mut |n: &Node| {
                if let Some(x) = n.str("name").filter(|_| n.kind == "name") {
                    if !own.iter().any(|o| o == x) && self.var(x).is_some() && !stray.contains(&x.to_string()) {
                        stray.push(x.to_string());
                    }
                }
            });
        }
        let lets: Vec<String> = { let mut v = vec![]; for b in f.nodes("body") { b.walk(&mut |n: &Node| if n.kind == "let" { v.push(n.str("name").unwrap_or("").to_string()) }); } v };
        for x in stray.into_iter().filter(|x| !lets.contains(x)) {
            self.error(format!("`{name}` reads `{x}`, which it does not capture: a capture is listed, `captures {x}` (Y § 3.1)"));
        }
        for (c, mutable) in &captures {
            let Some(v) = self.var(c).cloned() else {
                self.error(format!("`{name}` captures `{c}`, which is not in scope"));
                continue;
            };
            let mode = if *mutable { crate::model::Mode::Mut } else if self.is_copy_ty(&v.ty) { crate::model::Mode::Value } else { crate::model::Mode::Ref };
            // a number as written that nothing has typed: the function's result's type where it is a number, as the
            // value it is summed into is usually given back
            let ty = match (&v.ty, &self.ctx.result) {
                (Ty::Int, Some(r)) if r.is_int() => r.clone(),
                (Ty::Int, _) => Ty::U32,
                (t, _) => t.clone(),
            };
            info.params.push(crate::model::Param { name: c.clone(), ty, mutable: *mutable, injected: false, mode });
        }
        info.kind = if bound.is_some() { "lambda".into() } else { "nested".into() };
        // the function, lowered on its own, then written after the one it is in
        let saved_out = std::mem::take(&mut self.out);
        let saved_scopes = std::mem::replace(&mut self.scopes, vec![Default::default()]);
        let saved_ctx = std::mem::take(&mut self.ctx);
        let saved_olds = std::mem::take(&mut self.olds);
        let saved_ind = self.ind;
        let saved_after = std::mem::take(&mut self.after_fn);
        let saved_rest = (std::mem::take(&mut self.rest), std::mem::take(&mut self.rest_outer), std::mem::take(&mut self.loops));
        self.ind = 0;
        self.out.push(String::new());
        self.function_item(f, &info, "");
        let mut written = std::mem::replace(&mut self.out, saved_out);
        written.extend(std::mem::take(&mut self.after_fn));
        self.scopes = saved_scopes;
        self.ctx = saved_ctx;
        self.olds = saved_olds;
        self.ind = saved_ind;
        self.after_fn = saved_after;
        (self.rest, self.rest_outer, self.loops) = saved_rest;
        self.after_fn.extend(written);
        let plain: Vec<crate::model::Param> = info.params.iter().filter(|p| !captures.iter().any(|c| c.0 == p.name)).cloned().collect();
        // the closure that passes the captures, under the callable's name
        if !captures.is_empty() || bound.is_some() {
            let ps: Vec<String> = plain.iter().map(|p| {
                let t = self.rust_type(&p.ty);
                let t = match p.mode { crate::model::Mode::Ref => format!("&{t}"), crate::model::Mode::Mut => format!("&mut {t}"), crate::model::Mode::Value => t };
                format!("{}: {t}", p.name)
            }).collect();
            let mut args: Vec<String> = plain.iter().map(|p| p.name.clone()).collect();
            for (c, mutable) in &captures {
                let v = self.var(c).cloned();
                args.push(match (v.map(|v| v.form), mutable) {
                    (Some(Form::Mut), true) | (Some(Form::Ref), false) => c.clone(),
                    (_, true) => format!("&mut {c}"),
                    (_, false) if self.var(c).is_some_and(|v| self.is_copy_ty(&v.ty)) => c.clone(),
                    _ => format!("&{c}"),
                });
            }
            let kw = if captures.iter().any(|c| c.1) { "let mut" } else { "let" };
            self.line(format!("{kw} {name} = |{}| {name}({});", ps.join(", "), args.join(", ")));
        }
        let mut shown = info.clone();
        shown.params = plain;
        self.locals_fns.insert(name.clone(), (shown, captures));
        self.bind(&name, Ty::Named(format!("lambda:{name}")), Form::Place);
    }

    pub fn is_copy_ty(&self, t: &Ty) -> bool {
        crate::model::is_copy(self.module(), t)
    }
}

impl<'a> Lowerer<'a> {
    /// The methods of the declarations no example uses: a value's `becomes` another state, a unit's conversion to the
    /// unit it is made of, and a method an implementation gives a type.
    pub fn kind_method(&mut self, recv: &crate::lower::Ex, name: &str, args: &[(Option<&str>, &Node)]) -> Option<crate::lower::Ex> {
        let ex = |code: String, ty: Ty| crate::lower::Ex { code, ty, form: Form::Temp, prec: 100 };
        let r = if recv.prec < 100 { format!("({})", recv.code) } else { recv.code.clone() };
        // a structure that leaves a type open, filled (`pair<item_type=parcel>`), has the structure's methods
        let ty = match &recv.ty {
            Ty::Kind(k, _) if self.module().open.contains_key(k) => Ty::Named(k.clone()),
            t => t.clone(),
        };
        match &ty {
            // `charges.committed()`: the commands an outbox holds, which its atomic regions committed (Y § 2)
            Ty::Shared(o) if name == "committed" && args.is_empty() && matches!(&**o, Ty::Outbox(_)) => {
                let Ty::Outbox(c) = &**o else { unreachable!() };
                self.use_("y_collections::List");
                return Some(ex(format!("List::from({r}.snapshot().pending())"), Ty::List(c.clone())));
            }
            // `created.becomes(order.paid(receipt: r))`: the value in another state; the one it was is moved (§ 3.11)
            Ty::StateCase(st, _) if name == "becomes" => {
                let next = self.expr(args.first()?.1);
                let case = match &next.ty {
                    Ty::Named(n) if n.starts_with(&format!("{st}.")) && n.ends_with(":state") => n[st.len() + 1..n.len() - 6].to_string(),
                    _ => {
                        self.error(format!("`becomes` takes a state of `{st}`: `{st}.case(…)`"));
                        return None;
                    }
                };
                let declared = self.transitions_into(st, &case);
                if !declared {
                    self.error(format!("`becomes {st}.{case}` is in no transition that returns a `{st}.{case}`: a transition that is not declared does not exist (Y § 3.11)"));
                }
                Some(ex(format!("{r}.becomes({})", next.code), Ty::StateCase(st.clone(), case)))
            }
            // `distance.to_metre()`: a unit converted to one it is made of, by the factor its declaration gives
            Ty::Named(u) if name.starts_with("to_") && matches!(self.module().types.get(u).map(|t| &t.def), Some(TypeDef::Unit { .. })) => {
                let target = &name[3..];
                let mut factor: i128 = 1;
                let mut at = u.clone();
                while at != target {
                    match self.module().types.get(&at).map(|t| t.def.clone()) {
                        Some(TypeDef::Unit { factor: Some((f, of)), .. }) => {
                            factor *= f;
                            at = of;
                        }
                        _ => {
                            self.error(format!("`{name}`: `{u}` is not made of `{target}`, so there is no exact conversion (Y § 3.3)"));
                            return None;
                        }
                    }
                }
                Some(crate::lower::Ex { code: format!("{r} * {factor}"), ty: Ty::Named(target.to_string()), form: Form::Temp, prec: 80 })
            }
            // a method an implementation gives the type, or a trait's on a value of the trait
            Ty::Named(t) => {
                let m = self.module();
                // a value of an open type has the methods of the traits that bound it
                let bounds: Vec<String> = self.ctx.generics.iter().filter(|(g, _)| g == t).flat_map(|(_, b)| b.clone()).collect();
                let f = m
                    .methods
                    .get(t)
                    .and_then(|ms| ms.get(name))
                    .cloned()
                    .or_else(|| m.traits.get(t).and_then(|sigs| sigs.iter().find(|s| s.name == name)).cloned())
                    .or_else(|| bounds.iter().find_map(|b| m.traits.get(b).and_then(|sigs| sigs.iter().find(|s| s.name == name)).cloned()))?;
                let mut parts = vec![];
                let plain: Vec<crate::model::Param> = f.params.iter().filter(|p| !p.injected).cloned().collect();
                for (k, p) in plain.iter().enumerate() {
                    let arg = args.iter().find(|(an, _)| *an == Some(p.name.as_str())).or_else(|| args.get(k).filter(|(an, _)| an.is_none()));
                    if let Some((_, a)) = arg {
                        parts.push(self.pass(a, p));
                    }
                }
                let ret = f.result.as_ref().map(|r| r.1.clone()).unwrap_or(Ty::Unit);
                let ty = match &f.fails { Some(e) => Ty::Res(Box::new(ret), Box::new(e.clone())), None => ret };
                Some(ex(format!("{r}.{name}({})", parts.join(", ")), ty))
            }
            _ => None,
        }
    }

    /// Whether a transition of the module returns a value in a state: `becomes` may only make one that a transition
    /// does.
    fn transitions_into(&self, st: &str, case: &str) -> bool {
        let m = self.module();
        m.fns.values().any(|f| f.kind == "transition" && f.result.as_ref().is_some_and(|r| r.1 == Ty::StateCase(st.to_string(), case.to_string())))
    }
}

impl<'a> Lowerer<'a> {
    /// The unit of `a op b` where either is a quantity of a unit (Y § 3.3): the same unit for `+`, `-` and a
    /// comparison of two of it, and for `*` by a plain number; quantities of different units, or a unit and a plain
    /// number in a sum, are a compile error, as no conversion is implicit. `None` where neither is a unit.
    pub fn unit_result(&mut self, op: &str, a: &Ty, b: &Ty) -> Option<Ty> {
        let unit = |t: &Ty| match t {
            Ty::Named(n) if matches!(self.module().types.get(n).map(|t| &t.def), Some(TypeDef::Unit { .. })) => Some(n.clone()),
            _ => None,
        };
        let (ua, ub) = (unit(a), unit(b));
        if ua.is_none() && ub.is_none() {
            return None;
        }
        let plain = |t: &Ty| t.is_int() || matches!(t, Ty::Decimal(..));
        match (op, &ua, &ub) {
            (_, Some(x), Some(y)) if x != y => {
                self.error(format!("a `{x}` and a `{y}` together: quantities of different units do not mix without a stated conversion, such as `to_{y}()` (Y § 3.3)"));
                Some(a.clone())
            }
            ("+" | "-" | "==" | "!=" | "<" | ">" | "<=" | ">=", Some(_), Some(_)) => Some(a.clone()),
            ("*", Some(_), None) if plain(b) => Some(a.clone()),
            ("*", None, Some(_)) if plain(a) => Some(b.clone()),
            _ => {
                let u = ua.or(ub).unwrap();
                self.error(format!("`{op}` between a `{u}` and a number without a unit: the number needs its unit, as `5{}` (Y § 3.3)", self.module().types.get(&u).map(|t| match &t.def { TypeDef::Unit { symbol, .. } => symbol.clone(), _ => String::new() }).unwrap_or_default()));
                Some(a.clone())
            }
        }
    }

    /// A `conditional` on a value whose state is not known (`conditional s = stored:` with `condition s.paid = (s ==
    /// order.paid):`): a `match` on its state, recovered (`known()`), each arm the value in its state (Y § 3.11, "A value
    /// whose state is unknown … is matched to recover it").
    pub fn conditional_state(&mut self, st: &str, label: &str, s: &crate::lower::Ex, conds: &[&Node], els: Option<&Node>, tail: bool) {
        let subject = match s.form {
            Form::Temp | Form::Place => format!("{}.known()", s.code),
            _ => format!("{}.clone().known()", s.code),
        };
        let known = format!("{}Known", pascal(st));
        self.line(format!("match {subject} {{"));
        self.ind += 1;
        for c in conds {
            let t = c.node("test").unwrap();
            let case = t.node("right").filter(|r| r.kind == "member").and_then(|r| r.str("name")).unwrap_or("");
            let name = c.str("name").unwrap();
            self.push_scope();
            self.bind(label, Ty::StateCase(st.to_string(), case.to_string()), Form::Place);
            self.arm(&format!("{known}::{}({label})", pascal(case)), name, name, &c.nodes("body"), tail);
            self.pop_scope();
        }
        if let Some(e) = els {
            let name = e.str("name").unwrap();
            self.arm("_", name, name, &e.nodes("body"), tail);
        }
        self.ind -= 1;
        self.line("}");
    }
}
