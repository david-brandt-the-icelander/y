//! The native implementations of the module `claims` (Y § 3.18), written by hand. Each signature is the one the Y
//! compiler gives the declaration.
use std::cell::Cell;
use y_string::YString;

/// `tally_cell`: a counter the host keeps.
#[derive(Clone, Debug, PartialEq)]
pub struct TallyCell(Cell<u32>);

/// `checksum`: the CRC-32 of the text's UTF-8, by the standard reflected polynomial.
pub fn checksum(text: &YString) -> u32 {
    let mut crc = !0u32;
    for b in text.as_str().bytes() {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// `new_cell`: a counter at a number.
pub fn new_cell(start: u32) -> TallyCell {
    TallyCell(Cell::new(start))
}

/// `count_of`: what a counter holds.
pub fn count_of(cell: &TallyCell) -> u32 {
    cell.0.get()
}
