module decimals


-- A sum keeps the fee's tenth of a cent.
test a_sum_keeps_every_digit
        covers total_of:
    when:
        total = total_of(price: 4.50, fee: 0.125)
    then:
        exact = (total == 4.625)


-- A product's scale is the sum of the scales.
test a_product_keeps_every_digit
        covers taxed:
    when:
        taxed = taxed(price: 10.25, rate: 0.2400)
    then:
        exact = (taxed == 2.460000)


-- A price times a quantity keeps the price's scale.
test a_cost_keeps_the_price_scale
        covers cost:
    when:
        cost = cost(price: 2.25, quantity: 4)
    then:
        nine = (cost == 9.00)


-- A share is rounded half to even.
test a_share_rounds_half_to_even
        covers share:
    when:
        each = share(bill: 10.00, payers: 3)
    then:
        to_the_cent = (each == 3.33)


-- An even bill shares exactly.
test an_even_bill_shares_exactly
        covers exact_share:
    when:
        each = exact_share(bill: 10.00, payers: 4) or fail
    then:
        two_fifty = (each == 2.50)


-- A bill that does not divide to the cent does not share exactly.
test an_uneven_bill_does_not_share_exactly:
    when:
        each = exact_share(bill: 10.00, payers: 3)
    then:
        inexact = (each == result.err)


-- A third of an amount of whole thirds is exact.
test a_third_is_exact
        covers third_of:
    when:
        third = third_of(3.00) or fail
    then:
        one = (third == 1.00)


-- A third of a cent is not.
test a_third_of_a_cent_is_not_exact:
    when:
        third = third_of(0.01)
    then:
        inexact = (third == result.err)


-- A price below the limit is below it.
test a_lower_price_is_below
        covers is_below > cheaper:
    when:
        below = is_below(price: 1.99, limit: 2.000)
    then:
        is_below = below


-- A price at the limit is not below it.
test a_price_at_the_limit_is_not_below
        covers is_below > dearer:
    when:
        below = is_below(price: 2.00, limit: 2.000)
    then:
        not_below = (not below)


-- A price over the limit is not below it.
test a_higher_price_is_not_below
        covers is_below > dearer:
    when:
        below = is_below(price: 2.01, limit: 2.005)
    then:
        not_below = (not below)


-- Half a unit rounds away from zero.
test half_a_unit_rounds_away_from_zero
        covers whole_units:
    when:
        units = whole_units(2.50)
    then:
        three = (units == 3)


-- Two counts add.
test two_counts_add
        covers sum_of:
    when:
        total = sum_of(a: 7, b: 9)
    then:
        sixteen = (total == 16)


-- A price is near its float64.
test a_price_is_near_its_float
        covers nearest:
    when:
        near = nearest(1.50)
    then:
        one_and_a_half = (near == 1.5)
