//! Lowered from `y/src/warehouse.y`, the Y module `warehouse`.
//! Each Y facet is a Rust module of the same name, so a member's path keeps the facet: `warehouse::stock::reserve`.
//! Each named decision of the Y source is marked `// y: #name` where its condition is tested, each named `condition` of a
//! `conditional` on its `match` arm, and each alternative of an `orelse` on its closure. A region's execution
//! constraints are marked `// y: region …` where the region starts.

#[cfg(test)]
mod tests;

/// The Y facet `catalogue`: what the warehouse sells, as its catalogue stores them.
pub mod catalogue {
    use y_collections::SortedMap;
    use y_runtime::decimal::Decimal;
    use y_runtime::store::{Schema, Store};
    use y_runtime::fault;
    use y_string::{AsciiString, YString};

    /// An item's stock-keeping unit, such as `tea-1`: ASCII only, so it is kept as bytes.
    pub type Sku = AsciiString;

    /// A price: Y's `decimal<precision=12, scale=2>`.
    pub type Price = Decimal<i64, 12, 2>;

    /// A VAT rate: Y's `decimal<precision=5, scale=4>`.
    pub type Rate = Decimal<i32, 5, 4>;

    /// The VAT rates the warehouse charges: the Y constant group `tax`.
    pub mod tax {
        /// The standard rate, 24%.
        pub const STANDARD_RATE: super::Rate = super::Rate::of_units(2400);
    }

    /// An item as stored, at the schema's current version, 2.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Item {
        pub sku: Sku,
        pub name: YString,
        pub price: Price,
        pub vat_rate: Rate,
    }

    /// The codec of `schema item`, versions 1 and 2, with `migrate item v1 to v2`: what the Y compiler generates from
    /// the declarations. A record is `name;price` at version 1 and `name;price;vat_rate` at version 2.
    impl Schema for Item {
        const NAME: &'static str = "item";
        const VERSION: u32 = 2;

        fn key(&self) -> String {
            self.sku.to_string()
        }

        fn encode(&self) -> String {
            format!("{};{};{}", self.name, self.price, self.vat_rate)
        }

        fn decode(key: &str, version: u32, text: &str) -> Result<Self, String> {
            let sku = AsciiString::new(key).map_err(|_| format!("`{key}` is not a stock-keeping unit"))?;
            let fields: Vec<&str> = text.split(';').collect();
            let price = |t: &str| Price::parse(t).ok_or_else(|| format!("`{t}` is not a price"));
            match (version, fields.as_slice()) {
                // migrate item v1 to v2: vat_rate := tax.standard_rate
                (1, [name, p]) => Ok(Item { sku, name: YString::new(name), price: price(p)?, vat_rate: tax::STANDARD_RATE }),
                (2, [name, p, rate]) => {
                    let vat_rate = Rate::parse(rate).ok_or_else(|| format!("`{rate}` is not a VAT rate"))?;
                    Ok(Item { sku, name: YString::new(name), price: price(p)?, vat_rate })
                }
                _ => Err(format!("not an item of version {version}")),
            }
        }
    }

    /// Why the catalogue could not be read.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum CatalogueError {
        Empty,
        Unreadable { sku: Sku },
    }

    /// Reads the catalogue: every stored item, by its stock-keeping unit, each migrated to the current version as it is
    /// read.
    ///
    /// - `items`: the Y `injected` parameter.
    /// - Returns the items.
    /// - Fails `Empty`: nothing is stored.
    /// - Fails `Unreadable`: an item could not be read, or migrated to the current version.
    pub fn load_catalogue(items: &Store<Item>) -> Result<SortedMap<Sku, Item>, CatalogueError> {
        let mut read = SortedMap::new();
        let mut ran = false;
        for record in items.read_all() { // y: #record
            ran = true;
            match record {
                Ok(stored) => { // y: #stored.readable
                    read.set(stored.sku.clone(), stored);
                }
                Err(failure) => { // y: #stored.unreadable
                    let sku = AsciiString::new(&failure.key).unwrap_or_else(|_| fault("warehouse::load_catalogue", "a key"));
                    return Err(CatalogueError::Unreadable { sku });
                }
            }
        }
        if !ran { // y: #nothing_stored
            return Err(CatalogueError::Empty);
        }
        Ok(read)
    }
}

/// The Y facet `pricing`: orders, what they cost, and who pays.
pub mod pricing {
    use super::catalogue::{Price, Rate, Sku};
    use y_collections::List;
    use y_runtime::decimal::{Big, Decimal, Integer, Rational, Rounding, UNBOUNDED};
    use y_runtime::list;

    /// A line's quantity: Y's `integer<minimum=1, maximum=1000>`.
    pub type Quantity = Integer<i16, 1, 1000>;

    /// How many share a bill: Y's `integer<minimum=1, maximum=10>`.
    pub type Payers = Integer<i8, 1, 10>;

    /// An exact sum of prices: Y's `decimal<scale=2>`, which the compiler derives; this backend keeps it in an `i128`.
    pub type Net = Decimal<i128, 38, 2>;

    /// An exact VAT: Y's `decimal<scale=6>`.
    pub type Vat = Decimal<i128, 38, 6>;

    /// One line of an order: an item, how many, and its price and VAT rate when the order was taken.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Line {
        pub sku: Sku,
        pub quantity: Quantity,
        pub unit_price: Price,
        pub vat_rate: Rate,
    }

    /// An order, and how many people share its bill.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Order {
        pub number: u32,
        pub lines: List<Line>,
        pub payers: Payers,
    }

    /// The Y rule `taxed`.
    ///
    /// - `line`: the line to judge.
    pub fn taxed(line: &Line) -> bool {
        line.vat_rate > Rate::zero()
    }

    /// What a line costs before VAT.
    ///
    /// - `line`: the line.
    /// - Returns its price times its quantity, exact.
    pub fn net_of(line: &Line) -> Net {
        line.unit_price.to_decimal::<i128, 38, 2>().times(line.quantity.to_i128())
    }

    /// The VAT on a line, exact: it is rounded only with the order's total.
    ///
    /// - `line`: the line.
    /// - Returns its net price times its VAT rate.
    pub fn vat_of(line: &Line) -> Vat {
        net_of(line).multiply(line.vat_rate.to_decimal::<i128, 38, 4>())
    }

    /// What an order costs: its lines' net prices, with the VAT on the taxed ones, rounded once, to the cent, half away
    /// from zero.
    ///
    /// - `order`: the order.
    /// - Returns the order's total.
    pub fn order_total(order: &Order) -> Price {
        let net = order.lines.iter().map(net_of).fold(Net::zero(), |a, b| a + b);
        let vat = order.lines.iter().filter(|line| taxed(line)).map(vat_of).fold(Vat::zero(), |a, b| a + b);
        (net.to_decimal::<i128, 38, 6>() + vat).to_decimal_rounded(Rounding::HalfAwayFromZero)
    }

    /// Splits a bill between its payers: each pays an equal share, rounded down to the cent, and the first payer also
    /// pays the cents left over.
    ///
    /// - `total`: the bill.
    /// - `payers`: how many pay it.
    /// - Returns what each pays, in order, which add up to the bill.
    pub fn split(total: Price, payers: Payers) -> List<Price> {
        let fraction: Rational<Big, UNBOUNDED> = total.to_rational();
        let exact = fraction / Rational::new(payers.to_i128(), 1);
        let each: Price = exact.to_decimal_rounded(Rounding::TowardNegative);
        let left = total - each.times(payers.to_i128());
        let mut owed = List::from(list::repeated(each, payers.to_i128() as u32));
        if left == Price::zero() { // y: #even
            owed
        } else { // y: #uneven
            list::set_number(&mut owed, 1, each + left);
            owed
        }
    }
}

/// The Y facet `stock`: what the warehouse holds, shared by every task that packs.
pub mod stock {
    use super::catalogue::Sku;
    use super::pricing::Line;
    use std::convert::Infallible;
    use y_collections::{Bag, Queue};
    use y_runtime::not_reachable;
    use y_runtime::shared::{atomic_with, orelse, Isolation, Shared, Step, Tx};

    /// The units on the shelves, and the lines waiting for more.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Holdings {
        pub on_shelf: Bag<Sku>,
        pub backordered: Queue<Line>,
    }

    /// Where a line's units came from.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Reservation {
        FromShelf,
        Backordered,
    }

    /// Reserves a line's units: from the shelf when it holds enough of them, and otherwise on backorder, in the same
    /// transaction, so no two tasks take the same units.
    ///
    /// - `wanted`: the line to reserve.
    /// - `holdings`: the Y `injected` parameter.
    /// - Returns where the units came from.
    pub fn reserve(wanted: &Line, holdings: &Shared<Holdings>) -> Reservation {
        let quantity = wanted.quantity.to_usize();
        let mut take_from_shelf = |tx: &mut Tx| -> Result<Step<Reservation>, Infallible> { // y: #take_from_shelf
            let mut held = tx.get(holdings);
            // y: waits until enough
            if held.on_shelf.count_of(&wanted.sku) < quantity {
                return Ok(Step::Retry);
            }
            held.on_shelf.remove_times(&wanted.sku, quantity);
            tx.set(holdings, held);
            Ok(Step::Done(Reservation::FromShelf))
        };
        let mut put_on_backorder = |tx: &mut Tx| -> Result<Step<Reservation>, Infallible> { // y: #put_on_backorder
            let mut held = tx.get(holdings);
            held.backordered.add(wanted.clone()); // y: a copy
            tx.set(holdings, held);
            Ok(Step::Done(Reservation::Backordered))
        };
        match orelse("warehouse::reserve", None, vec![&mut take_from_shelf, &mut put_on_backorder]) {
            Ok(reserved) => reserved,
            Err(_) => not_reachable("warehouse::reserve#put_on_backorder"),
        }
    }

    /// Counts the units on the shelf, for a report. It reads one consistent snapshot of the stock and writes nothing, so a
    /// reservation that commits while it runs does not make it run again.
    ///
    /// - `holdings`: the Y `injected` parameter.
    /// - Returns how many units are on the shelf.
    pub fn units_on_shelf(holdings: &Shared<Holdings>) -> usize {
        // y: region counted is atomic, isolation snapshot
        let counted = atomic_with("warehouse::units_on_shelf#counted", None, Isolation::Snapshot, |tx| -> Result<Step<usize>, Infallible> {
            Ok(Step::Done(tx.get(holdings).on_shelf.count()))
        });
        match counted {
            Ok(units) => units,
            Err(_) => not_reachable("warehouse::units_on_shelf#counted"),
        }
    }
}

/// The Y facet `packing`: packing the day's orders, two tasks at a time.
pub mod packing {
    use super::catalogue::Sku;
    use super::pricing::{Line, Order};
    use super::stock::{reserve, Holdings, Reservation};
    use std::convert::Infallible;
    use std::time::Duration;
    use y_collections::{Bag, List};
    use y_runtime::shared::{atomic, Shared, Step};
    use y_runtime::task::{supervised, Restart, Supervision, TaskBody};
    use y_runtime::{Deadline, Expired};

    /// Why the day's packing stopped.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum PackingError {
        Expired { deadline: Expired },
    }

    /// An order's lines.
    ///
    /// - `order`: the order.
    /// - Returns its lines, in order.
    pub fn lines_of(order: &Order) -> List<Line> {
        order.lines.clone() // y: a copy
    }

    /// Packs orders: reserves each line, and counts the units it took from the shelf as sold.
    ///
    /// - `orders`: the orders.
    /// - `holdings`, `sold`: the Y `injected` parameters.
    pub fn pack_orders(orders: &List<Order>, holdings: &Shared<Holdings>, sold: &Shared<Bag<Sku>>) {
        let lines = orders.iter().flat_map(lines_of).collect::<List<Line>>();
        for wanted in &lines { // y: #wanted, else #no_lines
            let reserved = reserve(wanted, holdings);
            if reserved == Reservation::FromShelf { // y: #shelved
                // y: region count_sale is atomic
                let counted = atomic("warehouse::pack_orders#count_sale", None, |tx| -> Result<Step<()>, Infallible> {
                    let mut units = tx.get(sold);
                    units.add_times(wanted.sku.clone(), wanted.quantity.to_usize());
                    tx.set(sold, units);
                    Ok(Step::Done(()))
                });
                if counted.is_err() {
                    y_runtime::not_reachable("warehouse::pack_orders#count_sale");
                }
            } else { // y: #backordered
                continue;
            }
        }
    }

    /// Packs the day's orders with two tasks, each taking half of them, within ten seconds. A task that fails is started
    /// again, twice at most.
    ///
    /// - `orders`: the day's orders.
    /// - `holdings`, `sold`: the Y `injected` parameters.
    /// - Fails `Expired`: the packing took longer than ten seconds.
    pub fn pack_day(orders: &List<Order>, holdings: &Shared<Holdings>, sold: &Shared<Bag<Sku>>) -> Result<(), PackingError> {
        let half = orders.count().div_euclid(2);
        // y: region packing is within 10s or fail, supervise(restart: restart_policy.each_failed, at_most: 2, per: 1min)
        let deadline = Deadline::after(Duration::from_secs(10));
        let first_half: List<Order> = orders.iter().take(half).cloned().collect();
        let second_half: List<Order> = orders.iter().skip(half).cloned().collect();
        let first: TaskBody<'_, Infallible> = Box::new(|_| {
            pack_orders(&first_half, holdings, sold);
            Ok(())
        });
        let second: TaskBody<'_, Infallible> = Box::new(|_| {
            pack_orders(&second_half, holdings, sold);
            Ok(())
        });
        let policy = Supervision { restart: Restart::EachFailed, at_most: 2 };
        if let Err(failed) = supervised(policy, vec![first, second]) {
            match failed.failure {}
        }
        deadline.remaining().map_err(|deadline| PackingError::Expired { deadline })?;
        Ok(())
    }
}

/// The Y facet `receipts`: written for each order, and filed in the store.
pub mod receipts {
    use super::pricing::{order_total, Order};
    use std::time::Duration;
    use y_collections::List;
    use y_runtime::arena::{Arena, Held, Index};
    use y_runtime::store::{Schema, Store, StoreError};
    use y_runtime::task::Cancellation;
    use y_string::YString;

    /// A receipt as stored.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Receipt {
        pub order: u32,
        pub text: YString,
    }

    /// The codec of `schema receipt v1`: the text, with each line ending written `\n`.
    impl Schema for Receipt {
        const NAME: &'static str = "receipt";
        const VERSION: u32 = 1;

        fn key(&self) -> String {
            self.order.to_string()
        }

        fn encode(&self) -> String {
            self.text.as_str().replace('\n', "\\n")
        }

        fn decode(key: &str, version: u32, text: &str) -> Result<Self, String> {
            let order = key.parse().map_err(|_| format!("`{key}` is not an order's number"))?;
            match version {
                1 => Ok(Receipt { order, text: YString::new(&text.replace("\\n", "\n")) }),
                _ => Err(format!("not a receipt of version {version}")),
            }
        }
    }

    /// One line of a receipt being laid out, and the line it belongs under.
    pub struct Entry {
        pub text: YString,
        pub under: Option<Index<Entry>>,
    }

    /// Y's `receipt_scratch.rendered()`: the arena's lines in order, each under another indented by two spaces.
    fn rendered(scratch: &Arena<Entry>) -> YString {
        let lines: List<String> =
            scratch.iter().map(|e| if e.under.is_some() { format!("  {}", e.text) } else { e.text.to_string() }).collect();
        YString::from(lines.join("\n"))
    }

    /// Lays out an order's receipt: its number and total, then each line under them. The layout is built in an arena,
    /// which is released at once when the receipt is written.
    ///
    /// - `order`: the order.
    /// - Returns the receipt, one line each.
    pub fn receipt_text(order: &Order) -> YString {
        // y: arena receipt_scratch
        let mut receipt_scratch = Arena::new("receipt_scratch");
        let total = order_total(order);
        let heading = receipt_scratch.allocate(Entry { text: YString::from(format!("Order {}: {total}", order.number)), under: None });
        for wanted in &order.lines { // y: #wanted, else #no_lines
            let text = YString::from(format!("{} x {}", wanted.quantity, wanted.sku));
            receipt_scratch.allocate(Entry { text, under: Some(heading) });
        }
        rendered(&receipt_scratch)
    }

    /// Files the day's receipts in the store, as one batch: the batch is a resource, written to the store when it is
    /// released, and filing it is not cancelled half done.
    ///
    /// - `orders`: the day's orders.
    /// - `filed`: the Y `injected` parameter.
    /// - Fails: the batch could not be written.
    pub fn file_receipts(orders: &List<Order>, filed: &mut Store<Receipt>) -> Result<(), StoreError> {
        let mut batch = Held::new(filed.batch());
        // y: region filing is uncancellable, within 50ms
        Cancellation::new().held("warehouse::file_receipts#filing", Duration::from_millis(50), || {
            for order in orders { // y: #order, else #no_orders
                batch.get().put(Receipt { order: order.number, text: receipt_text(order) });
            }
        });
        batch.end()
    }
}
