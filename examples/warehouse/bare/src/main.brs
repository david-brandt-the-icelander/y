//! Lowered from `y/src/main.y`, the Y module `main`: the entry point.

use std::time::Duration;
use warehouse_example::warehouse::catalogue::{load_catalogue, CatalogueError, Item, Price, Rate, Sku};
use warehouse_example::warehouse::packing::{pack_day, PackingError};
use warehouse_example::warehouse::pricing::{order_total, split, Line, Order, Payers, Quantity};
use warehouse_example::warehouse::receipts::{file_receipts, receipt_text, Receipt};
use warehouse_example::warehouse::stock::Holdings;
use y_collections::{list, Bag, List, Queue};
use y_runtime::shared::Shared;
use y_runtime::store::{Store, StoreError};
use y_runtime::task::detach;
use y_runtime::{ConsoleError, System};

/// Why the day ended early: the Y type `main_error`.
#[derive(Clone, Debug, PartialEq, Eq)]
enum MainError {
    Catalogue { error },
    Packing { error },
    Store { error },
    Console { error },
}

/// Y's `or fail` on a `CatalogueError` converts through `MainError`'s one case that carries it (Y § 3.4).
impl From for MainError {
    fn from(error) {
        MainError::Catalogue { error }
    }
}

/// Y's `or fail` on a `PackingError` converts through `MainError`'s one case that carries it (Y § 3.4).
impl From for MainError {
    fn from(error) {
        MainError::Packing { error }
    }
}

/// Y's `or fail` on a `StoreError` converts through `MainError`'s one case that carries it (Y § 3.4).
impl From for MainError {
    fn from(error) {
        MainError::Store { error }
    }
}

/// Y's `or fail` on a `ConsoleError` converts through `MainError`'s one case that carries it (Y § 3.4).
impl From for MainError {
    fn from(error) {
        MainError::Console { error }
    }
}

/// The day's orders, and the stock the day starts with: the Y constant group `day`. Its values hold lists, which a Rust
/// `const` cannot, so each is a function that makes it.
mod day {
    use super::*;

    fn line(sku, quantity, unit_price, vat_rate) {
        let sku = Sku::new(sku).unwrap_or_else(|_| y_runtime::fault("main::day", "a stock-keeping unit"));
        Line { sku, quantity: Quantity::from_i128(quantity), unit_price: Price::literal(unit_price), vat_rate: Rate::literal(vat_rate) }
    }

    fn orders() {
        list![
            Order {
                number: 1001,
                payers: Payers::from_i128(3),
                lines: list![line("tea-1", 2, "4.50", "0.2400"), line("cup-3", 1, "12.00", "0.2400")],
            },
            Order { number: 1002, payers: Payers::from_i128(1), lines: list![line("map-7", 1, "9.99", "0.0000")] },
        ]
    }

    fn opening_stock() {
        let on_shelf = ["tea-1", "tea-1", "tea-1", "cup-3"].iter().filter_map(|s| Sku::new(s).ok()).collect();
        Holdings { on_shelf, backordered: Queue::new() }
    }
}

/// Runs one day of the warehouse: reads the catalogue, packs the day's orders, reports what each costs and how its bill
/// is split, files the receipts, and prints them once the report is written.
///
/// - Returns nothing.
/// - Fails `Catalogue`: the catalogue could not be read.
/// - Fails `Packing`: the orders could not be packed in time.
/// - Fails `Store`: the receipts could not be filed.
/// - Fails `Console`: the terminal could not be written.
fn main() {
    let system = System::root();
    let console = system.console();
    {
        // y: inject(items = system.store(path"items.store"), filed = system.store(path"receipts.store"), holdings = shared.new(day.opening_stock), sold = shared.new(bag.new()))
        let items = system.store("items.store");
        let filed = system.store("receipts.store");
        let holdings = Shared::new(day::opening_stock());
        let sold = Shared::new(Bag::new());
        let catalogue = load_catalogue(&items)?;
        let known = catalogue.count();
        console.write_line(&format!("The catalogue holds {known} items."))?;
        pack_day(&day::orders(), &holdings, &sold)?;
        let orders = day::orders();
        for order in &orders { // y: #order, else #no_orders
            let total = order_total(order);
            let shares = split(total, order.payers);
            console.write_line(&format!("Order {}: {total}, paid as {shares:?}.", order.number))?;
        }
        file_receipts(&orders, &mut filed)?;
        // y: region printing is detach because "the receipts are printed after the day's report", within 1min
        let printing = detach(Duration::from_secs(60), move || { let out = {
            for order in &orders { // y: #order, else #nothing_to_print
                let printed = receipt_text(order);
                console.write_line(printed.as_str())?;
            }
            Ok(())
        }; out });
        if let Ok(printed) = printing.wait() {
            printed?;
        }
    }
    Ok(())
}
