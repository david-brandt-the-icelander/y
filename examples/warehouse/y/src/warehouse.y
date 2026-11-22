module warehouse

from standard.console import console, console_error
from standard.store import store, store_error


-- The goods: what the warehouse sells, as its catalogue stores them.
public facet catalogue:
    -- An item's stock-keeping unit, such as `tea-1`: ASCII only, so a backend may keep it as bytes.
    type sku is string<character_set=ascii>


    -- An item as stored, version 1: before items carried a VAT rate.
    schema item v1:
        key sku is sku
        name is string
        price is decimal<precision=12, scale=2>


    -- An item as stored, version 2: with the VAT rate charged on it.
    schema item v2:
        key sku is sku
        name is string
        price is decimal<precision=12, scale=2>
        vat_rate is decimal<precision=5, scale=4>


    -- Version 2 adds the VAT rate: an item stored before it is charged the standard rate.
    migrate item v1 to v2:
        vat_rate := tax.standard_rate


    -- The VAT rates the warehouse charges.
    constants tax:
        standard_rate is decimal<precision=5, scale=4> = 0.2400


    -- Why the catalogue could not be read.
    type catalogue_error:
        case empty
        case unreadable(sku is sku)


    ------------------------------------------------------------------------------------------------------------------------
    Reads the catalogue: every stored item, by its stock-keeping unit, each migrated to the current version as it is read.

    items: The stored items, as store<schema_type=item>.

    found: The items, as sorted_map<key_type=sku, value_type=item>.

    fails catalogue_error.empty: Nothing is stored.
    fails catalogue_error.unreadable: An item could not be read, or migrated to the current version.
    ------------------------------------------------------------------------------------------------------------------------
    function load_catalogue(injected items) returns found fails catalogue_error:
        mutable read = sorted_map.new()

        for record in items.read_all():
            conditional stored = record:
                condition stored.readable = (stored == result.ok):
                    read.set(stored.value.sku, stored.value)
                else stored.unreadable:
                    fail catalogue_error.unreadable(sku: stored.failure.key)
        else nothing_stored:
            fail catalogue_error.empty
        return found = read


-- Orders, what they cost, and who pays.
public facet pricing:
    -- One line of an order: an item, how many, and its price and VAT rate when the order was taken.
    structure line:
        sku is sku
        quantity is integer<minimum=1, maximum=1000>
        unit_price is decimal<precision=12, scale=2>
        vat_rate is decimal<precision=5, scale=4>


    -- An order, and how many people share its bill.
    structure order:
        number is uint32
        lines is list<element_type=line>
        payers is integer<minimum=1, maximum=10>


    -- line: The line to judge, as line.
    rule taxed(line) = (line.vat_rate > 0.0000)


    ------------------------------------------------------------------------------------------------------------------------
    What a line costs before VAT.

    line: The line, as line.

    net: Its price times its quantity, exact, as decimal<scale=2>.
    ------------------------------------------------------------------------------------------------------------------------
    function net_of(line) returns net:
        return net = line.unit_price * line.quantity


    ------------------------------------------------------------------------------------------------------------------------
    The VAT on a line, exact: it is rounded only with the order's total.

    line: The line, as line.

    vat: Its net price times its VAT rate, as decimal<scale=6>.
    ------------------------------------------------------------------------------------------------------------------------
    function vat_of(line) returns vat:
        return vat = net_of(line) * line.vat_rate


    ------------------------------------------------------------------------------------------------------------------------
    What an order costs: its lines' net prices, with the VAT on the taxed ones, rounded once, to the cent, half away from
    zero.

    order: The order, as order.

    total: The order's total, as decimal<precision=12, scale=2>.
    ------------------------------------------------------------------------------------------------------------------------
    function order_total(order) returns total:
        net = order.lines.map(net_of).sum()
        vat = order.lines.filter(taxed).map(vat_of).sum()
        return total = (net + vat).to_decimal_rounded(scale: 2, rounding: rounding.half_away_from_zero)


    ------------------------------------------------------------------------------------------------------------------------
    Splits a bill between its payers: each pays an equal share, rounded down to the cent, and the first payer also pays
    the cents left over.

    total: The bill, as decimal<precision=12, scale=2>.
    payers: How many pay it, as integer<minimum=1, maximum=10>.

    shares: What each pays, in order, which add up to the bill, as list<element_type=decimal<precision=12, scale=2>>.
    ------------------------------------------------------------------------------------------------------------------------
    function split(total, payers) returns shares:
        each = (total.to_rational() / payers.to_rational()).to_decimal_rounded(scale: 2, rounding: rounding.toward_negative)
        left = total - (each * payers)
        assume settled = (left >= 0.00) because "each share is rounded down, so the shares never add up to more than the bill"
        mutable owed = list.repeated(each, times: payers)

        if even = (left == 0.00):
            return shares = owed
        else uneven:
            owed.set_number(1, each + left)
            return shares = owed


-- What the warehouse holds, shared by every task that packs.
public facet stock:
    -- The units on the shelves, and the lines waiting for more.
    structure holdings:
        on_shelf is bag<element_type=sku>
        backordered is queue<element_type=line>


    -- Where a line's units came from.
    constants reservation:
        from_shelf = "from the shelf"
        backordered = "backordered"


    ------------------------------------------------------------------------------------------------------------------------
    Reserves a line's units: from the shelf when it holds enough of them, and otherwise on backorder, in the same
    transaction, so no two tasks take the same units.

    wanted: The line to reserve, as line.

    reserved: Where the units came from, as reservation.

    holdings: The stock, as shared<value_type=holdings>.
    ------------------------------------------------------------------------------------------------------------------------
    function reserve(wanted, injected holdings) returns reserved:
        region take_from_shelf is atomic, waits until enough = (holdings.on_shelf.count_of(wanted.sku) >= wanted.quantity):
            holdings.on_shelf.remove(wanted.sku, times: wanted.quantity)
            return reserved = reservation.from_shelf
        orelse region put_on_backorder is atomic:
            holdings.backordered.add(wanted)
            return reserved = reservation.backordered


    ------------------------------------------------------------------------------------------------------------------------
    Counts the units on the shelf, for a report. It reads one consistent snapshot of the stock and writes nothing, so a
    reservation that commits while it runs does not make it run again.

    units: How many units are on the shelf, as count<unit_type=sku>.

    holdings: The stock, as shared<value_type=holdings>.
    ------------------------------------------------------------------------------------------------------------------------
    function units_on_shelf(injected holdings) returns units:
        region counted is atomic, isolation snapshot:
            return units = holdings.on_shelf.count()


-- Packing the day's orders, two tasks at a time.
public facet packing:
    -- Why the day's packing stopped.
    type packing_error:
        case expired(deadline is expired)


    ------------------------------------------------------------------------------------------------------------------------
    An order's lines.

    order: The order, as order.

    lines: Its lines, in order, as list<element_type=line>.
    ------------------------------------------------------------------------------------------------------------------------
    function lines_of(order) returns lines:
        return lines = order.lines


    ------------------------------------------------------------------------------------------------------------------------
    Packs orders: reserves each line, and counts the units it took from the shelf as sold.

    orders: The orders, as list<element_type=order>.

    holdings: The stock, as shared<value_type=holdings>.
    sold: The units sold so far, as shared<value_type=bag<element_type=sku>>.
    ------------------------------------------------------------------------------------------------------------------------
    function pack_orders(orders, injected holdings, injected sold):
        lines = orders.map(lines_of).flatten()

        for wanted in lines:
            reserved = reserve(wanted)

            if shelved = (reserved == reservation.from_shelf):
                region count_sale is atomic:
                    sold.add(wanted.sku, times: wanted.quantity)
            else backordered:
                continue
        else no_lines


    ------------------------------------------------------------------------------------------------------------------------
    Packs the day's orders with two tasks, each taking half of them, within ten seconds. A task that fails is started
    again, twice at most.

    orders: The day's orders, as list<element_type=order>.

    holdings: The stock, as shared<value_type=holdings>.
    sold: The units sold so far, as shared<value_type=bag<element_type=sku>>.

    fails packing_error.expired: The packing took longer than ten seconds.
    ------------------------------------------------------------------------------------------------------------------------
    function pack_day(orders, injected holdings, injected sold) fails packing_error:
        half = orders.count().divide_floor(2)

        region packing is within 10s or fail, supervise(restart: restart_policy.each_failed, at_most: 2, per: 1min):
            task first_half = pack_orders(orders.take(half))
            task second_half = pack_orders(orders.skip(half))


-- Receipts: written for each order, and filed in the store.
public facet receipts:
    -- A receipt as stored.
    schema receipt v1:
        key order is uint32
        text is string


    -- One line of a receipt being laid out, and the line it belongs under.
    structure entry:
        text is string
        under is optional<value_type=index<unit_type=entry>>


    ------------------------------------------------------------------------------------------------------------------------
    Lays out an order's receipt: its number and total, then each line under them. The layout is built in an arena, which
    is released at once when the receipt is written.

    order: The order, as order.

    written: The receipt, one line each, as string.
    ------------------------------------------------------------------------------------------------------------------------
    function receipt_text(order) returns written:
        arena receipt_scratch:
            total = order_total(order)
            heading = receipt_scratch.allocate(entry(text: "Order {order.number}: {total}", under: optional.none))

            for wanted in order.lines:
                receipt_scratch.allocate(entry(text: "{wanted.quantity} x {wanted.sku}", under: optional.some(heading)))
            else no_lines
            return written = receipt_scratch.rendered()


    ------------------------------------------------------------------------------------------------------------------------
    Files the day's receipts in the store, as one batch: the batch is a resource, written to the store when it is
    released, and filing it is not cancelled half done.

    orders: The day's orders, as list<element_type=order>.

    filed: The receipts, as store<schema_type=receipt>.

    fails store_error: The batch could not be written.
    ------------------------------------------------------------------------------------------------------------------------
    function file_receipts(orders, injected filed) fails store_error:
        batch = filed.batch()

        region filing is uncancellable, within 50ms:
            for order in orders:
                batch.put(receipt(order: order.number, text: receipt_text(order)))
            else no_orders
