module ensures


-- A negative number is kept at 0.
test a_negative_number_is_kept_at_zero
        covers clamped > low:
    when:
        kept = clamped(-5)
    then:
        zero = (kept == 0)


-- A large number is kept at 100.
test a_large_number_is_kept_at_a_hundred
        covers clamped > not_low'gt > high:
    when:
        kept = clamped(150)
    then:
        hundred = (kept == 100)


-- A number in range is kept as it is.
test a_number_in_range_is_kept
        covers clamped > not_low'gt > in_range'lt:
    when:
        kept = clamped(42)
    then:
        same = (kept == 42)


-- An empty list has no even number.
test an_empty_list_has_no_even_number
        covers first_even > none:
    when:
        even = first_even([])
    then:
        zero = (even == 0)


-- An even first number is found on the first pass.
test an_even_first_number_is_found
        covers first_even > number > divisible:
    when:
        even = first_even([4])
    then:
        four = (even == 4)


-- One odd number has no even number after it.
test one_odd_number_has_none
        covers first_even > number > indivisible:
    when:
        even = first_even([3])
    then:
        zero = (even == 0)


-- An even number after an odd one is found on a later pass.
test an_even_number_after_an_odd_one_is_found
        covers first_even > number'many > divisible:
    when:
        even = first_even([3, 8])
    then:
        eight = (even == 8)


-- Odd numbers only have no even number.
test odd_numbers_have_none
        covers first_even > number'many > indivisible:
    when:
        even = first_even([3, 5])
    then:
        zero = (even == 0)


-- An even number is halved.
test an_even_number_is_halved
        covers halved > even:
    when:
        half = halved(10) or fail
    then:
        five = (half == 5)


-- An odd number is not.
test an_odd_number_is_not_halved
        covers halved > odd:
    when:
        half = halved(7)
    then:
        odd = (half == result.err(halving_error.odd))


-- 0 is kept as it is.
test zero_is_kept
        covers clamped > not_low'eq > in_range:
    when:
        kept = clamped(0)
    then:
        zero = (kept == 0)


-- 100 is kept as it is.
test a_hundred_is_kept
        covers clamped > not_low'gt > in_range'eq:
    when:
        kept = clamped(100)
    then:
        hundred = (kept == 100)


-- 6 is even.
test six_is_even
        covers is_even:
    when:
        even = is_even(6)
    then:
        is_even = even
