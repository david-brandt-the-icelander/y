module main

from warehouse import catalogue.catalogue_error, catalogue.load_catalogue, packing.pack_day, packing.packing_error
from warehouse import pricing.line, pricing.order, pricing.order_total, pricing.split
from warehouse import receipts.file_receipts, receipts.receipt_text, stock.holdings
from standard.console import console_error
from standard.store import store_error


-- Why the day ended early.
type main_error:
    case catalogue(error is catalogue_error)
    case packing(error is packing_error)
    case store(error is store_error)
    case console(error is console_error)


-- The day's orders, and the stock the day starts with.
constants day:
    orders is list<element_type=order> = [
        order(number: 1001, payers: 3, lines: [
            line(sku: "tea-1", quantity: 2, unit_price: 4.50, vat_rate: 0.2400),
            line(sku: "cup-3", quantity: 1, unit_price: 12.00, vat_rate: 0.2400),
        ]),
        order(number: 1002, payers: 1, lines: [line(sku: "map-7", quantity: 1, unit_price: 9.99, vat_rate: 0.0000)]),
    ]
    opening_stock is holdings = holdings(on_shelf: bag.of(["tea-1", "tea-1", "tea-1", "cup-3"]), backordered: queue.new())


----------------------------------------------------------------------------------------------------
Runs one day of the warehouse: reads the catalogue, packs the day's orders, reports what each costs and how its bill is
split, files the receipts, and prints them once the report is written.

system: The root capability, from which the console and the stores come, as system.

fails main_error.catalogue: The catalogue could not be read.
fails main_error.packing: The orders could not be packed in time.
fails main_error.store: The receipts could not be filed.
fails main_error.console: The terminal could not be written.
----------------------------------------------------------------------------------------------------
function main(injected system) fails main_error:
    console = system.console()

    inject(
            items = system.store(path"items.store"),
            filed = system.store(path"receipts.store"),
            holdings = shared.new(day.opening_stock),
            sold = shared.new(bag.new())):
        catalogue = load_catalogue() or fail
        known = catalogue.count()
        console.write_line("The catalogue holds {known} items.") or fail
        pack_day(day.orders) or fail

        for order in day.orders:
            total = order_total(order)
            shares = split(total, order.payers)
            console.write_line("Order {order.number}: {total}, paid as {shares}.") or fail
        file_receipts(day.orders) or fail

        region printing is detach because "the receipts are printed after the day's report", within 1min:
            for order in day.orders:
                printed = receipt_text(order)
                console.write_line(printed) or fail
