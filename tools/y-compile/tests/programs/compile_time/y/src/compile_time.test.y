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


-- The middle of a board of five is its third column and row.
test the_middle_of_five_is_three
        covers centre:
    when:
        middle = centre(5)
    then:
        third = (middle == spot(column: 3, row: 3, name: "centre"))


-- A board's marks hold its middle, a wall along it and a place with nothing.
test a_board_has_three_marks
        covers marks_of:
    when:
        marks = marks_of(3)
    then:
        three = (marks == [mark.piece(at: spot(column: 2, row: 2, name: "centre")), mark.wall(from: 1, to: 3), mark.nothing])


-- A board's edges are its first and last numbers.
test the_edges_are_the_first_and_last
        covers edges_of:
    when:
        edges = edges_of(4)
    then:
        both = (edges == set.of([1, 4]))


-- A board's edges are named first and last.
test the_edges_are_named
        covers names_of:
    when:
        names = names_of(4)
    then:
        two = (names.count() == 2)


-- The table's middle is the one the build computed.
test the_table_centre_is_built
        covers table_centre:
    when:
        middle = table_centre()
    then:
        third = (middle.column == 3)


-- The table's marks are the ones the build computed.
test the_table_marks_are_built
        covers table_marks:
    when:
        marks = table_marks()
    then:
        three = (marks.count() == 3)


-- The table's edges are 1 and 5.
test the_table_edges_are_built
        covers is_table_edge:
    when:
        edge = is_table_edge(5)
    then:
        five = edge


-- The table names both its edges.
test the_table_names_are_built
        covers table_names:
    when:
        named = table_names()
    then:
        two = (named == 2)
