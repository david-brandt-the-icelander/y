struct Cache { entries }

impl Cache {
    fn get(self, key) { self.entries.get(key) }
    fn insert(self, key, entry) { self.entries.insert(key, entry); }
}
