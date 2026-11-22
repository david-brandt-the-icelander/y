module generics


-- Declarations that leave a type open, filled where they are used (Y § 3.1).
public facet kept:
    -- What has a weight.
    trait weighed:
        ----------------------------------------------------------------------------------------------------------------
        Its weight.

        weight: The weight, as uint32.
        ----------------------------------------------------------------------------------------------------------------
        function weight() returns weight

        ----------------------------------------------------------------------------------------------------------------
        Whether it weighs as much as something else that is weighed, of any kind.

        other: The other, as other_type.

        other_type: What the other is, as weighed.

        same: Whether they weigh the same, as boolean.
        ----------------------------------------------------------------------------------------------------------------
        function balances(other) returns same


    -- A parcel of some weight.
    structure parcel:
        grams is uint32


    -- Two things of one kind, carried together.
    structure pair:
        item_type is weighed
        first is item_type
        second is item_type


    -- A parcel weighs its grams.
    implementation weighed for parcel:
        ----------------------------------------------------------------------------------------------------------------
        The parcel's weight.

        weight: Its grams, as uint32.
        ----------------------------------------------------------------------------------------------------------------
        function weight() returns weight:
            return weight = me.grams

        ----------------------------------------------------------------------------------------------------------------
        Whether the parcel weighs what the other does.

        other: The other, as other_type.

        other_type: What the other is, as weighed.

        same: Whether they weigh the same, as boolean.
        ----------------------------------------------------------------------------------------------------------------
        function balances(other) returns same:
            return same = (me.weight() == other.weight())


    -- A pair weighs what both of its things weigh.
    implementation weighed for pair:
        ----------------------------------------------------------------------------------------------------------------
        The pair's weight.

        weight: The sum of its things' weights, as uint32.
        ----------------------------------------------------------------------------------------------------------------
        function weight() returns weight:
            return weight = me.first.weight() + me.second.weight()

        ----------------------------------------------------------------------------------------------------------------
        Whether the pair weighs what the other does.

        other: The other, as other_type.

        other_type: What the other is, as weighed.

        same: Whether they weigh the same, as boolean.
        ----------------------------------------------------------------------------------------------------------------
        function balances(other) returns same:
            return same = (me.weight() == other.weight())


    ------------------------------------------------------------------------------------------------------------------------
    The larger of two values of any ordered type.

    left: One value, as item_type.
    right: The other, as item_type.

    item_type: What the values are, as ordered.

    larger: The larger, as item_type.
    ------------------------------------------------------------------------------------------------------------------------
    function larger(left, right) returns larger:
        if first = (left >= right):
            return larger = left
        else second:
            return larger = right


    ------------------------------------------------------------------------------------------------------------------------
    The heavier of two things of one weighed kind.

    left: One thing, as item_type.
    right: The other, as item_type.

    item_type: What the things are, as weighed.

    heavier: The heavier, as item_type.
    ------------------------------------------------------------------------------------------------------------------------
    function heavier(left, right) returns heavier:
        if left_heavier = (left.weight() > right.weight()):
            return heavier = left
        else right_heavier:
            return heavier = right


    ------------------------------------------------------------------------------------------------------------------------
    Two things carried together.

    first: One thing, as item_type.
    second: The other, as item_type.

    item_type: What the things are, as weighed.

    carried: The two, as pair<item_type=item_type>.
    ------------------------------------------------------------------------------------------------------------------------
    function paired(first, second) returns carried:
        return carried = pair(first: first, second: second)


    ------------------------------------------------------------------------------------------------------------------------
    Whether a list of values of any type holds one.

    values: The values, as list<element_type=value_type>.
    wanted: The value looked for, as value_type.

    value_type: What the values are, as equatable.

    held: Whether it is there, as boolean.
    ------------------------------------------------------------------------------------------------------------------------
    function holds(values, wanted) returns held:
        return held = values.contains(wanted)
