-- An empty store is no catalogue.
test an_empty_store_is_no_catalogue
        covers load_catalogue > nothing_stored:
    inject:
            items = fake store(lines: [])
    when:
        refused = load_catalogue()
    then:
        empty = (refused == result.err(catalogue_error.empty))


-- One stored item is the whole catalogue.
test one_item_is_read
        covers load_catalogue > record > stored.readable:
    inject:
            items = fake store(lines: ["item v2 tea-1|Tea;4.50;0.2400"])
    when:
        found = load_catalogue() or fail
    then:
        one_item = (found.count() == 1)
        its_price = (found.get("tea-1").price == 4.50)


-- An item stored before items had a VAT rate is migrated, and charged the standard rate.
test an_item_of_version_1_is_migrated
        covers load_catalogue > record'many > stored.readable:
    inject:
            items = fake store(lines: ["item v1 cup-3|Cup;12.00", "item v2 map-7|Map;9.99;0.0000"])
    when:
        found = load_catalogue() or fail
    then:
        both = (found.count() == 2)
        standard_rate = (found.get("cup-3").vat_rate == tax.standard_rate)
        own_rate = (found.get("map-7").vat_rate == 0.0000)


-- An item that cannot be read fails the catalogue, and names the item.
test an_unreadable_item_fails_the_catalogue
        covers load_catalogue > record > stored.unreadable:
    inject:
            items = fake store(lines: ["item v2 tea-1|Tea;lots;0.2400"])
    when:
        refused = load_catalogue()
    then:
        named = (refused == result.err(catalogue_error.unreadable(sku: "tea-1")))


-- An unreadable item after a readable one still fails the catalogue.
test an_unreadable_item_after_a_good_one_fails_the_catalogue
        covers load_catalogue > record'many > stored.unreadable:
    inject:
            items = fake store(lines: ["item v2 cup-3|Cup;12.00;0.2400", "item v2 tea-1|Tea;lots;0.2400"])
    when:
        refused = load_catalogue()
    then:
        named = (refused == result.err(catalogue_error.unreadable(sku: "tea-1")))


-- A line's net price is its price times its quantity.
test a_line_costs_its_price_times_its_quantity
        covers net_of:
    given:
        two_teas = line(sku: "tea-1", quantity: 2, unit_price: 4.50, vat_rate: 0.2400)
    when:
        net = net_of(two_teas)
    then:
        nine = (net == 9.00)


-- A line's VAT is exact: it keeps every digit until the order's total is rounded.
test vat_is_exact_until_the_total
        covers vat_of:
    given:
        two_teas = line(sku: "tea-1", quantity: 2, unit_price: 4.55, vat_rate: 0.2400)
    when:
        vat = vat_of(two_teas)
    then:
        exact = (vat == 2.184000)


-- An order's total is its net prices and its VAT, rounded once, half away from zero.
test an_order_is_rounded_once
        covers order_total:
    given:
        mixed = order(number: 1, payers: 1, lines: [
            line(sku: "tea-1", quantity: 2, unit_price: 4.55, vat_rate: 0.2400),
            line(sku: "map-7", quantity: 1, unit_price: 9.99, vat_rate: 0.0000),
        ])
    when:
        total = order_total(mixed)
    then:
        to_the_cent = (total == 21.27)


-- A bill that divides evenly is shared evenly.
test an_even_bill_is_shared_evenly
        covers split > even:
    when:
        shares = split(30.00, 3)
    then:
        equal = (shares == [10.00, 10.00, 10.00])


-- The cents a bill does not divide into are paid by the first payer.
test the_first_payer_pays_the_odd_cents
        covers split > uneven:
    when:
        shares = split(10.00, 3)
    then:
        first_pays_more = (shares == [3.34, 3.33, 3.33])


-- A line the shelf can fill is taken from the shelf.
test a_line_is_taken_from_the_shelf
        covers reserve > take_from_shelf:
    given:
        two_teas = line(sku: "tea-1", quantity: 2, unit_price: 4.50, vat_rate: 0.2400)
    inject:
            holdings = shared.new(holdings(on_shelf: bag.of(["tea-1", "tea-1", "tea-1"]), backordered: queue.new()))
    when:
        reserved = reserve(two_teas)
    then:
        shelved = (reserved == reservation.from_shelf)
        one_left = (holdings.snapshot().on_shelf.count_of("tea-1") == 1)


-- A line the shelf cannot fill waits on backorder, and the shelf keeps what it had.
test a_line_the_shelf_cannot_fill_is_backordered
        covers reserve > put_on_backorder:
    given:
        five_teas = line(sku: "tea-1", quantity: 5, unit_price: 4.50, vat_rate: 0.2400)
    inject:
            holdings = shared.new(holdings(on_shelf: bag.of(["tea-1"]), backordered: queue.new()))
    when:
        reserved = reserve(five_teas)
    then:
        backordered = (reserved == reservation.backordered)
        waiting = (holdings.snapshot().backordered.count() == 1)
        shelf_kept = (holdings.snapshot().on_shelf.count_of("tea-1") == 1)


-- An order's lines are its own.
test an_orders_lines
        covers lines_of:
    given:
        one = order(number: 1, payers: 1, lines: [line(sku: "tea-1", quantity: 1, unit_price: 4.50, vat_rate: 0.2400)])
    when:
        lines = lines_of(one)
    then:
        same = (lines == one.lines)


-- No orders sell nothing.
test no_orders_sell_nothing
        covers pack_orders > no_lines:
    inject:
            holdings = shared.new(holdings(on_shelf: bag.of(["tea-1"]), backordered: queue.new()))
            sold = shared.new(bag.new())
    when:
        pack_orders([])
    then:
        nothing_sold = (sold.snapshot().count() == 0)


-- A line taken from the shelf is sold.
test a_shelved_line_is_sold
        covers pack_orders > wanted > shelved:
    given:
        one = order(number: 1, payers: 1, lines: [line(sku: "tea-1", quantity: 1, unit_price: 4.50, vat_rate: 0.2400)])
    inject:
            holdings = shared.new(holdings(on_shelf: bag.of(["tea-1"]), backordered: queue.new()))
            sold = shared.new(bag.new())
    when:
        pack_orders([one])
    then:
        one_sold = (sold.snapshot().count_of("tea-1") == 1)


-- A backordered line is not sold.
test a_backordered_line_is_not_sold
        covers pack_orders > wanted > backordered:
    given:
        one = order(number: 1, payers: 1, lines: [line(sku: "cup-3", quantity: 1, unit_price: 12.00, vat_rate: 0.2400)])
    inject:
            holdings = shared.new(holdings(on_shelf: bag.of(["tea-1"]), backordered: queue.new()))
            sold = shared.new(bag.new())
    when:
        pack_orders([one])
    then:
        nothing_sold = (sold.snapshot().count() == 0)


-- Every line of every order is packed, and the shelved ones are sold.
test every_line_is_packed
        covers pack_orders > wanted'many > shelved:
    given:
        first = order(number: 1, payers: 1, lines: [line(sku: "cup-3", quantity: 1, unit_price: 12.00, vat_rate: 0.2400)])
        second = order(number: 2, payers: 1, lines: [line(sku: "tea-1", quantity: 2, unit_price: 4.50, vat_rate: 0.2400)])
    inject:
            holdings = shared.new(holdings(on_shelf: bag.of(["tea-1", "tea-1"]), backordered: queue.new()))
            sold = shared.new(bag.new())
    when:
        pack_orders([first, second])
    then:
        teas_sold = (sold.snapshot().count_of("tea-1") == 2)
        cup_waits = (holdings.snapshot().backordered.count() == 1)


-- When the shelf runs out, the later lines wait on backorder.
test the_last_line_waits_when_the_shelf_runs_out
        covers pack_orders > wanted'many > backordered:
    given:
        first = order(number: 1, payers: 1, lines: [line(sku: "tea-1", quantity: 1, unit_price: 4.50, vat_rate: 0.2400)])
        second = order(number: 2, payers: 1, lines: [line(sku: "tea-1", quantity: 1, unit_price: 4.50, vat_rate: 0.2400)])
    inject:
            holdings = shared.new(holdings(on_shelf: bag.of(["tea-1"]), backordered: queue.new()))
            sold = shared.new(bag.new())
    when:
        pack_orders([first, second])
    then:
        one_sold = (sold.snapshot().count_of("tea-1") == 1)
        one_waits = (holdings.snapshot().backordered.count() == 1)


-- Two tasks pack the day's orders between them, and take no unit twice.
test two_tasks_pack_the_day
        covers pack_day:
    given:
        orders = [
            order(number: 1, payers: 1, lines: [line(sku: "tea-1", quantity: 2, unit_price: 4.50, vat_rate: 0.2400)]),
            order(number: 2, payers: 1, lines: [line(sku: "tea-1", quantity: 2, unit_price: 4.50, vat_rate: 0.2400)]),
            order(number: 3, payers: 1, lines: [line(sku: "tea-1", quantity: 2, unit_price: 4.50, vat_rate: 0.2400)]),
        ]
    inject:
            holdings = shared.new(holdings(on_shelf: bag.of(["tea-1", "tea-1", "tea-1", "tea-1"]), backordered: queue.new()))
            sold = shared.new(bag.new())
    when:
        pack_day(orders) or fail
    then:
        four_sold = (sold.snapshot().count_of("tea-1") == 4)
        one_waits = (holdings.snapshot().backordered.count() == 1)
        shelf_empty = (holdings.snapshot().on_shelf.count() == 0)


-- An order with no lines has a receipt of its heading alone.
test a_receipt_of_no_lines
        covers receipt_text > no_lines:
    given:
        empty = order(number: 7, payers: 1, lines: [])
    when:
        written = receipt_text(empty)
    then:
        heading_only = (written == "Order 7: 0.00")


-- A receipt lists its line under its heading.
test a_receipt_of_one_line
        covers receipt_text > wanted:
    given:
        one = order(number: 8, payers: 1, lines: [line(sku: "map-7", quantity: 1, unit_price: 9.99, vat_rate: 0.0000)])
    when:
        written = receipt_text(one)
    then:
        laid_out = (written == "Order 8: 9.99\n  1 x map-7")


-- A receipt lists every line, in order.
test a_receipt_of_two_lines
        covers receipt_text > wanted'many:
    given:
        two = order(number: 9, payers: 1, lines: [
            line(sku: "map-7", quantity: 1, unit_price: 9.99, vat_rate: 0.0000),
            line(sku: "tea-1", quantity: 2, unit_price: 4.50, vat_rate: 0.0000),
        ])
    when:
        written = receipt_text(two)
    then:
        laid_out = (written == "Order 9: 18.99\n  1 x map-7\n  2 x tea-1")


-- A day with no orders files no receipts.
test no_orders_file_no_receipts
        covers file_receipts > no_orders:
    inject:
            filed = fake store(lines: [])
    when:
        file_receipts([]) or fail
    then:
        none_filed = filed.lines().is_empty()


-- An order's receipt is filed under its number.
test a_receipt_is_filed
        covers file_receipts > order:
    given:
        one = order(number: 8, payers: 1, lines: [line(sku: "map-7", quantity: 1, unit_price: 9.99, vat_rate: 0.0000)])
    inject:
            filed = fake store(lines: [])
    when:
        file_receipts([one]) or fail
    then:
        filed_once = (filed.lines() == ["receipt v1 8|Order 8: 9.99\\n  1 x map-7"])


-- Every order's receipt is filed.
test every_receipt_is_filed
        covers file_receipts > order'many:
    given:
        first = order(number: 1, payers: 1, lines: [])
        second = order(number: 2, payers: 1, lines: [])
    inject:
            filed = fake store(lines: [])
    when:
        file_receipts([first, second]) or fail
    then:
        both_filed = (filed.lines().count() == 2)
