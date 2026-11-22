use strsim::*;
fn dist(a: &str, b: &str) -> Result<usize, Box<dyn std::error::Error>> { Ok(hamming(a, b)?) }
#[test] fn display() { assert_eq!(StrSimError::DifferentLengthArgs.to_string(), "Differing length arguments provided"); assert_eq!(format!("{}", StrSimError::DifferentLengthArgs), "Differing length arguments provided"); }
#[test] fn question_mark() { assert_eq!(dist("ham", "hum").unwrap(), 1); let e = dist("ham", "hamming").unwrap_err(); assert_eq!(e.to_string(), "Differing length arguments provided"); }
#[test] fn is_error() { fn takes<E: std::error::Error>(_: &E) {} takes(&StrSimError::DifferentLengthArgs); }
