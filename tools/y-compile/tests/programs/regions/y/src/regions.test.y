module regions


-- The count is read in one snapshot.
test the_count_is_read
        covers counted:
    inject:
        tally = shared.new(counter(count: 3))
    when:
        units = counted() or fail
    then:
        three = (units == 3)


-- One is taken from a count that has one.
test one_is_taken
        covers take_one:
    inject:
        tally = shared.new(counter(count: 2))
    when:
        take_one() or fail
    then:
        one_left = (tally.snapshot().count == 1)


-- One is added.
test one_is_added
        covers add_one:
    inject:
        tally = shared.new(counter(count: 0))
    when:
        add_one()
    then:
        one = (tally.snapshot().count == 1)


-- One is taken from a count that has one, with no bound on the wait.
test one_is_taken_eventually
        covers take_one_eventually:
    inject:
        tally = shared.new(counter(count: 1))
        forever = fake forever
    when:
        take_one_eventually()
    then:
        none_left = (tally.snapshot().count == 0)


-- Two tasks add two.
test two_tasks_add_two
        covers add_two:
    inject:
        tally = shared.new(counter(count: 0))
    when:
        add_two() or fail
    then:
        two = (tally.snapshot().count == 2)


-- Two detached tasks add two, which the call waits for.
test two_detached_tasks_add_two
        covers add_two_later:
    inject:
        tally = shared.new(counter(count: 0))
    when:
        add_two_later()
    then:
        two = (tally.snapshot().count == 2)


-- Detached work adds one, which the call waits for.
test detached_work_adds_one
        covers add_one_later:
    inject:
        tally = shared.new(counter(count: 0))
    when:
        add_one_later()
    then:
        one = (tally.snapshot().count == 1)


-- A task cancelled before it starts adds nothing; its sibling adds one, and the region ends.
test a_cancelled_task_does_not_run
        covers add_two:
    inject:
        tally = shared.new(counter(count: 0))
    faults:
        cancel at add_two#first
    when:
        add_two() or fail
    then:
        one = (tally.snapshot().count == 1)

