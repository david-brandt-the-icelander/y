module generics


-- The larger of two numbers, of two texts: one function, filled with each type where it is called.
test the_larger_of_two_values
        covers larger > first:
    when:
        number = larger(7, 3)
        word = larger("pear", "apple")
    then:
        seven = (number == 7)
        pear = (word == "pear")


-- The larger is the second when it is larger.
test the_larger_is_the_second
        covers larger > second:
    when:
        number = larger(3, 7)
    then:
        seven = (number == 7)


-- A parcel and a pair weigh what they hold.
test parcels_and_pairs_are_weighed
        covers weight:
    when:
        one = parcel(grams: 3).weight()
        both = paired(parcel(grams: 1), parcel(grams: 2)).weight()
    then:
        three = (one == 3)
        also_three = (both == 3)


-- A parcel balances a pair of the same weight: the trait's method takes a weighed thing of any kind.
test a_parcel_balances_a_pair
        covers balances:
    when:
        level = parcel(grams: 3).balances(paired(parcel(grams: 1), parcel(grams: 2)))
        also_level = paired(parcel(grams: 2), parcel(grams: 2)).balances(parcel(grams: 4))
    then:
        balanced = level
        also_balanced = also_level


-- Two parcels are carried as a pair.
test two_parcels_are_paired
        covers paired:
    when:
        carried = paired(parcel(grams: 1), parcel(grams: 2))
    then:
        first_is_one = (carried.first.grams == 1)


-- The heavier of two parcels is the one with more grams.
test the_heavier_parcel_is_the_first
        covers heavier > left_heavier:
    when:
        found = heavier(parcel(grams: 5), parcel(grams: 2))
    then:
        five = (found.grams == 5)


-- The heavier of two pairs is the second when it weighs more.
test the_heavier_pair_is_the_second
        covers heavier > right_heavier:
    when:
        found = heavier(paired(parcel(grams: 1), parcel(grams: 1)), paired(parcel(grams: 2), parcel(grams: 2)))
    then:
        four = (found.weight() == 4)


-- A list of texts holds a text, and a list of numbers a number.
test lists_of_any_type_hold_values
        covers holds:
    when:
        has_pear = holds(["apple", "pear"], "pear")
        has_nine = holds([1, 2, 3], 9)
    then:
        pear = has_pear
        no_nine = (not has_nine)
