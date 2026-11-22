//! `y-parse <file.y>…`: prints a JSON object that maps each file, as given, to its tree. With `--check`, prints
//! nothing and only reports errors. An error is printed as `file:line: message`, and the exit status is 1.
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let check = args.iter().any(|a| a == "--check");
    let files: Vec<&String> = args.iter().filter(|a| *a != "--check").collect();
    if files.is_empty() {
        eprintln!("usage: y-parse [--check] <file.y>…");
        std::process::exit(2);
    }
    let mut out = String::from("{");
    let mut failed = false;
    for (i, f) in files.iter().enumerate() {
        match y_parse::parse_file(Path::new(f)) {
            Ok(tree) => {
                if !check {
                    let json = tree.to_json().replace('\n', "\n  ");
                    out.push_str(&format!("{}\n  {}: {json}", if i > 0 { "," } else { "" }, y_parse::Value::from(f.as_str()).to_json_string()));
                }
            }
            Err(e) => {
                eprintln!("{e}");
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
    if !check {
        println!("{out}\n}}");
    }
}
