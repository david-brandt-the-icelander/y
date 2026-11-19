module narrowing


-- What conditions show of optionals, results and cases.
public facet narrowed:
    -- Why a value could not be looked up.
    type lookup_error:
        case missing(key is uint32)
        case closed


    -- A shape, by its size.
    type shape:
        case circle(radius is uint32)
        case square(side is uint32)


    ------------------------------------------------------------------------------------------------------------------------
    Whether a word is there and has a letter: the first operand of `and` narrows the word for the second.

    word: The word, if there is one, as optional<value_type=string>.

    has_letters: Whether it is there and is not empty, as boolean.
    ------------------------------------------------------------------------------------------------------------------------
    function present_and_long(word) returns has_letters:
        if long = ((word != optional.none) and (word.count() > 0)):
            return has_letters = true
        else short:
            return has_letters = false


    ------------------------------------------------------------------------------------------------------------------------
    How many letters a word has: none when it is absent, the first operand of `or` narrowing it for the else.

    word: The word, if there is one, as optional<value_type=string>.

    letters: Its letters, as count<unit_type=rune>.
    ------------------------------------------------------------------------------------------------------------------------
    function letters_of(word) returns letters:
        if nothing = ((word == optional.none) or (word.count() == 0)):
            return letters = 0
        else something:
            return letters = word.count()


    ------------------------------------------------------------------------------------------------------------------------
    A result's value, or 0: an `if` on a result narrows it in each branch.

    parsed: A number that was read, or why not, as result<value_type=uint32, failure_type=lookup_error>.

    value: The number, or 0, as uint32.
    ------------------------------------------------------------------------------------------------------------------------
    function value_or_zero(parsed) returns value:
        if read = (parsed == result.ok):
            return value = parsed.value
        else unread:
            return value = 0


    ------------------------------------------------------------------------------------------------------------------------
    Looks a key up: a key below 10 is found as its double, 13 is missing, and the others are closed.

    key: The key, as uint32.

    found: Its value, as uint32.

    fails lookup_error.missing: The key is 13.
    fails lookup_error.closed: The key is 10 or more and not 13.
    ------------------------------------------------------------------------------------------------------------------------
    function look_up(key) returns found fails lookup_error:
        conditional which:
            condition which.small = (key < 10):
                return found = key + key
            condition which.unlucky = (key == 13):
                fail lookup_error.missing(key: key)
            else which.large:
                fail lookup_error.closed


    ------------------------------------------------------------------------------------------------------------------------
    What a lookup gives, as a number: a condition on a case with its payload narrows the label to it.

    key: The key, as uint32.

    answer: The value found, the missing key, or 0, as uint32.
    ------------------------------------------------------------------------------------------------------------------------
    function answer_for(key) returns answer:
        conditional looked = look_up(key):
            condition looked.found = (looked == result.ok):
                return answer = looked.value
            condition looked.missing = (looked == result.err(lookup_error.missing)):
                return answer = looked.failure.key
            condition looked.closed = (looked == result.err(lookup_error.closed)):
                return answer = 0


    ------------------------------------------------------------------------------------------------------------------------
    How wide a shape is: a condition on a case narrows the label to it.

    drawn: The shape, as shape.

    width: Its width, as uint32.
    ------------------------------------------------------------------------------------------------------------------------
    function width_of(drawn) returns width:
        conditional measured = drawn:
            condition measured.round = (measured == shape.circle):
                return width = measured.radius + measured.radius
            condition measured.boxy = (measured == shape.square):
                return width = measured.side


    ------------------------------------------------------------------------------------------------------------------------
    A word's first letter count, and what conditions that did not hold show: once the first finds the word absent no
    longer, the others read it.

    word: The word, if there is one, as optional<value_type=string>.

    size: 0 when it is absent, 1 when it is empty, and its letters otherwise, as count<unit_type=rune>.
    ------------------------------------------------------------------------------------------------------------------------
    function size_of(word) returns size:
        conditional checked:
            condition checked.absent = (word == optional.none):
                return size = 0
            condition checked.empty = (word.count() == 0):
                return size = 1
            else checked.present:
                return size = word.count()


    ------------------------------------------------------------------------------------------------------------------------
    Counts a word's runes one at a time: the loop's condition narrows the word for its block.

    word: The word, if there is one, as optional<value_type=string>.

    counted: How many runes it has, or 0 when it is absent, as count<unit_type=rune>.
    ------------------------------------------------------------------------------------------------------------------------
    function counted_one_by_one(word) returns counted:
        mutable count = 0

        while going = ((word != optional.none) and (count < word.count())):
            count := count + word.count().min(1)
        else never
        return counted = count
