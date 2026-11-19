module compile_time


-- No squares up to 0.
test there_are_no_squares_up_to_zero
        covers squares > none:
    when:
        found = squares(0)
    then:
        empty = found.is_empty()


-- One square up to 1.
test one_is_its_own_square
        covers squares > n:
    when:
        found = squares(1)
    then:
        just_one = (found == [1])


-- The squares up to 3.
test the_squares_up_to_three
        covers squares > n'many:
    when:
        found = squares(3)
    then:
        three = (found == [1, 4, 9])


-- A text of words has them.
test a_text_has_its_words
        covers words_of > has_words:
    when:
        words = words_of("ash elm") or fail
    then:
        two = (words.count() == 2)


-- A blank text has no words.
test a_blank_text_has_none
        covers words_of > empty:
    when:
        words = words_of("  ")
    then:
        empty = (words == result.err(list_error.empty))


-- The square of 4 is read from the table.
test four_squared_is_sixteen
        covers square_of:
    when:
        square = square_of(4)
    then:
        sixteen = (square == 16)


-- The table holds five squares.
test the_table_holds_five
        covers table_size:
    when:
        count = table_size()
    then:
        five = (count == 5)
