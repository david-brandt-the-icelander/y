module orders


-- Orders: how urgent they are, their life from creation to payment, and the commands that charge for them.
public facet ordering:
    -- How urgent an order is, from least to most.
    enumeration priority:
        low
        normal
        high


    -- An order's life, from creation to payment.
    state order:
        id is uint32
        total is uint32

        case created
        case pending(charge is uint32)
        case paid(receipt is uint32)


    -- A count kept for a report; when it was last read is no part of its value.
    structure tally:
        count is uint32
        transient read_at is uint32


    -- What the bank holds, and how many charges it has given back.
    structure bank:
        balance is uint32
        refunds is uint32


    -- Why a charge failed.
    constants charge_error:
        declined = "declined"


    -- A sale as stored: the order, and what it came to.
    schema sale v1:
        key order is uint32
        amount is uint32


    -- Why a payment was not taken, with what the bank or the delivery said.
    type payment_error:
        case declined(reason is charge_error)
        case unreachable(why is abandonment)


    -- Why an order could not be paid for.
    constants fulfil_error:
        declined = "declined"
        unreachable = "unreachable"
        unpaid = "unpaid"


    -- Charges an order's total.
    command charge:
        order is uint32
        amount is uint32

        policy:
            deduplicate_by: order
            idempotent: true
            retry: backoff(first: 1ms, factor: 2, attempts: 3)
            deadline: 1s
            on_abandon: compensate refund(order: order)


    -- Gives an order's charge back.
    command refund:
        order is uint32

        policy:
            deduplicate_by: order
            idempotent: true
            retry: backoff(first: 1ms, factor: 1, attempts: 1)
            deadline: 1s
            on_abandon: park


    ------------------------------------------------------------------------------------------------------------------------
    How many priorities are above one.

    level: The priority, as priority.

    above: How many are above it, as uint32.
    ------------------------------------------------------------------------------------------------------------------------
    function priorities_above(level) returns above:
        mutable found = 0

        for each in priority:
            if higher = (each > level):
                found := found + 1
            else not_higher:
                continue
        return above = found


    ------------------------------------------------------------------------------------------------------------------------
    Asks for payment of a new order.

    created: The new order, as order.created.
    charge: The charge that was submitted, as uint32.

    pending: The order, waiting for payment, as order.pending.
    ------------------------------------------------------------------------------------------------------------------------
    transition request_payment(created, charge) returns pending:
        return pending = created.becomes(order.pending(charge: charge))


    ------------------------------------------------------------------------------------------------------------------------
    Records the payment of an order waiting for it.

    pending: The order, as order.pending.
    receipt: The bank's receipt, as uint32.

    paid: The order, paid, as order.paid.
    ------------------------------------------------------------------------------------------------------------------------
    transition record_payment(pending, receipt) returns paid:
        assert charged = (pending.charge > 0)
        return paid = pending.becomes(order.paid(receipt: receipt))


    ------------------------------------------------------------------------------------------------------------------------
    The receipt of an order whose state its type does not say: a conditional on its state recovers it.

    stored: The order, as order.

    receipt: Its receipt when it is paid, or 0, as uint32.
    ------------------------------------------------------------------------------------------------------------------------
    function receipt_of(stored) returns receipt:
        conditional known = stored:
            condition known.paid = (known == order.paid):
                return receipt = known.receipt
            else known.unpaid:
                return receipt = 0


    ------------------------------------------------------------------------------------------------------------------------
    Counts one more.

    tally: The count, as tally.
    ------------------------------------------------------------------------------------------------------------------------
    function counted(mutable tally)
            ensures one_more = (tally.count == old(tally.count) + 1):
        tally.count := tally.count + 1


    ------------------------------------------------------------------------------------------------------------------------
    Places an order: counts it, and enqueues its charge, in one transaction.

    id: The order, as uint32.
    total: What it costs, as uint32.

    placed: The orders placed, as shared<value_type=tally>.
    outbox: Where charges wait to be delivered, as enqueue<command_type=charge>.
    ------------------------------------------------------------------------------------------------------------------------
    function place(id, total, injected placed, injected outbox):
        region placing is atomic:
            placed.count := placed.count + 1
            enqueue charge(order: id, amount: total)

        log info "order {id} placed"
        metric orders_placed = 1


    ------------------------------------------------------------------------------------------------------------------------
    Charges a bank for an order. A charge of nothing is a bug of its sender, so its delivery faults.

    command: The charge, as charge.

    receipt: The bank's receipt, as uint32.

    bank: The bank, as shared<value_type=bank>.

    fails charge_error.declined: The balance does not cover the amount.
    ------------------------------------------------------------------------------------------------------------------------
    handler charge(command, injected bank) returns receipt fails charge_error:
        assert something = (command.amount > 0)

        if covered = (bank.snapshot().balance >= command.amount):
            region taken is atomic:
                bank.balance := bank.balance - command.amount
            return receipt = command.order + 1000
        else short:
            fail charge_error.declined


    ------------------------------------------------------------------------------------------------------------------------
    Gives a charge back.

    command: The refund, as refund.

    bank: The bank, as shared<value_type=bank>.
    ------------------------------------------------------------------------------------------------------------------------
    handler refund(command, injected bank):
        region given is atomic:
            bank.refunds := bank.refunds + 1


    ------------------------------------------------------------------------------------------------------------------------
    Takes an order's payment: submits its charge, and waits for the outcome.

    id: The order, as uint32.
    total: What it costs, as uint32.

    receipt: The bank's receipt, as uint32.

    charges: Where charges are submitted, as submit<command_type=charge>.

    fails fulfil_error.declined: The bank declined the charge.
    fails fulfil_error.unreachable: The charge could not be delivered, and was given back.
    ------------------------------------------------------------------------------------------------------------------------
    workflow fulfil(id, total, injected charges) returns receipt fails fulfil_error:
        conditional charged = submit charge(order: id, amount: total):
            condition charged.paid = (charged == result.ok):
                log info "order {id} paid"
                return receipt = charged.value
            condition charged.declined = (charged == result.err(undelivered.failed)):
                fail fulfil_error.declined
            condition charged.abandoned = (charged == result.err(undelivered.abandoned)):
                metric charges_abandoned = 1
                fail fulfil_error.unreachable


    ------------------------------------------------------------------------------------------------------------------------
    Reserves an order's stock, once, and then takes its payment: a crash between the two resumes after the reserving.

    id: The order, as uint32.
    total: What it costs, as uint32.

    receipt: The bank's receipt, as uint32.

    reserved: How many orders are reserved, as shared<value_type=tally>.
    charges: Where charges are submitted, as submit<command_type=charge>.

    fails fulfil_error.unpaid: The charge was declined, or could not be delivered.
    ------------------------------------------------------------------------------------------------------------------------
    workflow reserve_and_charge(id, total, injected reserved, injected charges) returns receipt fails fulfil_error:
        region reserving is atomic:
            reserved.count := reserved.count + 1

        conditional charged = submit charge(order: id, amount: total):
            condition charged.paid = (charged == result.ok):
                return receipt = charged.value
            else charged.unpaid:
                fail fulfil_error.unpaid


    ------------------------------------------------------------------------------------------------------------------------
    Takes an order's payment, and fails as the charge does: `or fail` converts a declined charge and an abandoned one.

    id: The order, as uint32.
    total: What it costs, as uint32.

    receipt: The bank's receipt, as uint32.

    charges: Where charges are submitted, as submit<command_type=charge>.

    fails payment_error.declined: The bank declined the charge.
    fails payment_error.unreachable: The charge could not be delivered, and was given back.
    ------------------------------------------------------------------------------------------------------------------------
    workflow charge_order(id, total, injected charges) returns receipt fails payment_error:
        return receipt = submit charge(order: id, amount: total) or fail


    ------------------------------------------------------------------------------------------------------------------------
    Records a sale: counts it in shared state and files it in the store, in one transaction, so that the two agree.

    id: The order, as uint32.
    total: What it came to, as uint32.

    placed: The orders placed, as shared<value_type=tally>.
    sales: The sales filed, as store<schema_type=sale>.
    ------------------------------------------------------------------------------------------------------------------------
    function record_sale(id, total, injected placed, injected sales):
        region recording is atomic:
            placed.count := placed.count + 1
            sales.put(sale(order: id, amount: total))

