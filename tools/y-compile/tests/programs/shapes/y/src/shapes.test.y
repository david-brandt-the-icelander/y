module shapes


-- A square's area is its side squared, and a rectangle's its sides multiplied.
test areas_are_measured
        covers area:
    when:
        square_area = square(side: 3).area()
        rectangle_area = rectangle(width: 2, height: 5).area()
    then:
        nine = (square_area == 9)
        ten = (rectangle_area == 10)


-- A square grows.
test a_square_grows
        covers grow:
    given:
        mutable grown = square(side: 3)
    when:
        grown.grow(by: 2)
    then:
        five = (grown.side == 5)


-- The area of anything measured is its own.
test the_area_of_a_measured_thing
        covers area_of:
    when:
        size = area_of(rectangle(width: 4, height: 4))
    then:
        sixteen = (size == 16)


-- The squares larger than a limit are counted.
test larger_squares_are_counted
        covers larger_than:
    when:
        count = larger_than(squares: [square(side: 1), square(side: 5)], limit: 4)
    then:
        one = (count == 1)


-- A large square is larger.
test a_large_square_is_larger
        covers is_larger > bigger:
    when:
        count = larger_than(squares: [square(side: 5)], limit: 4)
    then:
        one = (count == 1)


-- A small square is not.
test a_small_square_is_not_larger
        covers is_larger > not_bigger:
    when:
        count = larger_than(squares: [square(side: 1)], limit: 4)
    then:
        none = (count == 0)


-- No squares have no area.
test no_squares_have_no_area
        covers total_area > none:
    when:
        total = total_area([])
    then:
        zero = (total == 0)


-- One square's area is its own.
test one_square_s_area
        covers total_area > each:
    when:
        total = total_area([square(side: 2)])
    then:
        four = (total == 4)


-- Several squares' areas add.
test several_squares_areas_add
        covers total_area > each'many:
    when:
        total = total_area([square(side: 2), square(side: 3)])
    then:
        thirteen = (total == 13)


-- The nested function adds a square's area.
test a_square_s_area_is_added
        covers add:
    when:
        total = total_area([square(side: 1)])
    then:
        one = (total == 1)


-- A trip adds its legs, in metres.
test a_trip_adds_its_legs
        covers trip:
    when:
        total = trip(first: 500m, second: 2km)
    then:
        in_metres = (total == 2500m)


-- The next job is the first added.
test the_next_job_is_the_first_added
        covers next_job:
    given:
        mutable waiting = queue.new()
        waiting.add(3)
        waiting.add(4)
    when:
        job = next_job(mutable waiting)
    then:
        three = (job == optional.some(3))


-- A tag seen twice is new once.
test a_tag_is_new_once
        covers mark:
    given:
        mutable seen = sorted_set.new()
    when:
        first = mark(mutable seen, 5)
        again = mark(mutable seen, 5)
    then:
        new_then_old = (first and not again)
        one = (seen.count() == 1)
