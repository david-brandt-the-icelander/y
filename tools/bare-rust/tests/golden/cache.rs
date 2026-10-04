pub(crate) struct Cache { entries: std::collections::HashMap<Key, Entry> }

impl Cache {
    pub fn get(&self, key: &Key) -> Option<&Entry> { self.entries.get(key) }
    pub fn insert(&mut self, key: Key, entry: Entry) { self.entries.insert(key, entry); }
}
