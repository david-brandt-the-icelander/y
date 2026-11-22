module regions

from standard.wait import forever


-- Regions over a shared counter, with the constraints that go together.
public facet counting:
    -- A count that tasks share.
    structure counter:
        count is uint32


    -- Why a region stopped.
    type region_error:
        case expired(deadline is expired)


    ------------------------------------------------------------------------------------------------------------------------
    Reads the count from one consistent snapshot, within a second.

    units: The count, as uint32.

    tally: The counter, as shared<value_type=counter>.

    fails region_error.expired: The read took longer than a second.
    ------------------------------------------------------------------------------------------------------------------------
    function counted(injected tally) returns units fails region_error:
        region read_once is atomic, isolation snapshot, within 1s or fail:
            return units = tally.count


    ------------------------------------------------------------------------------------------------------------------------
    Takes one from the count once it has one, waiting no longer than a second.

    tally: The counter, as shared<value_type=counter>.

    fails region_error.expired: The count stayed at 0 for a second.
    ------------------------------------------------------------------------------------------------------------------------
    function take_one(injected tally) fails region_error:
        region taken is atomic, waits until some = (tally.count > 0), within 1s or fail:
            tally.count := tally.count - 1


    ------------------------------------------------------------------------------------------------------------------------
    Adds one to the count, and is not cancelled half done.

    tally: The counter, as shared<value_type=counter>.
    ------------------------------------------------------------------------------------------------------------------------
    function add_one(injected tally):
        region added is atomic, uncancellable, within 1s:
            tally.count := tally.count + 1


    ------------------------------------------------------------------------------------------------------------------------
    Takes one from the count once it has one, however long that takes.

    tally: The counter, as shared<value_type=counter>.
    forever: The right to wait for it without bound, as forever.
    ------------------------------------------------------------------------------------------------------------------------
    function take_one_eventually(injected tally, injected forever):
        region taken is atomic, waits until ready = (tally.count > 0), forever because "a caller adds one first":
            tally.count := tally.count - 1


    ------------------------------------------------------------------------------------------------------------------------
    Adds two to the count, with two tasks that cancellation does not stop half done.

    tally: The counter, as shared<value_type=counter>.

    fails region_error.expired: The tasks took longer than a second.
    ------------------------------------------------------------------------------------------------------------------------
    function add_two(injected tally) fails region_error:
        region both is within 1s or fail, supervise(restart: restart_policy.each_failed, at_most: 1), uncancellable:
            task first = add_one()
            task second = add_one()


    ------------------------------------------------------------------------------------------------------------------------
    Adds two to the count with tasks that may outlive the call.

    tally: The counter, as shared<value_type=counter>.
    ------------------------------------------------------------------------------------------------------------------------
    function add_two_later(injected tally):
        region later is detach because "the count is read after the call", within 1s, supervise(restart: restart_policy.never, at_most: 0):
            task first = add_one()
            task second = add_one()


    ------------------------------------------------------------------------------------------------------------------------
    Adds one to the count with work that may outlive the call, and that is not cancelled half done.

    tally: The counter, as shared<value_type=counter>.
    ------------------------------------------------------------------------------------------------------------------------
    function add_one_later(injected tally):
        region later is detach because "the count is read after the call", uncancellable, within 1s:
            add_one()
