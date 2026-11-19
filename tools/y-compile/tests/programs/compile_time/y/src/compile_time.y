module compile_time


-- Tables computed while the program is built.
public facet tables:
    -- What a table is made of.
    constants table_rules:
        -- How many squares the table holds.
        size is uint32 = 5
        -- The words a list is made of.
        words = "ash elm oak"


    -- Why a word list could not be made.
    constants list_error:
        empty = "empty"


    ------------------------------------------------------------------------------------------------------------------------
    The squares of the numbers from 1 to a limit.

    limit: The largest number, as uint32.

    squares: Each square, in order, as list<element_type=uint32>.
    ------------------------------------------------------------------------------------------------------------------------
    function squares(limit) returns squares:
        mutable found = list.new()

        for n in range(from: 1, through: limit):
            found.push(n * n)
        else none
        return squares = found


    ------------------------------------------------------------------------------------------------------------------------
    The words of a text, or a failure when it has none.

    text: The words, separated by spaces, as string.

    words: Each word, as list<element_type=string>.

    fails list_error.empty: The text has no words.
    ------------------------------------------------------------------------------------------------------------------------
    function words_of(text) returns words fails list_error:
        pieces = text.split_whitespace()

        if empty = pieces.is_empty():
            fail list_error.empty
        else has_words:
            return words = pieces


    ------------------------------------------------------------------------------------------------------------------------
    The square of a number from 1 to 5, read from a table the build computed.

    n: The number, as uint32.

    square: Its square, as uint32.
    ------------------------------------------------------------------------------------------------------------------------
    function square_of(n) returns square:
        table = compile squares(table_rules.size)
        return square = table.at_number(n)


    ------------------------------------------------------------------------------------------------------------------------
    How many squares the table holds, from a count the build computed of a table it computed.

    count: How many, as count<unit_type=uint32>.
    ------------------------------------------------------------------------------------------------------------------------
    function table_size() returns count:
        table = compile squares(table_rules.size)
        return count = table.count()
