//! Lowered from `y/src/warehouse.test.y`. Each test says, in a comment, the path its Y source declares it covers.
//! An `or fail` in a test's `given` or `when` lowers to `expect`: a failure there fails the test. A decimal literal is
//! `literal("…")` of the type its place has, as the Y compiler would check it.

use super::{catalogue::*, packing::*, pricing::*, receipts::*, stock::*};
use y_collections::{list, Bag, List, Queue};
use y_runtime::shared::Shared;
use y_runtime::store::Store;
use y_string::YString;

fn sku(text: &str) -> Sku {
    Sku::new(text).expect("a stock-keeping unit")
}

fn line(code: &str, quantity: i128, unit_price: &str, vat_rate: &str) -> Line {
    Line { sku: sku(code), quantity: Quantity::from_i128(quantity), unit_price: Price::literal(unit_price), vat_rate: Rate::literal(vat_rate) }
}

fn order(number: u32, payers: i128, lines: List<Line>) -> Order {
    Order { number, payers: Payers::from_i128(payers), lines }
}

fn shelf(units: &[&str]) -> Shared<Holdings> {
    Shared::new(Holdings { on_shelf: units.iter().map(|u| sku(u)).collect(), backordered: Queue::new() })
}

#[test]
fn an_empty_store_is_no_catalogue() {
    // y: covers load_catalogue > nothing_stored
    let items: Store<Item> = Store::fake(&[]);
    let refused = load_catalogue(&items);
    assert!(refused == Err(CatalogueError::Empty), "empty");
}

#[test]
fn one_item_is_read() {
    // y: covers load_catalogue > record > stored.readable
    let items: Store<Item> = Store::fake(&["item v2 tea-1|Tea;4.50;0.2400"]);
    let found = load_catalogue(&items).expect("when");
    assert!(found.count() == 1, "one_item");
    assert!(found.get(&sku("tea-1")).map(|i| i.price) == Some(Price::literal("4.50")), "its_price");
}

#[test]
fn an_item_of_version_1_is_migrated() {
    // y: covers load_catalogue > record'many > stored.readable
    let items: Store<Item> = Store::fake(&["item v1 cup-3|Cup;12.00", "item v2 map-7|Map;9.99;0.0000"]);
    let found = load_catalogue(&items).expect("when");
    assert!(found.count() == 2, "both");
    assert!(found.get(&sku("cup-3")).map(|i| i.vat_rate) == Some(tax::STANDARD_RATE), "standard_rate");
    assert!(found.get(&sku("map-7")).map(|i| i.vat_rate) == Some(Rate::literal("0.0000")), "own_rate");
}

#[test]
fn an_unreadable_item_fails_the_catalogue() {
    // y: covers load_catalogue > record > stored.unreadable
    let items: Store<Item> = Store::fake(&["item v2 tea-1|Tea;lots;0.2400"]);
    let refused = load_catalogue(&items);
    assert!(refused == Err(CatalogueError::Unreadable { sku: sku("tea-1") }), "named");
}

#[test]
fn an_unreadable_item_after_a_good_one_fails_the_catalogue() {
    // y: covers load_catalogue > record'many > stored.unreadable
    let items: Store<Item> = Store::fake(&["item v2 cup-3|Cup;12.00;0.2400", "item v2 tea-1|Tea;lots;0.2400"]);
    let refused = load_catalogue(&items);
    assert!(refused == Err(CatalogueError::Unreadable { sku: sku("tea-1") }), "named");
}

#[test]
fn a_line_costs_its_price_times_its_quantity() {
    // y: covers net_of
    let two_teas = line("tea-1", 2, "4.50", "0.2400");
    let net = net_of(&two_teas);
    assert!(net == Net::literal("9.00"), "nine");
}

#[test]
fn vat_is_exact_until_the_total() {
    // y: covers vat_of
    let two_teas = line("tea-1", 2, "4.55", "0.2400");
    let vat = vat_of(&two_teas);
    assert!(vat == Vat::literal("2.184000"), "exact");
}

#[test]
fn an_order_is_rounded_once() {
    // y: covers order_total
    let mixed = order(1, 1, list![line("tea-1", 2, "4.55", "0.2400"), line("map-7", 1, "9.99", "0.0000")]);
    let total = order_total(&mixed);
    assert!(total == Price::literal("21.27"), "to_the_cent");
}

#[test]
fn an_even_bill_is_shared_evenly() {
    // y: covers split > even
    let shares = split(Price::literal("30.00"), Payers::from_i128(3));
    assert!(shares == list![Price::literal("10.00"), Price::literal("10.00"), Price::literal("10.00")], "equal");
}

#[test]
fn the_first_payer_pays_the_odd_cents() {
    // y: covers split > uneven
    let shares = split(Price::literal("10.00"), Payers::from_i128(3));
    assert!(shares == list![Price::literal("3.34"), Price::literal("3.33"), Price::literal("3.33")], "first_pays_more");
}

#[test]
fn a_line_is_taken_from_the_shelf() {
    // y: covers reserve > take_from_shelf
    let two_teas = line("tea-1", 2, "4.50", "0.2400");
    let holdings = shelf(&["tea-1", "tea-1", "tea-1"]);
    let reserved = reserve(&two_teas, &holdings);
    assert!(reserved == Reservation::FromShelf, "shelved");
    assert!(holdings.snapshot().on_shelf.count_of(&sku("tea-1")) == 1, "one_left");
}

#[test]
fn a_line_the_shelf_cannot_fill_is_backordered() {
    // y: covers reserve > put_on_backorder
    let five_teas = line("tea-1", 5, "4.50", "0.2400");
    let holdings = shelf(&["tea-1"]);
    let reserved = reserve(&five_teas, &holdings);
    assert!(reserved == Reservation::Backordered, "backordered");
    assert!(holdings.snapshot().backordered.count() == 1, "waiting");
    assert!(holdings.snapshot().on_shelf.count_of(&sku("tea-1")) == 1, "shelf_kept");
}

#[test]
fn the_units_on_the_shelf_are_counted() {
    // y: covers units_on_shelf
    let holdings = shelf(&["tea-1", "tea-1", "cup-3"]);
    let units = units_on_shelf(&holdings);
    assert!(units == 3, "three");
}

#[test]
fn an_orders_lines() {
    // y: covers lines_of
    let one = order(1, 1, list![line("tea-1", 1, "4.50", "0.2400")]);
    let lines = lines_of(&one);
    assert!(lines == one.lines, "same");
}

#[test]
fn no_orders_sell_nothing() {
    // y: covers pack_orders > no_lines
    let holdings = shelf(&["tea-1"]);
    let sold = Shared::new(Bag::new());
    pack_orders(&List::new(), &holdings, &sold);
    assert!(sold.snapshot().count() == 0, "nothing_sold");
}

#[test]
fn a_shelved_line_is_sold() {
    // y: covers pack_orders > wanted > shelved
    let one = order(1, 1, list![line("tea-1", 1, "4.50", "0.2400")]);
    let holdings = shelf(&["tea-1"]);
    let sold = Shared::new(Bag::new());
    pack_orders(&list![one], &holdings, &sold);
    assert!(sold.snapshot().count_of(&sku("tea-1")) == 1, "one_sold");
}

#[test]
fn a_backordered_line_is_not_sold() {
    // y: covers pack_orders > wanted > backordered
    let one = order(1, 1, list![line("cup-3", 1, "12.00", "0.2400")]);
    let holdings = shelf(&["tea-1"]);
    let sold = Shared::new(Bag::new());
    pack_orders(&list![one], &holdings, &sold);
    assert!(sold.snapshot().count() == 0, "nothing_sold");
}

#[test]
fn every_line_is_packed() {
    // y: covers pack_orders > wanted'many > shelved
    let first = order(1, 1, list![line("cup-3", 1, "12.00", "0.2400")]);
    let second = order(2, 1, list![line("tea-1", 2, "4.50", "0.2400")]);
    let holdings = shelf(&["tea-1", "tea-1"]);
    let sold = Shared::new(Bag::new());
    pack_orders(&list![first, second], &holdings, &sold);
    assert!(sold.snapshot().count_of(&sku("tea-1")) == 2, "teas_sold");
    assert!(holdings.snapshot().backordered.count() == 1, "cup_waits");
}

#[test]
fn the_last_line_waits_when_the_shelf_runs_out() {
    // y: covers pack_orders > wanted'many > backordered
    let first = order(1, 1, list![line("tea-1", 1, "4.50", "0.2400")]);
    let second = order(2, 1, list![line("tea-1", 1, "4.50", "0.2400")]);
    let holdings = shelf(&["tea-1"]);
    let sold = Shared::new(Bag::new());
    pack_orders(&list![first, second], &holdings, &sold);
    assert!(sold.snapshot().count_of(&sku("tea-1")) == 1, "one_sold");
    assert!(holdings.snapshot().backordered.count() == 1, "one_waits");
}

#[test]
fn two_tasks_pack_the_day() {
    // y: covers pack_day
    let orders = list![
        order(1, 1, list![line("tea-1", 2, "4.50", "0.2400")]),
        order(2, 1, list![line("tea-1", 2, "4.50", "0.2400")]),
        order(3, 1, list![line("tea-1", 2, "4.50", "0.2400")]),
    ];
    let holdings = shelf(&["tea-1", "tea-1", "tea-1", "tea-1"]);
    let sold = Shared::new(Bag::new());
    pack_day(&orders, &holdings, &sold).expect("when");
    assert!(sold.snapshot().count_of(&sku("tea-1")) == 4, "four_sold");
    assert!(holdings.snapshot().backordered.count() == 1, "one_waits");
    assert!(holdings.snapshot().on_shelf.count() == 0, "shelf_empty");
}

#[test]
fn a_receipt_of_no_lines() {
    // y: covers receipt_text > no_lines
    let empty = order(7, 1, List::new());
    let written = receipt_text(&empty);
    assert!(written == YString::new("Order 7: 0.00"), "heading_only");
}

#[test]
fn a_receipt_of_one_line() {
    // y: covers receipt_text > wanted
    let one = order(8, 1, list![line("map-7", 1, "9.99", "0.0000")]);
    let written = receipt_text(&one);
    assert!(written == YString::new("Order 8: 9.99\n  1 x map-7"), "laid_out");
}

#[test]
fn a_receipt_of_two_lines() {
    // y: covers receipt_text > wanted'many
    let two = order(9, 1, list![line("map-7", 1, "9.99", "0.0000"), line("tea-1", 2, "4.50", "0.0000")]);
    let written = receipt_text(&two);
    assert!(written == YString::new("Order 9: 18.99\n  1 x map-7\n  2 x tea-1"), "laid_out");
}

#[test]
fn no_orders_file_no_receipts() {
    // y: covers file_receipts > no_orders
    let mut filed: Store<Receipt> = Store::fake(&[]);
    file_receipts(&List::new(), &mut filed).expect("when");
    assert!(filed.lines().is_empty(), "none_filed");
}

#[test]
fn a_receipt_is_filed() {
    // y: covers file_receipts > order
    let one = order(8, 1, list![line("map-7", 1, "9.99", "0.0000")]);
    let mut filed: Store<Receipt> = Store::fake(&[]);
    file_receipts(&list![one], &mut filed).expect("when");
    assert!(filed.lines() == ["receipt v1 8|Order 8: 9.99\\n  1 x map-7"], "filed_once");
}

#[test]
fn every_receipt_is_filed() {
    // y: covers file_receipts > order'many
    let first = order(1, 1, List::new());
    let second = order(2, 1, List::new());
    let mut filed: Store<Receipt> = Store::fake(&[]);
    file_receipts(&list![first, second], &mut filed).expect("when");
    assert!(filed.lines().len() == 2, "both_filed");
}
