module shapes


-- Shapes and what they measure, distances, and work that waits.
public facet geometry:
    -- A length in metres.
    unit metre symbol m is uint32


    -- A thousand metres.
    unit kilometre symbol km is 1000 metre


    -- Jobs waiting: taken from the front, in the order they were added at the back.
    type backlog is collection:
        ordered by position
        offers back_add, front_take


    -- Tags seen: no two the same, iterated smallest first.
    type tags_seen is collection:
        unique
        ordered by value


    -- What can be measured, and grown.
    trait measured:
        ----------------------------------------------------------------------------------------------------------------
        The area it covers.

        size: The area, as uint32.
        ----------------------------------------------------------------------------------------------------------------
        function area() returns size

        ----------------------------------------------------------------------------------------------------------------
        Makes it larger.

        by: How much each side grows, as uint32.
        ----------------------------------------------------------------------------------------------------------------
        mutable function grow(by)


    -- A square, by its side.
    structure square:
        side is uint32


    -- A rectangle, by its sides.
    structure rectangle:
        width is uint32
        height is uint32


    -- A square measures its side squared.
    implementation measured for square:
        ----------------------------------------------------------------------------------------------------------------
        The square's area.

        size: The area, as uint32.
        ----------------------------------------------------------------------------------------------------------------
        function area() returns size:
            return size = me.side * me.side

        ----------------------------------------------------------------------------------------------------------------
        Grows the square.

        by: How much its side grows, as uint32.
        ----------------------------------------------------------------------------------------------------------------
        mutable function grow(by):
            me.side := me.side + by


    -- A rectangle measures its width times its height.
    implementation measured for rectangle:
        ----------------------------------------------------------------------------------------------------------------
        The rectangle's area.

        size: The area, as uint32.
        ----------------------------------------------------------------------------------------------------------------
        function area() returns size:
            return size = me.width * me.height

        ----------------------------------------------------------------------------------------------------------------
        Grows the rectangle.

        by: How much each side grows, as uint32.
        ----------------------------------------------------------------------------------------------------------------
        mutable function grow(by):
            me.width := me.width + by
            me.height := me.height + by


    ------------------------------------------------------------------------------------------------------------------------
    The area of anything measured.

    shape: The shape, as measured.

    size: Its area, as uint32.
    ------------------------------------------------------------------------------------------------------------------------
    function area_of(shape) returns size:
        return size = shape.area()


    ------------------------------------------------------------------------------------------------------------------------
    How many squares are larger than a limit: the lambda that judges each captures the limit.

    squares: The squares, as list<element_type=square>.
    limit: The area to exceed, as uint32.

    count: How many exceed it, as count<unit_type=square>.
    ------------------------------------------------------------------------------------------------------------------------
    function larger_than(squares, limit) returns count:
        ------------------------------------------------------------------------------------------------------------
        Whether a square is larger than the limit.

        shape: The square, as square.

        larger: Whether its area exceeds the limit, as boolean.
        ------------------------------------------------------------------------------------------------------------
        is_larger = lambda is_larger(shape) returns larger captures limit:
            if bigger = (shape.area() > limit):
                return larger = true
            else not_bigger:
                return larger = false
        return count = squares.filter(is_larger).count()


    ------------------------------------------------------------------------------------------------------------------------
    The total area of some squares, summed by a nested function that captures the sum to change it.

    squares: The squares, as list<element_type=square>.

    total: Their areas, summed, as uint32.
    ------------------------------------------------------------------------------------------------------------------------
    function total_area(squares) returns total:
        mutable sum = 0

        ------------------------------------------------------------------------------------------------------------
        Adds a square's area to the sum.

        shape: The square, as square.
        ------------------------------------------------------------------------------------------------------------
        nested function add(shape) captures mutable sum:
            sum := sum + shape.area()

        for each in squares:
            add(each)
        else none
        return total = sum


    ------------------------------------------------------------------------------------------------------------------------
    A trip of two legs, in metres: the second converted from kilometres, as its unit says.

    first: The first leg, as metre.
    second: The second leg, as kilometre.

    total: The whole trip, as metre.
    ------------------------------------------------------------------------------------------------------------------------
    function trip(first, second) returns total:
        return total = first + second.to_metre()


    ------------------------------------------------------------------------------------------------------------------------
    The next job, taken from the front of the backlog.

    jobs: The backlog, as backlog<element_type=uint32>.

    job: The job, or nothing when none waits, as optional<value_type=uint32>.
    ------------------------------------------------------------------------------------------------------------------------
    function next_job(mutable jobs) returns job:
        return job = jobs.take()


    ------------------------------------------------------------------------------------------------------------------------
    Marks a tag as seen.

    seen: The tags seen so far, as tags_seen<element_type=uint32>.

    tag: The tag, as uint32.

    new: Whether the tag had not been seen, as boolean.
    ------------------------------------------------------------------------------------------------------------------------
    function mark(mutable seen, tag) returns new:
        return new = seen.add(tag)
