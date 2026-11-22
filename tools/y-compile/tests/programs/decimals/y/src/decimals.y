module decimals


-- Arithmetic whose results keep every digit, or round as named.
public facet sums:
    ------------------------------------------------------------------------------------------------------------------------
    Adds two prices of different scales: the sum has the larger scale and room for a carry.

    price: A price, as decimal<precision=6, scale=2>.
    fee: A fee, to the tenth of a cent, as decimal<precision=8, scale=3>.

    total: Their sum, as decimal<precision=9, scale=3>.
    ------------------------------------------------------------------------------------------------------------------------
    function total_of(price, fee) returns total:
        return total = price + fee


    ------------------------------------------------------------------------------------------------------------------------
    A price times a rate: the scales and the precisions add.

    price: A price, as decimal<precision=6, scale=2>.
    rate: A rate, as decimal<precision=5, scale=4>.

    taxed: Their product, as decimal<precision=11, scale=6>.
    ------------------------------------------------------------------------------------------------------------------------
    function taxed(price, rate) returns taxed:
        return taxed = price * rate


    ------------------------------------------------------------------------------------------------------------------------
    A price times a quantity: the price's scale, and the quantity's digits added to its precision.

    price: A price, as decimal<precision=6, scale=2>.
    quantity: How many, as integer<minimum=0, maximum=99>.

    cost: Their product, as decimal<precision=8, scale=2>.
    ------------------------------------------------------------------------------------------------------------------------
    function cost(price, quantity) returns cost:
        return cost = price * quantity


    ------------------------------------------------------------------------------------------------------------------------
    Shares a bill: the quotient at the bill's scale, rounded half to even.

    bill: The bill, as decimal<precision=6, scale=2>.
    payers: How many pay, as a decimal, as decimal<precision=2, scale=0>.

    each: What each pays, as decimal<precision=6, scale=2>.
    ------------------------------------------------------------------------------------------------------------------------
    function share(bill, payers) returns each:
        return each = bill.divide_rounded(payers, rounding.half_even)


    ------------------------------------------------------------------------------------------------------------------------
    Shares a bill exactly, or not at all.

    bill: The bill, as decimal<precision=6, scale=2>.
    payers: How many pay, as decimal<precision=2, scale=0>.

    each: What each pays, as decimal<precision=6, scale=2>.

    fails inexact: The bill does not divide to the cent.
    ------------------------------------------------------------------------------------------------------------------------
    function exact_share(bill, payers) returns each fails inexact:
        return each = bill.divide_exact(payers) or fail


    ------------------------------------------------------------------------------------------------------------------------
    A third of an amount, kept exact as a rational, and then put back as a decimal exactly, or not at all.

    amount: The amount, as decimal<precision=6, scale=2>.

    third: A third of it, as decimal<precision=6, scale=2>.

    fails inexact: A third of the amount is not a whole number of cents.
    ------------------------------------------------------------------------------------------------------------------------
    function third_of(amount) returns third fails inexact:
        one_third = rational.new(numerator: 1, denominator: 3)
        exact = amount.to_rational() * one_third
        return third = exact.to_decimal_exact(scale: 2) or fail


    ------------------------------------------------------------------------------------------------------------------------
    Whether one price is below another, compared exactly across their scales.

    price: A price, as decimal<precision=6, scale=2>.
    limit: The limit, as decimal<precision=8, scale=3>.

    below: Whether the price is below the limit, as boolean.
    ------------------------------------------------------------------------------------------------------------------------
    function is_below(price, limit) returns below:
        if cheaper = (price < limit):
            return below = true
        else dearer:
            return below = false


    ------------------------------------------------------------------------------------------------------------------------
    A price in whole units, rounded half away from zero.

    price: A price, as decimal<precision=6, scale=2>.

    units: The whole units, as integer<minimum=-10000, maximum=10000>.
    ------------------------------------------------------------------------------------------------------------------------
    function whole_units(price) returns units:
        return units = price.to_integer_rounded(rounding.half_away_from_zero)


    ------------------------------------------------------------------------------------------------------------------------
    The sum of two small counts: its range is the sum of theirs.

    a: A count, as integer<minimum=0, maximum=10>.
    b: Another, as integer<minimum=0, maximum=10>.

    total: Their sum, as integer<minimum=0, maximum=20>.
    ------------------------------------------------------------------------------------------------------------------------
    function sum_of(a, b) returns total:
        return total = a + b


    ------------------------------------------------------------------------------------------------------------------------
    A price as the nearest float64.

    price: A price, as decimal<precision=6, scale=2>.

    near: The float64 nearest it, as float64.
    ------------------------------------------------------------------------------------------------------------------------
    function nearest(price) returns near:
        return near = price.to_float64_nearest()
