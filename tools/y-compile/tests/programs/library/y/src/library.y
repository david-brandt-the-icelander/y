module library


-- Operations of the standard library that the lowering has no rule of its own for: each is the runtime crate's
-- method of the same name, as the crate declares it.
public facet operations:
    ------------------------------------------------------------------------------------------------------------------------
    Empties a list.

    items: The list, as list<element_type=uint32>.
    ------------------------------------------------------------------------------------------------------------------------
    function emptied(mutable items):
        items.clear()


    ------------------------------------------------------------------------------------------------------------------------
    Removes a letter from a set, and says whether it was there.

    seen: The letters, as set<element_type=rune>.
    letter: The letter, as rune.

    removed: Whether it was there, as boolean.
    ------------------------------------------------------------------------------------------------------------------------
    function forget(mutable seen, letter) returns removed:
        return removed = seen.remove(letter)


    ------------------------------------------------------------------------------------------------------------------------
    The first item of a list, if it has one.

    items: The list, as list<element_type=uint32>.

    first: The first item, as optional<value_type=uint32>.
    ------------------------------------------------------------------------------------------------------------------------
    function first_of(items) returns first:
        return first = items.front()


    ------------------------------------------------------------------------------------------------------------------------
    A letter in lower case.

    letter: The letter, as rune.

    lower: It in lower case, as rune.
    ------------------------------------------------------------------------------------------------------------------------
    function lowered(letter) returns lower:
        return lower = letter.lowercased()


    ------------------------------------------------------------------------------------------------------------------------
    Whether a word holds a letter.

    word: The word, as string.
    letter: The letter, as rune.

    held: Whether it does, as boolean.
    ------------------------------------------------------------------------------------------------------------------------
    function holds(word, letter) returns held:
        return held = word.contains(letter)


    ------------------------------------------------------------------------------------------------------------------------
    Whether a text matches a pattern of the subset the runtime's matcher takes.

    text: The text, as string.

    shaped: Whether it is a word, an `@` and a word with a dot, as boolean.
    ------------------------------------------------------------------------------------------------------------------------
    function is_address(text) returns shaped:
        return shaped = text.matches(pattern"^\w+@\w+(\.\w+)+$")
