//! The standard library's collections and mappings (Y § 3.13), as Rust types: one facade per kind, offering only what
//! the kind's constraints allow, over whichever Rust representation suits it.
//!
//! | Y kind | Constraints | Here | Over |
//! |---|---|---|---|
//! | `list` | ordered by position; offers index, front, back | [`List`] | `Vec` |
//! | `array` | ordered by position; count fixed; offers index | [`Array`] | `Box<[T]>` |
//! | `stack` | ordered by position; offers back | [`Stack`] | `Vec` |
//! | `queue` | ordered by position; offers back_add, front_take | [`Queue`] | `VecDeque` |
//! | `deque` | ordered by position; offers front, back | [`Deque`] | `VecDeque` |
//! | `linked_list` | ordered by position; offers cursor; costs constant for cursor insert, cursor remove | [`LinkedList`] | nodes in a `Vec`, linked by position |
//! | `sorted_list` | ordered by value | [`SortedList`] | a sorted `Vec` |
//! | `set` | unique; ordered by insertion | [`Set`] | a `Vec` and a `HashMap` of positions |
//! | `sorted_set` | unique; ordered by value | [`SortedSet`] | `BTreeSet` |
//! | `unordered_set` | unique; unordered | [`UnorderedSet`] | `HashSet` |
//! | `bag` | unordered | [`Bag`] | a `HashMap` from each element to its count |
//! | `enumerable` | ordered by position; offers iterate_only | [`Enumerable`] | a boxed iterator |
//! | `map` | ordered by insertion | [`Map`] | a `Vec` of entries and a `HashMap` of positions |
//! | `sorted_map` | ordered by key | [`SortedMap`] | `BTreeMap` |
//! | `unordered_map` | unordered | [`UnorderedMap`] | `HashMap` |
//! | `multimap` | ordered by insertion; values many | [`MultiMap`] | a [`Map`] from each key to a `Vec` |
//!
//! An unordered kind has no iterator: it offers membership, counting, `all`, `any`, set operations and `sorted()`,
//! whose results cannot depend on hash order, so replay never sees it (Y § 4.7). A count is a `usize` and a position is
//! from 0, as the lowering's `count<of=t>` and `index<of=t>` are. Reading a position that does not exist is `None`;
//! writing one is a fault, since the position is the caller's precondition.
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::hash::Hash;
use y_runtime::fault;

fn no_such_position(kind: &str) -> ! {
    fault(&format!("y_collections::{kind}"), "no element at that position")
}

// ---------------------------------------------------------------------------------------------------------------------
// Ordered by position

/// Y's `list`: ordered by position; offers index, front, back.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct List<T>(Vec<T>);

impl<T> List<T> {
    pub fn new() -> Self {
        List(Vec::new())
    }
    pub fn count(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn get(&self, index: usize) -> Option<&T> {
        self.0.get(index)
    }
    pub fn set(&mut self, index: usize, value: T) {
        match self.0.get_mut(index) {
            Some(slot) => *slot = value,
            None => no_such_position("list"),
        }
    }
    pub fn front(&self) -> Option<&T> {
        self.0.first()
    }
    pub fn back(&self) -> Option<&T> {
        self.0.last()
    }
    pub fn push_front(&mut self, value: T) {
        self.0.insert(0, value);
    }
    pub fn push(&mut self, value: T) {
        self.0.push(value);
    }
    pub fn take_front(&mut self) -> Option<T> {
        (!self.0.is_empty()).then(|| self.0.remove(0))
    }
    pub fn take_back(&mut self) -> Option<T> {
        self.0.pop()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }
}

impl<T> FromIterator<T> for List<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        List(items.into_iter().collect())
    }
}

/// Y's `array`: ordered by position; count fixed; offers index.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Array<T>(Box<[T]>);

impl<T> Array<T> {
    /// An array of `count` copies of `value`; its count never changes.
    pub fn repeated(value: T, count: usize) -> Self
    where
        T: Clone,
    {
        Array(vec![value; count].into_boxed_slice())
    }
    pub fn count(&self) -> usize {
        self.0.len()
    }
    pub fn get(&self, index: usize) -> Option<&T> {
        self.0.get(index)
    }
    pub fn set(&mut self, index: usize, value: T) {
        match self.0.get_mut(index) {
            Some(slot) => *slot = value,
            None => no_such_position("array"),
        }
    }
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }
}

impl<T> FromIterator<T> for Array<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        Array(items.into_iter().collect())
    }
}

/// Y's `stack`: ordered by position; offers back. Last in, first out.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stack<T>(Vec<T>);

impl<T> Stack<T> {
    pub fn new() -> Self {
        Stack(Vec::new())
    }
    pub fn count(&self) -> usize {
        self.0.len()
    }
    pub fn push(&mut self, value: T) {
        self.0.push(value);
    }
    pub fn take(&mut self) -> Option<T> {
        self.0.pop()
    }
    pub fn top(&self) -> Option<&T> {
        self.0.last()
    }
    /// From the bottom to the top.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }
}

/// Y's `queue`: ordered by position; offers back_add, front_take. First in, first out.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Queue<T>(VecDeque<T>);

impl<T> Queue<T> {
    pub fn new() -> Self {
        Queue(VecDeque::new())
    }
    pub fn count(&self) -> usize {
        self.0.len()
    }
    pub fn add(&mut self, value: T) {
        self.0.push_back(value);
    }
    pub fn take(&mut self) -> Option<T> {
        self.0.pop_front()
    }
    pub fn next(&self) -> Option<&T> {
        self.0.front()
    }
    /// From the front to the back.
    pub fn iter(&self) -> std::collections::vec_deque::Iter<'_, T> {
        self.0.iter()
    }
}

/// Y's `deque`: ordered by position; offers front, back.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Deque<T>(VecDeque<T>);

impl<T> Deque<T> {
    pub fn new() -> Self {
        Deque(VecDeque::new())
    }
    pub fn count(&self) -> usize {
        self.0.len()
    }
    pub fn add_front(&mut self, value: T) {
        self.0.push_front(value);
    }
    pub fn add_back(&mut self, value: T) {
        self.0.push_back(value);
    }
    pub fn take_front(&mut self) -> Option<T> {
        self.0.pop_front()
    }
    pub fn take_back(&mut self) -> Option<T> {
        self.0.pop_back()
    }
    pub fn front(&self) -> Option<&T> {
        self.0.front()
    }
    pub fn back(&self) -> Option<&T> {
        self.0.back()
    }
    pub fn iter(&self) -> std::collections::vec_deque::Iter<'_, T> {
        self.0.iter()
    }
}

/// A place in a [`LinkedList`]: it stays valid while its element is in the list, whatever is inserted or removed
/// elsewhere. A node's slot is reused after its element is removed; the generation tells the old cursor from the new.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Cursor(usize, u64);

#[derive(Clone, Debug)]
struct Node<T> {
    value: Option<T>,
    generation: u64,
    previous: Option<usize>,
    next: Option<usize>,
}

/// Y's `linked_list`: ordered by position; offers cursor; costs constant for cursor insert, cursor remove. Its nodes
/// live in one `Vec` and refer to each other by position, so no node is ever reached through a dangling pointer.
#[derive(Clone, Debug)]
pub struct LinkedList<T> {
    nodes: Vec<Node<T>>,
    free: Vec<usize>,
    first: Option<usize>,
    last: Option<usize>,
    count: usize,
}

impl<T> Default for LinkedList<T> {
    fn default() -> Self {
        LinkedList { nodes: Vec::new(), free: Vec::new(), first: None, last: None, count: 0 }
    }
}

impl<T> LinkedList<T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn count(&self) -> usize {
        self.count
    }
    fn cursor(&self, at: usize) -> Cursor {
        Cursor(at, self.nodes[at].generation)
    }
    pub fn first(&self) -> Option<Cursor> {
        self.first.map(|at| self.cursor(at))
    }
    pub fn last(&self) -> Option<Cursor> {
        self.last.map(|at| self.cursor(at))
    }
    fn node(&self, at: Cursor) -> &Node<T> {
        match self.nodes.get(at.0) {
            Some(node) if node.value.is_some() && node.generation == at.1 => node,
            _ => no_such_position("linked_list"),
        }
    }
    pub fn get(&self, at: Cursor) -> &T {
        self.node(at).value.as_ref().unwrap_or_else(|| no_such_position("linked_list"))
    }
    pub fn next(&self, at: Cursor) -> Option<Cursor> {
        self.node(at).next.map(|n| self.cursor(n))
    }
    pub fn previous(&self, at: Cursor) -> Option<Cursor> {
        self.node(at).previous.map(|p| self.cursor(p))
    }
    fn allocate(&mut self, value: T, previous: Option<usize>, next: Option<usize>) -> usize {
        match self.free.pop() {
            Some(at) => {
                let generation = self.nodes[at].generation + 1;
                self.nodes[at] = Node { value: Some(value), generation, previous, next };
                at
            }
            None => {
                self.nodes.push(Node { value: Some(value), generation: 0, previous, next });
                self.nodes.len() - 1
            }
        }
    }
    /// Inserts after `at`, or first when `at` is `None`. Constant cost.
    pub fn insert_after(&mut self, at: Option<Cursor>, value: T) -> Cursor {
        let previous = at.map(|c| {
            self.node(c);
            c.0
        });
        let next = match previous {
            Some(p) => self.nodes[p].next,
            None => self.first,
        };
        let new = self.allocate(value, previous, next);
        match previous {
            Some(p) => self.nodes[p].next = Some(new),
            None => self.first = Some(new),
        }
        match next {
            Some(n) => self.nodes[n].previous = Some(new),
            None => self.last = Some(new),
        }
        self.count += 1;
        self.cursor(new)
    }
    /// Adds at the end. Constant cost.
    pub fn push(&mut self, value: T) -> Cursor {
        let last = self.last();
        self.insert_after(last, value)
    }
    /// Removes the element at `at`, whose cursor is then invalid. Constant cost.
    pub fn remove(&mut self, at: Cursor) -> T {
        let (previous, next) = {
            let node = self.node(at);
            (node.previous, node.next)
        };
        match previous {
            Some(p) => self.nodes[p].next = next,
            None => self.first = next,
        }
        match next {
            Some(n) => self.nodes[n].previous = previous,
            None => self.last = previous,
        }
        self.count -= 1;
        self.free.push(at.0);
        self.nodes[at.0].value.take().unwrap_or_else(|| no_such_position("linked_list"))
    }
    /// From the first element to the last.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        std::iter::successors(self.first, |&at| self.nodes[at].next).filter_map(|at| self.nodes[at].value.as_ref())
    }
}

/// Y's `sorted_list`: ordered by value; duplicates allowed.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SortedList<T: Ord>(Vec<T>);

impl<T: Ord> SortedList<T> {
    pub fn new() -> Self {
        SortedList(Vec::new())
    }
    pub fn count(&self) -> usize {
        self.0.len()
    }
    /// Adds an element where its value puts it, after any equal to it.
    pub fn add(&mut self, value: T) {
        let at = self.0.partition_point(|x| x <= &value);
        self.0.insert(at, value);
    }
    pub fn contains(&self, value: &T) -> bool {
        self.0.binary_search(value).is_ok()
    }
    pub fn get(&self, index: usize) -> Option<&T> {
        self.0.get(index)
    }
    pub fn smallest(&self) -> Option<&T> {
        self.0.first()
    }
    pub fn largest(&self) -> Option<&T> {
        self.0.last()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }
}

impl<T: Ord> FromIterator<T> for SortedList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        let mut v: Vec<T> = items.into_iter().collect();
        v.sort();
        SortedList(v)
    }
}

/// Y's `enumerable`: ordered by position; offers iterate_only. Read once, in order, possibly without end.
pub struct Enumerable<'a, T>(Box<dyn Iterator<Item = T> + 'a>);

impl<'a, T> Enumerable<'a, T> {
    pub fn new(items: impl Iterator<Item = T> + 'a) -> Self {
        Enumerable(Box::new(items))
    }
}

impl<T> Iterator for Enumerable<'_, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.0.next()
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Unique

/// Y's `set`: unique; ordered by insertion. Iteration visits each element in the order it was first added.
#[derive(Clone, Debug)]
pub struct Set<T: Hash + Eq + Clone> {
    order: Vec<T>,
    position: HashMap<T, usize>,
}

impl<T: Hash + Eq + Clone> Default for Set<T> {
    fn default() -> Self {
        Set { order: Vec::new(), position: HashMap::new() }
    }
}

impl<T: Hash + Eq + Clone> PartialEq for Set<T> {
    /// Two sets are equal when they hold the same elements in the same order, as Y's structural equality says of an
    /// ordered collection.
    fn eq(&self, other: &Self) -> bool {
        self.order == other.order
    }
}

impl<T: Hash + Eq + Clone> Eq for Set<T> {}

impl<T: Hash + Eq + Clone> Set<T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn count(&self) -> usize {
        self.order.len()
    }
    pub fn contains(&self, value: &T) -> bool {
        self.position.contains_key(value)
    }
    /// Adds an element if it is not there; whether it was added.
    pub fn add(&mut self, value: T) -> bool {
        if self.position.contains_key(&value) {
            return false;
        }
        self.position.insert(value.clone(), self.order.len());
        self.order.push(value);
        true
    }
    /// Removes an element; whether it was there. The others keep their order.
    pub fn remove(&mut self, value: &T) -> bool {
        let Some(at) = self.position.remove(value) else { return false };
        self.order.remove(at);
        for later in &self.order[at..] {
            if let Some(p) = self.position.get_mut(later) {
                *p -= 1;
            }
        }
        true
    }
    pub fn is_subset_of(&self, other: &Set<T>) -> bool {
        self.order.iter().all(|x| other.contains(x))
    }
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.order.iter()
    }
}

impl<T: Hash + Eq + Clone> FromIterator<T> for Set<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        let mut set = Set::new();
        for x in items {
            set.add(x);
        }
        set
    }
}

/// Y's `sorted_set`: unique; ordered by value.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SortedSet<T: Ord>(BTreeSet<T>);

impl<T: Ord> SortedSet<T> {
    pub fn new() -> Self {
        SortedSet(BTreeSet::new())
    }
    pub fn count(&self) -> usize {
        self.0.len()
    }
    pub fn contains(&self, value: &T) -> bool {
        self.0.contains(value)
    }
    pub fn add(&mut self, value: T) -> bool {
        self.0.insert(value)
    }
    pub fn remove(&mut self, value: &T) -> bool {
        self.0.remove(value)
    }
    pub fn is_subset_of(&self, other: &SortedSet<T>) -> bool {
        self.0.is_subset(&other.0)
    }
    pub fn iter(&self) -> std::collections::btree_set::Iter<'_, T> {
        self.0.iter()
    }
}

impl<T: Ord> FromIterator<T> for SortedSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        SortedSet(items.into_iter().collect())
    }
}

/// Y's `unordered_set`: unique; unordered. It has no iterator: what it offers cannot depend on hash order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnorderedSet<T: Hash + Eq>(HashSet<T>);

impl<T: Hash + Eq> UnorderedSet<T> {
    pub fn new() -> Self {
        UnorderedSet(HashSet::new())
    }
    pub fn count(&self) -> usize {
        self.0.len()
    }
    pub fn contains(&self, value: &T) -> bool {
        self.0.contains(value)
    }
    pub fn add(&mut self, value: T) -> bool {
        self.0.insert(value)
    }
    pub fn remove(&mut self, value: &T) -> bool {
        self.0.remove(value)
    }
    pub fn all(&self, test: impl Fn(&T) -> bool) -> bool {
        self.0.iter().all(test)
    }
    pub fn any(&self, test: impl Fn(&T) -> bool) -> bool {
        self.0.iter().any(test)
    }
    pub fn is_subset_of(&self, other: &UnorderedSet<T>) -> bool {
        self.0.is_subset(&other.0)
    }
    pub fn union(&self, other: &UnorderedSet<T>) -> UnorderedSet<T>
    where
        T: Clone,
    {
        UnorderedSet(self.0.union(&other.0).cloned().collect())
    }
    pub fn intersection(&self, other: &UnorderedSet<T>) -> UnorderedSet<T>
    where
        T: Clone,
    {
        UnorderedSet(self.0.intersection(&other.0).cloned().collect())
    }
    /// Its elements in an order a program may observe: their own.
    pub fn sorted(&self) -> SortedList<T>
    where
        T: Ord + Clone,
    {
        self.0.iter().cloned().collect()
    }
}

impl<T: Hash + Eq> FromIterator<T> for UnorderedSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        UnorderedSet(items.into_iter().collect())
    }
}

/// Y's `bag`: unordered, with duplicates. It has no iterator, as an `unordered_set` has none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bag<T: Hash + Eq> {
    counts: HashMap<T, usize>,
    count: usize,
}

impl<T: Hash + Eq> Bag<T> {
    pub fn new() -> Self {
        Bag { counts: HashMap::new(), count: 0 }
    }
    pub fn count(&self) -> usize {
        self.count
    }
    /// How many of the bag's elements equal `value`.
    pub fn count_of(&self, value: &T) -> usize {
        self.counts.get(value).copied().unwrap_or(0)
    }
    pub fn add(&mut self, value: T) {
        *self.counts.entry(value).or_insert(0) += 1;
        self.count += 1;
    }
    /// Removes one element equal to `value`; whether there was one.
    pub fn remove(&mut self, value: &T) -> bool {
        match self.counts.get_mut(value) {
            Some(n) => {
                *n -= 1;
                if *n == 0 {
                    self.counts.remove(value);
                }
                self.count -= 1;
                true
            }
            None => false,
        }
    }
    pub fn sorted(&self) -> SortedList<T>
    where
        T: Ord + Clone,
    {
        self.counts.iter().flat_map(|(x, n)| std::iter::repeat_n(x.clone(), *n)).collect()
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Mappings

/// Y's `map`: keys unique; ordered by insertion. Iteration visits each key in the order it was first added.
#[derive(Clone, Debug)]
pub struct Map<K: Hash + Eq + Clone, V> {
    entries: Vec<(K, V)>,
    position: HashMap<K, usize>,
}

impl<K: Hash + Eq + Clone, V> Default for Map<K, V> {
    fn default() -> Self {
        Map { entries: Vec::new(), position: HashMap::new() }
    }
}

impl<K: Hash + Eq + Clone, V: PartialEq> PartialEq for Map<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl<K: Hash + Eq + Clone, V> Map<K, V> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn count(&self) -> usize {
        self.entries.len()
    }
    pub fn get(&self, key: &K) -> Option<&V> {
        self.position.get(key).map(|&at| &self.entries[at].1)
    }
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.position.get(key).map(|&at| &mut self.entries[at].1)
    }
    pub fn contains_key(&self, key: &K) -> bool {
        self.position.contains_key(key)
    }
    /// Sets the value of a key; a key already there keeps its place. The value it had, if any.
    pub fn set(&mut self, key: K, value: V) -> Option<V> {
        match self.position.get(&key) {
            Some(&at) => Some(std::mem::replace(&mut self.entries[at].1, value)),
            None => {
                self.position.insert(key.clone(), self.entries.len());
                self.entries.push((key, value));
                None
            }
        }
    }
    /// Removes a key; the others keep their order.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let at = self.position.remove(key)?;
        let (_, value) = self.entries.remove(at);
        for (later, _) in &self.entries[at..] {
            if let Some(p) = self.position.get_mut(later) {
                *p -= 1;
            }
        }
        Some(value)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.iter().map(|(k, _)| k)
    }
}

/// Y's `sorted_map`: keys unique; ordered by key.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SortedMap<K: Ord, V>(BTreeMap<K, V>);

impl<K: Ord, V> SortedMap<K, V> {
    pub fn new() -> Self {
        SortedMap(BTreeMap::new())
    }
    pub fn count(&self) -> usize {
        self.0.len()
    }
    pub fn get(&self, key: &K) -> Option<&V> {
        self.0.get(key)
    }
    pub fn contains_key(&self, key: &K) -> bool {
        self.0.contains_key(key)
    }
    pub fn set(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.0.remove(key)
    }
    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, K, V> {
        self.0.iter()
    }
}

/// Y's `unordered_map`: keys unique; unordered. It has no iterator; `sorted()` gives its entries in key order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnorderedMap<K: Hash + Eq, V>(HashMap<K, V>);

impl<K: Hash + Eq, V> UnorderedMap<K, V> {
    pub fn new() -> Self {
        UnorderedMap(HashMap::new())
    }
    pub fn count(&self) -> usize {
        self.0.len()
    }
    pub fn get(&self, key: &K) -> Option<&V> {
        self.0.get(key)
    }
    pub fn contains_key(&self, key: &K) -> bool {
        self.0.contains_key(key)
    }
    pub fn set(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.0.remove(key)
    }
    pub fn all(&self, test: impl Fn(&K, &V) -> bool) -> bool {
        self.0.iter().all(|(k, v)| test(k, v))
    }
    pub fn any(&self, test: impl Fn(&K, &V) -> bool) -> bool {
        self.0.iter().any(|(k, v)| test(k, v))
    }
    pub fn sorted(&self) -> SortedMap<K, V>
    where
        K: Ord + Clone,
        V: Clone,
    {
        SortedMap(self.0.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    }
}

/// Y's `multimap`: keys ordered by insertion; values many, in the order each was added.
#[derive(Clone, Debug)]
pub struct MultiMap<K: Hash + Eq + Clone, V>(Map<K, Vec<V>>);

impl<K: Hash + Eq + Clone, V> Default for MultiMap<K, V> {
    fn default() -> Self {
        MultiMap(Map::new())
    }
}

impl<K: Hash + Eq + Clone, V> MultiMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }
    /// How many keys it holds.
    pub fn count(&self) -> usize {
        self.0.count()
    }
    pub fn add(&mut self, key: K, value: V) {
        match self.0.get_mut(&key) {
            Some(values) => values.push(value),
            None => {
                self.0.set(key, vec![value]);
            }
        }
    }
    /// The values of a key, in the order they were added; none for a key it does not hold.
    pub fn get(&self, key: &K) -> &[V] {
        self.0.get(key).map(Vec::as_slice).unwrap_or(&[])
    }
    pub fn remove(&mut self, key: &K) -> Vec<V> {
        self.0.remove(key).unwrap_or_default()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&K, &[V])> {
        self.0.iter().map(|(k, v)| (k, v.as_slice()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_by_position() {
        let mut list: List<u32> = [2, 3].into_iter().collect();
        list.push_front(1);
        list.push(4);
        assert_eq!(list.iter().copied().collect::<Vec<_>>(), [1, 2, 3, 4]);
        assert_eq!((list.take_front(), list.take_back(), list.count()), (Some(1), Some(4), 2));
        assert!(std::panic::catch_unwind(move || list.clone().set(9, 0)).is_err());

        let mut array = Array::repeated(0, 3);
        array.set(1, 7);
        assert_eq!((array.count(), array.get(1), array.get(3)), (3, Some(&7), None));

        let mut stack = Stack::new();
        stack.push('a');
        stack.push('b');
        assert_eq!((stack.take(), stack.top()), (Some('b'), Some(&'a')));

        let mut queue = Queue::new();
        queue.add('a');
        queue.add('b');
        assert_eq!((queue.take(), queue.next()), (Some('a'), Some(&'b')));

        let mut deque = Deque::new();
        deque.add_back(2);
        deque.add_front(1);
        assert_eq!((deque.front(), deque.back()), (Some(&1), Some(&2)));
    }

    #[test]
    fn a_linked_list_keeps_its_cursors() {
        let mut list = LinkedList::new();
        let a = list.push('a');
        let c = list.push('c');
        let b = list.insert_after(Some(a), 'b');
        assert_eq!(list.iter().collect::<String>(), "abc");
        assert_eq!(list.remove(b), 'b');
        assert_eq!(list.next(a), Some(c));
        let d = list.insert_after(Some(c), 'd');
        assert_eq!((list.iter().collect::<String>(), list.count(), list.last()), ("acd".to_string(), 3, Some(d)));
        list.insert_after(None, 'z');
        assert_eq!(list.iter().collect::<String>(), "zacd");
        assert!(std::panic::catch_unwind(move || list.clone().remove(b)).is_err());
    }

    #[test]
    fn sorted_kinds_keep_their_order() {
        let mut list: SortedList<i32> = [3, 1, 2].into_iter().collect();
        list.add(2);
        assert_eq!(list.iter().copied().collect::<Vec<_>>(), [1, 2, 2, 3]);
        let set: SortedSet<i32> = [3, 1, 3].into_iter().collect();
        assert_eq!(set.iter().copied().collect::<Vec<_>>(), [1, 3]);
    }

    #[test]
    fn a_set_and_a_map_keep_insertion_order() {
        let mut set: Set<&str> = ["pear", "apple", "pear"].into_iter().collect();
        assert!(!set.add("apple"));
        set.add("fig");
        set.remove(&"pear");
        assert_eq!(set.iter().copied().collect::<Vec<_>>(), ["apple", "fig"]);
        assert!(set.contains(&"fig") && !set.contains(&"pear"));

        let mut map = Map::new();
        map.set("b", 1);
        map.set("a", 2);
        assert_eq!(map.set("b", 3), Some(1));
        map.set("c", 4);
        map.remove(&"a");
        assert_eq!(map.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(), [("b", 3), ("c", 4)]);
        assert_eq!(map.get(&"c"), Some(&4));

        let mut multi = MultiMap::new();
        multi.add("x", 1);
        multi.add("y", 2);
        multi.add("x", 3);
        assert_eq!(multi.get(&"x"), [1, 3]);
        assert_eq!(multi.iter().map(|(k, _)| *k).collect::<Vec<_>>(), ["x", "y"]);
    }

    #[test]
    fn unordered_kinds_show_order_only_when_sorted() {
        let set: UnorderedSet<u8> = [5, 1, 3].into_iter().collect();
        assert!(set.all(|x| x % 2 == 1) && set.contains(&3));
        assert_eq!(set.sorted().iter().copied().collect::<Vec<_>>(), [1, 3, 5]);
        let both = set.intersection(&[3, 4].into_iter().collect());
        assert_eq!((both.count(), set.union(&[4].into_iter().collect()).count()), (1, 4));

        let mut bag = Bag::new();
        for x in ["b", "a", "b"] {
            bag.add(x);
        }
        assert_eq!((bag.count(), bag.count_of(&"b")), (3, 2));
        assert!(bag.remove(&"b"));
        assert_eq!(bag.sorted().iter().copied().collect::<Vec<_>>(), ["a", "b"]);

        let mut map = UnorderedMap::new();
        map.set(2, "two");
        map.set(1, "one");
        assert_eq!(map.sorted().iter().map(|(k, _)| *k).collect::<Vec<_>>(), [1, 2]);
    }

    #[test]
    fn an_enumerable_is_read_once() {
        let mut naturals = Enumerable::new(0u32..);
        assert_eq!(naturals.by_ref().take(3).collect::<Vec<_>>(), [0, 1, 2]);
        assert_eq!(naturals.next(), Some(3));
    }
}
