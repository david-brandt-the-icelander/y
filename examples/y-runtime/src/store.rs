//! Y's persistent data (Y § 3.12): `store<schema_type=s>`, the capability that reads and writes the records of a
//! schema, each stored with its version, and migrated on read.
//!
//! A schema's codec and migrations are what the Y compiler generates from its `schema` and `migrate` declarations; the
//! examples write them by hand, as an implementation of [`Schema`]. A record the codec or a migration cannot read fails
//! alone, against its key, and the others are read.
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

/// Why one stored record could not be read: its key, the version it was stored at, and what went wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordFailure {
    pub key: String,
    pub version: u32,
    pub reason: String,
}

/// Why a store could not be read or written at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreError {
    pub reason: String,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.reason)
    }
}

/// A schema's codec: its name and current version, its key, and how a record is written and read. `decode` reads every
/// version the schema still has, and migrates an older one to the current version.
pub trait Schema: Sized {
    const NAME: &'static str;
    const VERSION: u32;
    fn key(&self) -> String;
    fn encode(&self) -> String;
    /// The record stored under `key` as `text` at `version`, migrated to the current version, or why it cannot be.
    fn decode(key: &str, version: u32, text: &str) -> Result<Self, String>;
}

/// Y's `store<schema_type=s>`: the stored records of one schema, kept in a file, or in memory for a fake.
pub struct Store<S: Schema> {
    file: Option<PathBuf>,
    /// Each record's key, to its version and text.
    records: BTreeMap<String, (u32, String)>,
    _schema: std::marker::PhantomData<S>,
}

impl<S: Schema> Store<S> {
    /// A store kept in a file, whose lines are `name v<version> <key>|<text>`. A file that does not exist is empty.
    pub fn open(file: PathBuf) -> Result<Self, StoreError> {
        let text = match std::fs::read_to_string(&file) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(StoreError { reason: e.to_string() }),
        };
        let mut store = Store::fake(&[]);
        store.file = Some(file);
        for line in text.lines().filter(|l| !l.is_empty()) {
            store.load_line(line)?;
        }
        Ok(store)
    }

    /// A store in memory, with these lines already in it: what a test injects.
    pub fn fake(lines: &[&str]) -> Self {
        let mut store = Store { file: None, records: BTreeMap::new(), _schema: std::marker::PhantomData };
        for line in lines {
            store.load_line(line).unwrap_or_else(|e| super::fault("y_runtime::store::fake", &e.reason));
        }
        store
    }

    fn load_line(&mut self, line: &str) -> Result<(), StoreError> {
        let bad = || StoreError { reason: format!("not a record of {}: `{line}`", S::NAME) };
        let rest = line.strip_prefix(S::NAME).and_then(|r| r.strip_prefix(" v")).ok_or_else(bad)?;
        let (version, rest) = rest.split_once(' ').ok_or_else(bad)?;
        let (key, text) = rest.split_once('|').ok_or_else(bad)?;
        let version = version.parse().map_err(|_| bad())?;
        self.records.insert(key.to_string(), (version, text.to_string()));
        Ok(())
    }

    /// Every record, in key order, each read and migrated alone: a record that fails is reported against its key.
    pub fn read_all(&self) -> Vec<Result<S, RecordFailure>> {
        self.records
            .iter()
            .map(|(key, (version, text))| {
                S::decode(key, *version, text).map_err(|reason| RecordFailure { key: key.clone(), version: *version, reason })
            })
            .collect()
    }

    /// Writes a record at the current version, replacing the one with its key.
    pub fn put(&mut self, record: &S) {
        self.records.insert(record.key(), (S::VERSION, record.encode()));
    }

    /// The stored lines, as the file holds them.
    pub fn lines(&self) -> Vec<String> {
        self.records.iter().map(|(k, (v, t))| format!("{} v{v} {k}|{t}", S::NAME)).collect()
    }

    /// A batch of writes to this store, held as a resource: its records are written, and the store saved, when it is
    /// released (Y § 3.6, § 3.12).
    pub fn batch(&mut self) -> Batch<'_, S> {
        Batch { store: self, records: Vec::new() }
    }

    /// Writes the records to the file; a fake has none to write to.
    pub fn save(&self) -> Result<(), StoreError> {
        match &self.file {
            Some(f) => std::fs::write(f, self.lines().join("\n") + "\n").map_err(|e| StoreError { reason: e.to_string() }),
            None => Ok(()),
        }
    }
}

/// A batch of writes to a store: a resource, whose release writes them and saves the store, and fails as saving does.
pub struct Batch<'a, S: Schema> {
    store: &'a mut Store<S>,
    records: Vec<S>,
}

impl<S: Schema> Batch<'_, S> {
    /// Adds a record to the batch; it is written when the batch is released.
    pub fn put(&mut self, record: S) {
        self.records.push(record);
    }
}

impl<S: Schema> super::arena::Resource for Batch<'_, S> {
    type ReleaseFailure = StoreError;
    fn release(&mut self) -> Result<(), StoreError> {
        for record in self.records.drain(..) {
            self.store.put(&record);
        }
        self.store.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Item {
        sku: String,
        stock: u32,
        shelf: String,
    }

    /// Version 1 has no shelf; the migration to version 2 puts every item on the shelf `unsorted`.
    impl Schema for Item {
        const NAME: &'static str = "item";
        const VERSION: u32 = 2;
        fn key(&self) -> String {
            self.sku.clone()
        }
        fn encode(&self) -> String {
            format!("{};{}", self.stock, self.shelf)
        }
        fn decode(key: &str, version: u32, text: &str) -> Result<Self, String> {
            let parts: Vec<&str> = text.split(';').collect();
            let stock = parts[0].parse().map_err(|_| format!("stock `{}` is not a count", parts[0]))?;
            match (version, parts.as_slice()) {
                (1, [_]) => Ok(Item { sku: key.into(), stock, shelf: "unsorted".into() }),
                (2, [_, shelf]) => Ok(Item { sku: key.into(), stock, shelf: shelf.to_string() }),
                _ => Err(format!("no version {version} of item")),
            }
        }
    }

    #[test]
    fn records_are_migrated_on_read_and_fail_alone() {
        let mut store: Store<Item> = Store::fake(&["item v1 a-1|3", "item v2 b-2|5;north", "item v1 c-3|lots"]);
        let read = store.read_all();
        assert_eq!(read[0].as_ref().map(|i| i.shelf.as_str()), Ok("unsorted"));
        assert_eq!(read[1].as_ref().map(|i| i.stock), Ok(5));
        assert_eq!(read[2].as_ref().map_err(|f| (f.key.as_str(), f.version)), Err(("c-3", 1)));
        store.put(&Item { sku: "a-1".into(), stock: 2, shelf: "south".into() });
        assert_eq!(store.lines()[0], "item v2 a-1|2;south");
    }

    #[test]
    fn a_store_survives_in_its_file() {
        let file = std::env::temp_dir().join(format!("y-store-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&file);
        let mut store: Store<Item> = Store::open(file.clone()).unwrap();
        store.put(&Item { sku: "z-9".into(), stock: 1, shelf: "east".into() });
        store.save().unwrap();
        let again: Store<Item> = Store::open(file.clone()).unwrap();
        assert_eq!(again.lines(), ["item v2 z-9|1;east"]);
        let _ = std::fs::remove_file(&file);
    }
}
