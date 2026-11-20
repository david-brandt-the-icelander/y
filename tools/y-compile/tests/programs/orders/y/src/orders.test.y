module orders


-- Two priorities are above the lowest.
test two_are_above_the_lowest
        covers priorities_above > each'many > higher:
    when:
        above = priorities_above(priority.low)
    then:
        two = (above == 2)


-- None is above the highest.
test none_is_above_the_highest
        covers priorities_above > each'many > not_higher:
    when:
        above = priorities_above(priority.high)
    then:
        none = (above == 0)


-- A new order waits for its payment, its fields kept.
test payment_is_requested
        covers request_payment:
    given:
        created = order.created(id: 1, total: 30)
    when:
        pending = request_payment(created: created, charge: 7)
    then:
        charge_kept = (pending.charge == 7)
        total_kept = (pending.total == 30)


-- A payment is recorded.
test payment_is_recorded
        covers record_payment:
    given:
        pending = request_payment(created: order.created(id: 1, total: 30), charge: 7)
    when:
        paid = record_payment(pending: pending, receipt: 99)
    then:
        receipt_kept = (paid.receipt == 99)


-- A paid order has its receipt.
test a_paid_order_has_its_receipt
        covers receipt_of > known.paid:
    given:
        paid = record_payment(pending: request_payment(created: order.created(id: 2, total: 5), charge: 3), receipt: 41)
    when:
        receipt = receipt_of(paid)
    then:
        forty_one = (receipt == 41)


-- An order not yet paid has no receipt.
test an_unpaid_order_has_none
        covers receipt_of > known.unpaid:
    when:
        receipt = receipt_of(order.created(id: 3, total: 5))
    then:
        none = (receipt == 0)


-- The count goes up by one, and when it was read is no part of it.
test the_count_goes_up_by_one
        covers counted:
    given:
        mutable kept = tally(count: 4, read_at: 1)
    when:
        counted(mutable kept)
    then:
        five = (kept.count == 5)
        read_time_is_no_part_of_it = (kept == tally(count: 5, read_at: 99))


-- An order is placed once, and its charge enqueued once, though its transaction conflicts twice.
test an_order_is_placed_once_though_it_conflicts
        covers place:
    inject:
        placed = shared.new(tally(count: 0, read_at: 0))
        outbox = fake enqueue<command_type=charge>
    faults:
        conflict at place#placing, times: 2
    when:
        place(id: 1, total: 30)
    then:
        counted_once = (placed.snapshot().count == 1)
        charged_once = (outbox.committed().count() == 1)


-- A charge the balance covers is taken.
test a_covered_charge_is_taken
        covers charge > covered:
    given:
        wanted = charge(order: 7, amount: 20)
    inject:
        bank = shared.new(bank(balance: 50, refunds: 0))
    when:
        receipt = charge(wanted) or fail
    then:
        its_receipt = (receipt == 1007)
        thirty_left = (bank.snapshot().balance == 30)


-- A charge the balance does not cover is declined.
test a_short_charge_is_declined
        covers charge > short:
    given:
        wanted = charge(order: 7, amount: 20)
    inject:
        bank = shared.new(bank(balance: 10, refunds: 0))
    when:
        receipt = charge(wanted)
    then:
        declined = (receipt == result.err(charge_error.declined))


-- A refund is counted.
test a_refund_is_counted
        covers refund:
    given:
        given_back = refund(order: 7)
    inject:
        bank = shared.new(bank(balance: 0, refunds: 0))
    when:
        refund(given_back)
    then:
        one = (bank.snapshot().refunds == 1)


-- An order the bank covers is paid for.
test a_covered_order_is_paid_for
        covers fulfil > charged.paid:
    inject:
        bank = shared.new(bank(balance: 50, refunds: 0))
        charges = submit.to(charge)
    when:
        receipt = fulfil(id: 7, total: 20) or fail
    then:
        paid = (receipt == 1007)


-- An order the bank does not cover is declined.
test a_short_order_is_declined
        covers fulfil > charged.declined:
    inject:
        bank = shared.new(bank(balance: 10, refunds: 0))
        charges = submit.to(charge)
    when:
        fulfilled = fulfil(id: 7, total: 20)
    then:
        declined = (fulfilled == result.err(fulfil_error.declined))


-- An order whose charge cannot be delivered is given back.
test an_undeliverable_charge_is_given_back
        covers fulfil > charged.abandoned:
    inject:
        bank = shared.new(bank(balance: 50, refunds: 0))
        charges = submit.to(charge)
    when:
        fulfilled = fulfil(id: 7, total: 0)
    then:
        unreachable = (fulfilled == result.err(fulfil_error.unreachable))
        given_back = (bank.snapshot().refunds == 1)
        compensated = (charges.compensated() == 1)


-- A crash after the order is reserved restarts the program: the workflow resumes, and does not reserve it again.
test a_workflow_resumes_after_a_crash
        covers reserve_and_charge > charged.paid:
    inject:
        reserved = shared.new(tally(count: 0, read_at: 0))
        bank = shared.new(bank(balance: 50, refunds: 0))
        charges = submit.to(charge)
    faults:
        crash after commit reserve_and_charge#reserving
    when:
        receipt = reserve_and_charge(id: 7, total: 20) or fail
    then:
        paid = (receipt == 1007)
        reserved_once = (reserved.snapshot().count == 1)
        charged_once = (bank.snapshot().balance == 30)


-- A charge the bank declines leaves the order unpaid.
test a_declined_charge_leaves_the_order_unpaid
        covers reserve_and_charge > charged.unpaid:
    inject:
        reserved = shared.new(tally(count: 0, read_at: 0))
        bank = shared.new(bank(balance: 10, refunds: 0))
        charges = submit.to(charge)
    when:
        fulfilled = reserve_and_charge(id: 7, total: 20)
    then:
        unpaid = (fulfilled == result.err(fulfil_error.unpaid))


-- A crash after the bank's handler commits restarts the program before the outcome is recorded: the command is
-- delivered again, as at-least-once delivery does, and a handler that is not idempotent in effect charges twice.
test a_command_is_delivered_again_after_a_crash
        covers fulfil > charged.paid:
    inject:
        bank = shared.new(bank(balance: 50, refunds: 0))
        charges = submit.to(charge)
    faults:
        crash after commit charge#taken
    when:
        receipt = fulfil(id: 7, total: 20) or fail
    then:
        paid = (receipt == 1007)
        charged_twice = (bank.snapshot().balance == 10)


-- A charge the bank covers is the order's receipt.
test a_charged_order_has_its_receipt
        covers charge_order:
    inject:
        bank = shared.new(bank(balance: 50, refunds: 0))
        charges = submit.to(charge)
    when:
        receipt = charge_order(id: 7, total: 20) or fail
    then:
        paid = (receipt == 1007)


-- A charge the bank declines fails the order with the bank's reason.
test a_declined_charge_fails_with_its_reason
        covers charge_order > receipt'fails:
    inject:
        bank = shared.new(bank(balance: 10, refunds: 0))
        charges = submit.to(charge)
    when:
        charged = charge_order(id: 7, total: 20)
    then:
        declined = (charged == result.err(payment_error.declined(reason: charge_error.declined)))


-- A worker delivers the charge a placed order enqueued, once its transaction commits: the bank takes it, and the
-- outbox is empty.
test an_enqueued_charge_is_delivered_by_a_worker
        covers place:
    inject:
        placed = shared.new(tally(count: 0, read_at: 0))
        bank = shared.new(bank(balance: 50, refunds: 0))
        outbox = enqueue.to(charge)
    when:
        place(id: 1, total: 30)
    then:
        delivered = (outbox.committed().count() == 0)
        charged = (bank.snapshot().balance == 20)

