module library


-- A list is emptied.
test a_list_is_emptied
        covers emptied:
    given:
        mutable items = [1, 2, 3]
    when:
        emptied(mutable items)
    then:
        empty = items.is_empty()


-- A letter that is there is removed.
test a_letter_is_forgotten
        covers forget:
    given:
        mutable seen = set.of(['a', 'b'])
    when:
        removed = forget(mutable seen, 'a')
    then:
        was_there = removed
        one_left = (seen.count() == 1)


-- A list's first item.
test the_first_item
        covers first_of:
    when:
        first = first_of([7, 8])
    then:
        seven = (first == optional.some(7))


-- A capital is lowered.
test a_capital_is_lowered
        covers lowered:
    when:
        lower = lowered('Q')
    then:
        small = (lower == 'q')


-- A word holds its letters.
test a_word_holds_its_letter
        covers holds:
    when:
        held = holds(word: "owl", letter: 'w')
    then:
        holds_it = held


-- An address matches the pattern.
test an_address_matches
        covers is_address:
    when:
        shaped = is_address("ada@example.org")
    then:
        matches = shaped
        no_at = (not is_address("example.org"))
