//! Warehouse's atomic regions under the optimistic engine (chosen with `System::use_engine`), which may run a region
//! again (Y § 3.8).
//! This is not a lowered Y test: it makes another task's commit land between a region's read and its commit, with the
//! runtime's `after_next_read`, so the re-run happens every time and not only when the scheduler gives one.
use warehouse_example::warehouse::catalogue::{Price, Rate, Sku};
use warehouse_example::warehouse::pricing::{Line, Quantity};
use warehouse_example::warehouse::stock::{reserve, Holdings, Reservation};
use y_collections::Queue;
use y_runtime::shared::{after_next_read, reruns, Engine, Shared};
use y_runtime::System;

fn teas(quantity: i128) -> Line {
    let sku = Sku::new("tea-1").expect("a stock-keeping unit");
    Line { sku, quantity: Quantity::from_i128(quantity), unit_price: Price::literal("4.50"), vat_rate: Rate::literal("0.2400") }
}

#[test]
fn a_reservation_runs_again_when_another_commits_first() {
    System::root().use_engine(Engine::Optimistic);
    let tea = Sku::new("tea-1").expect("a stock-keeping unit");
    let holdings = Shared::new(Holdings { on_shelf: [tea.clone(), tea.clone(), tea.clone()].into_iter().collect(), backordered: Queue::new() });
    let other = holdings.clone();
    let before = reruns();
    // another task takes one tea after this reservation has read the shelf, and commits first
    after_next_read(move || {
        assert_eq!(reserve(&teas(1), &other), Reservation::FromShelf);
    });
    // this one wanted two of the three it saw: it runs again, sees two, and takes them
    assert_eq!(reserve(&teas(2), &holdings), Reservation::FromShelf);
    assert_eq!(reruns(), before + 1, "the reservation ran again");
    assert_eq!(holdings.snapshot().on_shelf.count_of(&tea), 0, "no tea taken twice");
}
