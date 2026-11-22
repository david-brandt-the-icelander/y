module ensures


-- What functions ensure, on every way they return.
public facet bounded:
    -- Why a number could not be halved.
    constants halving_error:
        odd = "odd"


    ------------------------------------------------------------------------------------------------------------------------
    Whether a number is even.

    n: The number, as uint32.

    even: Whether it is, as boolean.
    ------------------------------------------------------------------------------------------------------------------------
    function is_even(n) returns even:
        return even = (n.divide_floor(2) * 2 == n)


    ------------------------------------------------------------------------------------------------------------------------
    A number kept between 0 and 100: it returns early at either end, and falls through in the middle.

    n: The number, as int32.

    kept: The number, within 0 and 100, as int32.
    ------------------------------------------------------------------------------------------------------------------------
    function clamped(n) returns kept
            ensures within = ((kept >= 0) and (kept <= 100)):
        if low = (n < 0):
            return kept = 0
        else not_low:
            if high = (n > 100):
                return kept = 100
            else in_range:
                return kept = n


    ------------------------------------------------------------------------------------------------------------------------
    The first even number of a list, from a loop that returns from inside it.

    numbers: The numbers, as list<element_type=uint32>.

    even: The first even number, or 0, as uint32.
    ------------------------------------------------------------------------------------------------------------------------
    function first_even(numbers) returns even
            ensures even_or_none = is_even(even):
        for number in numbers:
            if divisible = is_even(number):
                return even = number
            else indivisible:
                continue
        else none
        return even = 0


    ------------------------------------------------------------------------------------------------------------------------
    Half a number, which fails for an odd one; what it ensures holds when it succeeds.

    n: The number, as uint32.

    half: Half of it, as uint32.

    fails halving_error.odd: The number is odd.
    ------------------------------------------------------------------------------------------------------------------------
    function halved(n) returns half fails halving_error
            ensures doubles_back = (half + half == n):
        if odd = (not is_even(n)):
            fail halving_error.odd
        else even:
            return half = n.divide_floor(2)
