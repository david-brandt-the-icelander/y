module claims


-- What the compiler cannot check, each claimed with its reason.
public facet claimed:
    -- A counter whose representation the host gives.
    type tally_cell native because "a Rust Cell<u32>: a copy of it is independent, and it holds nothing to release"


    ------------------------------------------------------------------------------------------------------------------------
    The CRC-32 of a text's UTF-8.

    text: The text, as string.

    crc: Its CRC-32, as uint32.
    ------------------------------------------------------------------------------------------------------------------------
    function checksum(text) returns crc
            native because "the host computes CRC-32 by the standard polynomial, and touches nothing else"


    ------------------------------------------------------------------------------------------------------------------------
    A counter at a number.

    start: Where it starts, as uint32.

    cell: The counter, as tally_cell.
    ------------------------------------------------------------------------------------------------------------------------
    function new_cell(start) returns cell
            native because "a Cell holds the value it is given"


    ------------------------------------------------------------------------------------------------------------------------
    What a counter holds.

    cell: The counter, as tally_cell.

    count: What it holds, as uint32.
    ------------------------------------------------------------------------------------------------------------------------
    function count_of(cell) returns count
            native because "reading a Cell changes nothing"


    ------------------------------------------------------------------------------------------------------------------------
    How long a secret is: the secret is revealed to count it, and only its length leaves.

    secret: The secret, as sensitive<value_type=string>.

    length: How many runes it has, as count<unit_type=rune>.

    reveal: The right to see it, as reveal.
    ------------------------------------------------------------------------------------------------------------------------
    function secret_length(secret, injected reveal) returns length:
        log debug "counting {secret}"
        shown = reveal(secret) because "only its length leaves this function"
        return length = shown.count()


    ------------------------------------------------------------------------------------------------------------------------
    The smallest of some numbers that are sorted.

    numbers: The numbers, smallest first, as list<element_type=uint32>.

    least: The smallest, as uint32.
    ------------------------------------------------------------------------------------------------------------------------
    function smallest(numbers) returns least
            where some = (not numbers.is_empty()):
        assume sorted = (numbers.at_number(1) <= numbers.at_number(numbers.count())) because "every caller passes them sorted"
        allow(Y2031) because "kept to show where a suppressed lint is claimed"
        return least = numbers.at_number(1)
