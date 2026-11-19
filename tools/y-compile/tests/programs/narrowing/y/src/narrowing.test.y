module narrowing


-- A word that is there and has letters is long.
test a_present_word_with_letters_is_long
        covers present_and_long > long:
    when:
        long = present_and_long(optional.some("dragon"))
    then:
        is_long = long


-- An absent word is short at the first operand.
test an_absent_word_is_short
        covers present_and_long > short'1:
    when:
        long = present_and_long(optional.none)
    then:
        is_short = (not long)


-- An empty word is short at the second operand.
test an_empty_word_is_short
        covers present_and_long > short'2:
    when:
        long = present_and_long(optional.some(""))
    then:
        is_short = (not long)


-- An absent word has no letters.
test an_absent_word_has_no_letters
        covers letters_of > nothing'1:
    when:
        letters = letters_of(optional.none)
    then:
        none = (letters == 0)


-- An empty word has no letters.
test an_empty_word_has_no_letters
        covers letters_of > nothing'2:
    when:
        letters = letters_of(optional.some(""))
    then:
        none = (letters == 0)


-- A word has its letters.
test a_word_has_its_letters
        covers letters_of > something:
    when:
        letters = letters_of(optional.some("owl"))
    then:
        three = (letters == 3)


-- A number that was read is its value.
test a_read_number_is_its_value
        covers value_or_zero > read:
    when:
        value = value_or_zero(result.ok(7))
    then:
        seven = (value == 7)


-- A number that was not read is 0.
test an_unread_number_is_zero
        covers value_or_zero > unread:
    when:
        value = value_or_zero(result.err(lookup_error.closed))
    then:
        zero = (value == 0)


-- A small key is found as its double.
test a_small_key_is_doubled
        covers look_up > which.small:
    when:
        found = look_up(4) or fail
    then:
        eight = (found == 8)


-- Thirteen is missing.
test thirteen_is_missing
        covers look_up > which.unlucky:
    when:
        found = look_up(13)
    then:
        missing = (found == result.err(lookup_error.missing(key: 13)))


-- A large key is closed.
test a_large_key_is_closed
        covers look_up > which.large:
    when:
        found = look_up(20)
    then:
        closed = (found == result.err(lookup_error.closed))


-- A key that is found answers with its value.
test a_found_key_answers_its_value
        covers answer_for > looked.found:
    when:
        answer = answer_for(3)
    then:
        six = (answer == 6)


-- A missing key answers with the key, read from the failure's payload.
test a_missing_key_answers_the_key
        covers answer_for > looked.missing:
    when:
        answer = answer_for(13)
    then:
        thirteen = (answer == 13)


-- A closed lookup answers 0.
test a_closed_lookup_answers_zero
        covers answer_for > looked.closed:
    when:
        answer = answer_for(50)
    then:
        zero = (answer == 0)


-- A circle is twice its radius wide.
test a_circle_is_twice_its_radius
        covers width_of > measured.round:
    when:
        width = width_of(shape.circle(radius: 4))
    then:
        eight = (width == 8)


-- A square is its side wide.
test a_square_is_its_side
        covers width_of > measured.boxy:
    when:
        width = width_of(shape.square(side: 5))
    then:
        five = (width == 5)


-- An absent word has size 0.
test an_absent_word_has_size_zero
        covers size_of > checked.absent:
    when:
        size = size_of(optional.none)
    then:
        zero = (size == 0)


-- An empty word has size 1.
test an_empty_word_has_size_one
        covers size_of > checked.empty:
    when:
        size = size_of(optional.some(""))
    then:
        one = (size == 1)


-- A word's size is its letters.
test a_word_has_its_size
        covers size_of > checked.present:
    when:
        size = size_of(optional.some("lark"))
    then:
        four = (size == 4)


-- An absent word is never counted.
test an_absent_word_is_never_counted
        covers counted_one_by_one > never'1:
    when:
        counted = counted_one_by_one(optional.none)
    then:
        zero = (counted == 0)


-- An empty word is never counted.
test an_empty_word_is_never_counted
        covers counted_one_by_one > never'2:
    when:
        counted = counted_one_by_one(optional.some(""))
    then:
        zero = (counted == 0)


-- A word of one rune is counted in one pass.
test a_word_of_one_rune_is_counted_once
        covers counted_one_by_one > going:
    when:
        counted = counted_one_by_one(optional.some("a"))
    then:
        one = (counted == 1)


-- A longer word is counted in several passes.
test a_longer_word_is_counted_in_several
        covers counted_one_by_one > going'many:
    when:
        counted = counted_one_by_one(optional.some("owls"))
    then:
        four = (counted == 4)
