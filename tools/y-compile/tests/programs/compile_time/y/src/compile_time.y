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


-- Values of every kind the build writes as data: a structure, a closed type with payloads, a set and a map.
public facet shapes:
    -- A spot on a board, with its name.
    structure spot:
        column is uint32
        row is uint32
        name is string


    -- What a board holds at a place.
    type mark:
        case piece(at is spot)
        case wall(from is uint32, to is uint32)
        case nothing


    ------------------------------------------------------------------------------------------------------------------------
    The spot in the middle of a board.

    size: How many columns and rows the board has, as uint32.

    middle: The middle spot, as spot.
    ------------------------------------------------------------------------------------------------------------------------
    function centre(size) returns middle:
        half = (size + 1) / 2
        return middle = spot(column: half, row: half, name: "centre")


    ------------------------------------------------------------------------------------------------------------------------
    The marks of a board: a piece in its middle, a wall along it, and a place with nothing.

    size: How many columns and rows the board has, as uint32.

    marks: The marks, as list<element_type=mark>.
    ------------------------------------------------------------------------------------------------------------------------
    function marks_of(size) returns marks:
        middle = centre(size)
        return marks = [mark.piece(at: middle), mark.wall(from: 1, to: size), mark.nothing]


    ------------------------------------------------------------------------------------------------------------------------
    The numbers of a board's edges.

    size: How many columns and rows the board has, as uint32.

    edges: The first and the last, as set<element_type=uint32>.
    ------------------------------------------------------------------------------------------------------------------------
    function edges_of(size) returns edges:
        return edges = set.of([1, size])


    ------------------------------------------------------------------------------------------------------------------------
    The names of a board's edges, by their numbers.

    size: How many columns and rows the board has, as uint32.

    names: Each edge's name, as map<key_type=uint32, value_type=string>.
    ------------------------------------------------------------------------------------------------------------------------
    function names_of(size) returns names:
        mutable named = map.new()
        named.set(1, "first")
        named.set(size, "last")
        return names = named


    ------------------------------------------------------------------------------------------------------------------------
    The middle of the table's board, from a spot the build computed.

    middle: The middle spot, as spot.
    ------------------------------------------------------------------------------------------------------------------------
    function table_centre() returns middle:
        built = compile centre(table_rules.size)
        return middle = built


    ------------------------------------------------------------------------------------------------------------------------
    The marks of the table's board, which the build computed.

    marks: The marks, as list<element_type=mark>.
    ------------------------------------------------------------------------------------------------------------------------
    function table_marks() returns marks:
        built = compile marks_of(table_rules.size)
        return marks = built


    ------------------------------------------------------------------------------------------------------------------------
    Whether a number is an edge of the table's board, from a set the build computed.

    n: The number, as uint32.

    edge: Whether it is an edge, as boolean.
    ------------------------------------------------------------------------------------------------------------------------
    function is_table_edge(n) returns edge:
        edges = compile edges_of(table_rules.size)
        return edge = edges.contains(n)


    ------------------------------------------------------------------------------------------------------------------------
    How many of the table's board's edges are named, from a map the build computed.

    named: How many, as count<unit_type=uint32>.
    ------------------------------------------------------------------------------------------------------------------------
    function table_names() returns named:
        names = compile names_of(table_rules.size)
        return named = names.count()
