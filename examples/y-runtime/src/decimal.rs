//! Y's constrained integers, decimals, rationals and rounding modes (Y § 3.3): the native implementation of the standard library's
//! `integer<minimum=a, maximum=b>` and unbounded `integer`,
//! `decimal16` to `decimal256`, `decimal<precision=p, scale=s>`, `rational16` to `rational256`, `rational<precision=p>`
//! and `rounding`.
//!
//! A decimal stores its digits as an integer of its size, the units of its last place: `12.34` in a
//! `decimal64<scale=2>` is the `i64` 1234. Lowered code names the Rust type the lowering chose from the Y type:
//!
//! | Y | Rust |
//! |---|---|
//! | `decimal64<scale=2>`, which is `decimal<precision=18, scale=2>` | `Decimal64<2>`, which is `Decimal<i64, 18, 2>` |
//! | `decimal<precision=12, scale=2>` | `Decimal<i64, 12, 2>`: the smallest size that holds 12 digits |
//! | `rational128`, which is `rational<precision=38>` | `Rational128`, which is `Rational<i128, 38>`: two `i128`, 256 bits |
//! | `rational<precision=12>` | `Rational<i64, 12>` |
//! | `decimal<scale=2>`, with no precision | `Decimal<Big, UNBOUNDED, 2>` |
//! | `rational`, with no precision | `Rational<Big, UNBOUNDED>` |
//!
//! In Y a size is a constraint, not a type, and the compiler derives each result's constraints; the lowering converts
//! operands stored in different sizes with `to_decimal` and `to_rational`. Every value is checked against the precision
//! of the type it is stored in, and an overflow is a fault. Intermediate results are computed
//! exactly, in an integer of any size ([`Big`]), so a result that fits is never lost to an intermediate that would not.
//! That is slower than Y's runtime would be, and the same in meaning.
//!
//! Where Y's compiler rejects a program (a scale that would fall without a rounding mode, a product whose scale is not
//! the sum of its operands'), this stand-in, which has no compiler before it, faults instead.
use super::{fault, require};
use std::cmp::Ordering;
use std::fmt;

// ---------------------------------------------------------------------------------------------------------------------
// Rounding

/// Y's `rounding`: how an operation that cannot keep every digit chooses the one it keeps (Y § 3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rounding {
    /// To the nearest; a tie to the even neighbour. Banker's rounding, IEEE 754's default.
    HalfEven,
    /// To the nearest; a tie away from zero. Oracle's and T-SQL's `ROUND`, COBOL's `ROUNDED`.
    HalfAwayFromZero,
    /// To the nearest; a tie toward zero.
    HalfTowardZero,
    /// To the nearest; a tie toward +∞. JavaScript's `Math.round`.
    HalfTowardPositive,
    /// To the nearest; a tie toward −∞.
    HalfTowardNegative,
    /// To the neighbour toward zero: truncation.
    TowardZero,
    /// To the neighbour away from zero.
    AwayFromZero,
    /// To the neighbour toward +∞: the ceiling.
    TowardPositive,
    /// To the neighbour toward −∞: the floor.
    TowardNegative,
}

/// Y's failure `inexact`: an `_exact` operation would have had to round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inexact;

/// `numerator / denominator`, rounded to a whole number as `mode` says. The denominator is not zero.
fn divide_rounding(numerator: &Big, denominator: &Big, mode: Rounding) -> Big {
    let (quotient, remainder) = numerator.divide_truncated(denominator);
    if remainder.is_zero() {
        return quotient;
    }
    // The exact quotient lies strictly between `quotient` and `quotient + away`, `away` being one step from zero.
    let away = if numerator.negative != denominator.negative { Big::from_i128(-1) } else { Big::from_i128(1) };
    let positive = !away.negative;
    let further = quotient.plus(&away);
    let half = remainder.magnitude_doubled().compare_magnitude(denominator);
    let toward_positive = if positive { further.clone() } else { quotient.clone() };
    let toward_negative = if positive { quotient.clone() } else { further.clone() };
    match mode {
        Rounding::TowardZero => quotient,
        Rounding::AwayFromZero => further,
        Rounding::TowardPositive => toward_positive,
        Rounding::TowardNegative => toward_negative,
        _ => match half {
            Ordering::Less => quotient,
            Ordering::Greater => further,
            Ordering::Equal => match mode {
                Rounding::HalfEven => {
                    if quotient.is_even() {
                        quotient
                    } else {
                        further
                    }
                }
                Rounding::HalfAwayFromZero => further,
                Rounding::HalfTowardZero => quotient,
                Rounding::HalfTowardPositive => toward_positive,
                Rounding::HalfTowardNegative => toward_negative,
                _ => unreachable!("the directed modes are handled above"),
            },
        },
    }
}

/// `numerator / denominator` when it is a whole number.
fn divide_exactly(numerator: &Big, denominator: &Big) -> Result<Big, Inexact> {
    let (quotient, remainder) = numerator.divide_truncated(denominator);
    if remainder.is_zero() {
        Ok(quotient)
    } else {
        Err(Inexact)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Decimals

/// An integer type that stores a decimal's digits: `i16`, `i32`, `i64`, `i128` or [`I256`].
pub trait Digits: Clone + Eq + Ord + std::hash::Hash + fmt::Debug {
    /// The most digits the type always holds: ClickHouse's precision for its size (4 for `i16`, by the same rule).
    const PRECISION: u32;
    /// The value, exactly.
    fn to_big(&self) -> Big;
    /// The value, if the type holds it.
    fn from_big(value: &Big) -> Option<Self>;
}

macro_rules! digits {
    ($t:ty, $precision:expr) => {
        impl Digits for $t {
            const PRECISION: u32 = $precision;
            fn to_big(&self) -> Big {
                Big::from_i128(*self as i128)
            }
            fn from_big(value: &Big) -> Option<Self> {
                value.to_i128().and_then(|v| <$t>::try_from(v).ok())
            }
        }
    };
}

digits!(i8, 2);
digits!(i16, 4);
digits!(i32, 9);
digits!(i64, 18);
digits!(i128, 38);

/// The precision of a decimal or rational with none: Y's `decimal<scale=s>` and `rational`, stored in a [`Big`].
pub const UNBOUNDED: u32 = u32::MAX;

impl Digits for Big {
    const PRECISION: u32 = UNBOUNDED;

    fn to_big(&self) -> Big {
        self.clone()
    }

    fn from_big(value: &Big) -> Option<Self> {
        Some(value.clone())
    }
}

/// A signed 256-bit integer in two's complement, least significant word first: the storage of a `decimal256`. Rust has
/// none, so this one only stores; arithmetic goes through [`Big`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct I256([u64; 4]);

impl PartialOrd for I256 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for I256 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.to_big().compare(&other.to_big())
    }
}

impl Digits for I256 {
    const PRECISION: u32 = 76;

    fn to_big(&self) -> Big {
        let negative = self.0[3] >> 63 == 1;
        let mut words = self.0;
        if negative {
            // The magnitude of a negative number: invert, and add one.
            let mut carry = 1u128;
            for word in words.iter_mut() {
                let sum = (!*word) as u128 + carry;
                *word = sum as u64;
                carry = sum >> 64;
            }
        }
        let mut magnitude = Vec::new();
        for word in words {
            magnitude.push(word as u32);
            magnitude.push((word >> 32) as u32);
        }
        Big::new(negative, magnitude)
    }

    fn from_big(value: &Big) -> Option<Self> {
        if value.magnitude.len() > 8 {
            return None;
        }
        let mut words = [0u64; 4];
        for (i, limb) in value.magnitude.iter().enumerate() {
            words[i / 2] |= (*limb as u64) << (32 * (i % 2));
        }
        let top_bit = words[3] >> 63 == 1;
        if value.negative {
            // Up to 2^255 in magnitude; then negate.
            if top_bit && (words[3] != 1 << 63 || words[..3].iter().any(|w| *w != 0)) {
                return None;
            }
            let mut carry = 1u128;
            for word in words.iter_mut() {
                let sum = (!*word) as u128 + carry;
                *word = sum as u64;
                carry = sum >> 64;
            }
        } else if top_bit {
            return None;
        }
        Some(I256(words))
    }
}

/// A decimal of `P` digits, `S` of them after the point, stored in `D` (Y § 3.3).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Decimal<D: Digits, const P: u32, const S: u32> {
    units: D,
}

/// Y's `decimal16<scale=s>`.
pub type Decimal16<const S: u32> = Decimal<i16, 4, S>;
/// Y's `decimal32<scale=s>`.
pub type Decimal32<const S: u32> = Decimal<i32, 9, S>;
/// Y's `decimal64<scale=s>`.
pub type Decimal64<const S: u32> = Decimal<i64, 18, S>;
/// Y's `decimal128<scale=s>`.
pub type Decimal128<const S: u32> = Decimal<i128, 38, S>;
/// Y's `decimal256<scale=s>`.
pub type Decimal256<const S: u32> = Decimal<I256, 76, S>;

impl<D: Digits, const P: u32, const S: u32> Decimal<D, P, S> {
    const IDENTITY: &'static str = "y_runtime::decimal";

    /// The decimal whose last place holds `units`, if its precision holds it.
    fn from_units(units: &Big) -> Option<Self> {
        assert!(P >= 1 && P <= D::PRECISION && S <= P, "a decimal's precision and scale are out of range");
        if P != UNBOUNDED && units.compare_magnitude(&Big::power_of_ten(P)) != Ordering::Less {
            return None;
        }
        D::from_big(units).map(|units| Decimal { units })
    }

    /// The same, where not holding it is an overflow: a fault.
    fn from_units_or_fault(units: &Big) -> Self {
        Self::from_units(units).unwrap_or_else(|| fault(Self::IDENTITY, "overflow"))
    }

    /// The units of the last place, exactly.
    pub fn units(&self) -> Big {
        self.units.to_big()
    }

    /// Zero.
    pub fn zero() -> Self {
        Self::from_units_or_fault(&Big::zero())
    }

    /// A decimal written in text, such as `-12.5`: the value of a Y literal. Text that is not a decimal, or has more
    /// digits after the point than the scale, is `None`, as such a literal does not compile.
    pub fn parse(text: &str) -> Option<Self> {
        let (negative, digits) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        let digits_only = |part: &str| part.chars().all(|c| c.is_ascii_digit());
        if whole.is_empty() || !digits_only(whole) || !digits_only(fraction) || fraction.len() > S as usize {
            return None;
        }
        let padded = format!("{whole}{fraction:0<width$}", width = S as usize);
        let units = Big::parse_whole(&padded)?;
        Self::from_units(&if negative { units.negate() } else { units })
    }

    /// A literal, which Y's compiler checked; one that is not this type's is a fault.
    pub fn literal(text: &str) -> Self {
        Self::parse(text).unwrap_or_else(|| fault(Self::IDENTITY, "a literal this decimal cannot hold"))
    }

    /// A whole number as this decimal: an integer converted. One the precision cannot hold is an overflow.
    pub fn from_integer(value: i128) -> Self {
        Self::from_units_or_fault(&Big::from_i128(value).multiply(&Big::power_of_ten(S)))
    }

    /// Y's `a * n`, a decimal times an integer: the decimal's type.
    pub fn times(self, n: i128) -> Self {
        Self::from_units_or_fault(&self.units().multiply(&Big::from_i128(n)))
    }

    /// Y's `a * b` between two decimals of one size: the scale is the sum of theirs, at the size's precision, as in
    /// ClickHouse. The lowering names the result's type; one whose scale is not `S + S2` is a fault here, and does not
    /// compile in Y.
    pub fn multiply<const P2: u32, const S2: u32, const P3: u32, const S3: u32>(
        self,
        other: Decimal<D, P2, S2>,
    ) -> Decimal<D, P3, S3> {
        if S3 != S + S2 {
            fault(Self::IDENTITY, "a product's scale is the sum of its operands' scales");
        }
        Decimal::<D, P3, S3>::from_units_or_fault(&self.units().multiply(&other.units()))
    }

    /// The quotient's units before rounding, as a fraction: `self / other` at this decimal's scale.
    fn quotient_parts<const P2: u32, const S2: u32>(self, other: Decimal<D, P2, S2>) -> (Big, Big) {
        require(!other.units().is_zero(), "y_runtime::decimal::divide#nonzero_divisor");
        (self.units().multiply(&Big::power_of_ten(S2)), other.units())
    }

    /// Y's `a.divide_rounded(b, mode)`: the quotient at the dividend's scale, as in ClickHouse, rounded as named.
    pub fn divide_rounded<const P2: u32, const S2: u32>(self, other: Decimal<D, P2, S2>, mode: Rounding) -> Self {
        let (numerator, denominator) = self.quotient_parts(other);
        Self::from_units_or_fault(&divide_rounding(&numerator, &denominator, mode))
    }

    /// Y's `a.divide_exact(b)`: the quotient at the dividend's scale, or `inexact` when it has more digits.
    pub fn divide_exact<const P2: u32, const S2: u32>(self, other: Decimal<D, P2, S2>) -> Result<Self, Inexact> {
        let (numerator, denominator) = self.quotient_parts(other);
        divide_exactly(&numerator, &denominator).map(|units| Self::from_units_or_fault(&units))
    }

    /// Y's `to_decimal(...)` and `to_decimal64(scale: ...)` and the like: the same value in another decimal type,
    /// keeping every digit. A scale that would fall is a fault here; in Y it does not compile.
    pub fn to_decimal<D2: Digits, const P2: u32, const S2: u32>(self) -> Decimal<D2, P2, S2> {
        if S2 < S {
            fault(Self::IDENTITY, "a scale that falls needs a rounding mode");
        }
        Decimal::<D2, P2, S2>::from_units_or_fault(&self.units().multiply(&Big::power_of_ten(S2 - S)))
    }

    /// Y's `to_decimal_rounded(precision: p, scale: s, rounding: mode)`: the value in another decimal type, rounded to
    /// its scale as named.
    pub fn to_decimal_rounded<D2: Digits, const P2: u32, const S2: u32>(self, mode: Rounding) -> Decimal<D2, P2, S2> {
        let units = if S2 >= S {
            self.units().multiply(&Big::power_of_ten(S2 - S))
        } else {
            divide_rounding(&self.units(), &Big::power_of_ten(S - S2), mode)
        };
        Decimal::<D2, P2, S2>::from_units_or_fault(&units)
    }

    /// Y's `to_integer_rounded(mode)`. A whole number beyond an `int128` is an overflow.
    pub fn to_integer_rounded(self, mode: Rounding) -> i128 {
        divide_rounding(&self.units(), &Big::power_of_ten(S), mode)
            .to_i128()
            .unwrap_or_else(|| fault(Self::IDENTITY, "overflow"))
    }

    /// Y's `to_float64_nearest()`: the nearest `float64`, a tie to even.
    pub fn to_float64_nearest(self) -> f64 {
        // Rust parses decimal text to the nearest float, so the text is the exact route.
        format!("{}e-{}", self.units(), S).parse().unwrap_or_else(|_| unreachable!("the text is a number"))
    }

    /// `float64`'s `to_decimal_rounded(...)`: the float's exact binary value, rounded to this scale as named. A value
    /// that is not finite is a fault.
    pub fn from_float64_rounded(value: f64, mode: Rounding) -> Self {
        let (numerator, denominator) = Big::fraction_of_float(value)
            .unwrap_or_else(|| fault(Self::IDENTITY, "a float that is not finite"));
        let scaled = numerator.multiply(&Big::power_of_ten(S));
        Self::from_units_or_fault(&divide_rounding(&scaled, &denominator, mode))
    }

    /// Y's comparison of two decimals of one size at different scales, which is exact.
    pub fn compare<const P2: u32, const S2: u32>(self, other: Decimal<D, P2, S2>) -> Ordering {
        let scale = S.max(S2);
        let left = self.units().multiply(&Big::power_of_ten(scale - S));
        let right = other.units().multiply(&Big::power_of_ten(scale - S2));
        left.compare(&right)
    }

    /// Y's `to_rational64()` and the like: the same value as a rational, exactly.
    pub fn to_rational<D2: Digits, const P2: u32>(self) -> Rational<D2, P2> {
        Rational::from_fraction(&self.units(), &Big::power_of_ten(S))
    }
}

impl<D: Digits, const P: u32, const S: u32> std::ops::Add for Decimal<D, P, S> {
    type Output = Self;
    /// Y's `+` on two decimals of one type. An overflow is a fault.
    fn add(self, other: Self) -> Self {
        Self::from_units_or_fault(&self.units().plus(&other.units()))
    }
}

impl<D: Digits, const P: u32, const S: u32> std::ops::Sub for Decimal<D, P, S> {
    type Output = Self;
    /// Y's `-` on two decimals of one type. An overflow is a fault.
    fn sub(self, other: Self) -> Self {
        Self::from_units_or_fault(&self.units().minus(&other.units()))
    }
}

impl<D: Digits, const P: u32, const S: u32> std::ops::Neg for Decimal<D, P, S> {
    type Output = Self;
    fn neg(self) -> Self {
        Self::from_units_or_fault(&self.units().negate())
    }
}

impl<D: Digits, const P: u32, const S: u32> fmt::Display for Decimal<D, P, S> {
    /// The value with exactly `S` digits after the point: `12.30`, `-0.05`, `7`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let units = self.units();
        let digits = format!("{:0>width$}", units.magnitude_text(), width = S as usize + 1);
        let (whole, fraction) = digits.split_at(digits.len() - S as usize);
        let sign = if units.negative { "-" } else { "" };
        if S == 0 {
            write!(f, "{sign}{whole}")
        } else {
            write!(f, "{sign}{whole}.{fraction}")
        }
    }
}

impl<D: Digits, const P: u32, const S: u32> fmt::Debug for Decimal<D, P, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Integers

/// Y's `integer<minimum=MIN, maximum=MAX>`, stored in `D`, which the lowering chose as the smallest integer that holds
/// the range (Y § 3.3). An unbounded `integer` is a [`Big`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Integer<D: Digits, const MIN: i128, const MAX: i128>(D);

impl<D: Digits, const MIN: i128, const MAX: i128> Integer<D, MIN, MAX> {
    /// The value where it meets its constraints. One outside them is an overflow: a fault.
    pub fn new(value: &Big) -> Self {
        let inside = value.compare(&Big::from_i128(MIN)) != Ordering::Less && value.compare(&Big::from_i128(MAX)) != Ordering::Greater;
        match D::from_big(value) {
            Some(stored) if inside => Integer(stored),
            _ => fault("y_runtime::integer", "overflow"),
        }
    }

    /// The same, from a fixed-width value: Y's `to_integer()`.
    pub fn from_i128(value: i128) -> Self {
        Self::new(&Big::from_i128(value))
    }

    /// The value, exactly, for arithmetic, whose result the lowering stores where it meets its next constraint.
    pub fn value(&self) -> Big {
        self.0.to_big()
    }

    /// Y's `to_int64()` and the like: the value at a fixed width, which faults where it does not fit.
    pub fn to_i128(&self) -> i128 {
        self.value().to_i128().unwrap_or_else(|| fault("y_runtime::integer", "overflow"))
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Rationals

/// A rational: a numerator and a denominator of `P` digits each, stored in `D`, in lowest terms with a positive
/// denominator (Y § 3.3). Zero is 0/1, so each value has one representation, and the derived equality and hashing are
/// those of numbers.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rational<D: Digits, const P: u32> {
    numerator: D,
    denominator: D,
}

/// Y's `rational16`: 32 bits.
pub type Rational16 = Rational<i16, 4>;
/// Y's `rational32`: 64 bits.
pub type Rational32 = Rational<i32, 9>;
/// Y's `rational64`: 128 bits.
pub type Rational64 = Rational<i64, 18>;
/// Y's `rational128`: 256 bits.
pub type Rational128 = Rational<i128, 38>;
/// Y's `rational256`: 512 bits.
pub type Rational256 = Rational<I256, 76>;

impl<D: Digits, const P: u32> Rational<D, P> {
    const IDENTITY: &'static str = "y_runtime::rational";

    /// The fraction in lowest terms. A part that its precision cannot hold, after reducing, is an overflow.
    fn from_fraction(numerator: &Big, denominator: &Big) -> Self {
        assert!(P >= 1 && P <= D::PRECISION, "a rational's precision is out of range");
        require(!denominator.is_zero(), "y_runtime::rational::new#nonzero_denominator");
        let divisor = numerator.greatest_common_divisor(denominator);
        let (mut numerator, _) = numerator.divide_truncated(&divisor);
        let (mut denominator, _) = denominator.divide_truncated(&divisor);
        if denominator.negative {
            numerator = numerator.negate();
            denominator = denominator.negate();
        }
        let limit = (P != UNBOUNDED).then(|| Big::power_of_ten(P));
        let part = |value: &Big| {
            if limit.as_ref().is_some_and(|limit| value.compare_magnitude(limit) != Ordering::Less) {
                fault(Self::IDENTITY, "overflow");
            }
            D::from_big(value).unwrap_or_else(|| fault(Self::IDENTITY, "overflow"))
        };
        Rational { numerator: part(&numerator), denominator: part(&denominator) }
    }

    /// Y's `rational64.new(numerator: n, denominator: d)`. A zero denominator breaks its precondition: a fault.
    pub fn new(numerator: i128, denominator: i128) -> Self {
        Self::from_fraction(&Big::from_i128(numerator), &Big::from_i128(denominator))
    }

    /// The numerator, in lowest terms; its sign is the rational's.
    pub fn numerator(&self) -> Big {
        self.numerator.to_big()
    }

    /// The denominator, in lowest terms: always positive.
    pub fn denominator(&self) -> Big {
        self.denominator.to_big()
    }

    /// Y's `to_decimal_rounded(precision: p, scale: s, rounding: mode)`.
    pub fn to_decimal_rounded<D2: Digits, const P2: u32, const S2: u32>(self, mode: Rounding) -> Decimal<D2, P2, S2> {
        let scaled = self.numerator().multiply(&Big::power_of_ten(S2));
        Decimal::<D2, P2, S2>::from_units_or_fault(&divide_rounding(&scaled, &self.denominator(), mode))
    }

    /// Y's `to_decimal_exact(...)`: the value as a decimal of that scale, or `inexact` when it has more digits.
    pub fn to_decimal_exact<D2: Digits, const P2: u32, const S2: u32>(self) -> Result<Decimal<D2, P2, S2>, Inexact> {
        let scaled = self.numerator().multiply(&Big::power_of_ten(S2));
        divide_exactly(&scaled, &self.denominator()).map(|units| Decimal::<D2, P2, S2>::from_units_or_fault(&units))
    }
}

impl<D: Digits, const P: u32> PartialOrd for Rational<D, P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<D: Digits, const P: u32> Ord for Rational<D, P> {
    /// By value: a/b against c/d is a·d against c·b, the denominators being positive.
    fn cmp(&self, other: &Self) -> Ordering {
        let left = self.numerator().multiply(&other.denominator());
        let right = other.numerator().multiply(&self.denominator());
        left.compare(&right)
    }
}

impl<D: Digits, const P: u32> std::ops::Add for Rational<D, P> {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        let numerator = self.numerator().multiply(&other.denominator()).plus(&other.numerator().multiply(&self.denominator()));
        Self::from_fraction(&numerator, &self.denominator().multiply(&other.denominator()))
    }
}

impl<D: Digits, const P: u32> std::ops::Sub for Rational<D, P> {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        self + -other
    }
}

impl<D: Digits, const P: u32> std::ops::Neg for Rational<D, P> {
    type Output = Self;
    fn neg(self) -> Self {
        Self::from_fraction(&self.numerator().negate(), &self.denominator())
    }
}

impl<D: Digits, const P: u32> std::ops::Mul for Rational<D, P> {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        Self::from_fraction(
            &self.numerator().multiply(&other.numerator()),
            &self.denominator().multiply(&other.denominator()),
        )
    }
}

impl<D: Digits, const P: u32> std::ops::Div for Rational<D, P> {
    type Output = Self;
    /// Y's `/` on rationals: exact. Dividing by zero breaks its precondition: a fault.
    fn div(self, other: Self) -> Self {
        require(!other.numerator().is_zero(), "y_runtime::rational::divide#nonzero_divisor");
        Self::from_fraction(
            &self.numerator().multiply(&other.denominator()),
            &self.denominator().multiply(&other.numerator()),
        )
    }
}

impl<D: Digits, const P: u32> fmt::Display for Rational<D, P> {
    /// `n/d`, or `n` when the denominator is one.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let denominator = self.denominator();
        if denominator == Big::from_i128(1) {
            write!(f, "{}", self.numerator())
        } else {
            write!(f, "{}/{}", self.numerator(), denominator)
        }
    }
}

impl<D: Digits, const P: u32> fmt::Debug for Rational<D, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Exact integers

/// An integer of any size: the exact intermediate results of decimal and rational arithmetic, and the storage of a
/// decimal or rational with no precision. A sign and a magnitude in 32-bit limbs, least significant first, with no zero
/// limb at the top. Zero is not negative.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Big {
    negative: bool,
    magnitude: Vec<u32>,
}

impl Big {
    fn new(negative: bool, mut magnitude: Vec<u32>) -> Big {
        while magnitude.last() == Some(&0) {
            magnitude.pop();
        }
        let negative = negative && !magnitude.is_empty();
        Big { negative, magnitude }
    }

    fn zero() -> Big {
        Big::new(false, Vec::new())
    }

    /// The integer `value`.
    pub fn from_i128(value: i128) -> Big {
        let mut rest = value.unsigned_abs();
        let mut magnitude = Vec::new();
        while rest > 0 {
            magnitude.push(rest as u32);
            rest >>= 32;
        }
        Big::new(value < 0, magnitude)
    }

    /// The value as an `i128`, if it fits.
    pub fn to_i128(&self) -> Option<i128> {
        if self.magnitude.len() > 4 {
            return None;
        }
        let mut value: u128 = 0;
        for limb in self.magnitude.iter().rev() {
            value = (value << 32) | *limb as u128;
        }
        if self.negative {
            if value <= i128::MAX as u128 + 1 {
                Some((value as i128).wrapping_neg())
            } else {
                None
            }
        } else {
            i128::try_from(value).ok()
        }
    }

    fn parse_whole(digits: &str) -> Option<Big> {
        let ten = Big::from_i128(10);
        let mut value = Big::zero();
        for c in digits.chars() {
            value = value.multiply(&ten).plus(&Big::from_i128(c.to_digit(10)? as i128));
        }
        Some(value)
    }

    fn power_of_ten(exponent: u32) -> Big {
        let ten = Big::from_i128(10);
        (0..exponent).fold(Big::from_i128(1), |value, _| value.multiply(&ten))
    }

    /// The exact value of a finite float, as a numerator and a positive denominator (a power of two).
    fn fraction_of_float(value: f64) -> Option<(Big, Big)> {
        if !value.is_finite() {
            return None;
        }
        let bits = value.to_bits();
        let negative = bits >> 63 == 1;
        let exponent = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1 << 52) - 1);
        let (mantissa, power) = if exponent == 0 { (fraction, -1074) } else { (fraction | 1 << 52, exponent - 1075) };
        let mut numerator = Big::from_i128(mantissa as i128);
        let mut denominator = Big::from_i128(1);
        let two = Big::from_i128(2);
        for _ in 0..power.unsigned_abs() {
            if power > 0 {
                numerator = numerator.multiply(&two);
            } else {
                denominator = denominator.multiply(&two);
            }
        }
        Some((if negative { numerator.negate() } else { numerator }, denominator))
    }

    pub fn is_zero(&self) -> bool {
        self.magnitude.is_empty()
    }

    fn is_even(&self) -> bool {
        self.magnitude.first().is_none_or(|limb| limb % 2 == 0)
    }

    pub fn negate(&self) -> Big {
        Big::new(!self.negative, self.magnitude.clone())
    }

    pub fn plus(&self, other: &Big) -> Big {
        if self.negative == other.negative {
            return Big::new(self.negative, add_magnitudes(&self.magnitude, &other.magnitude));
        }
        match compare_magnitudes(&self.magnitude, &other.magnitude) {
            Ordering::Less => Big::new(other.negative, subtract_magnitudes(&other.magnitude, &self.magnitude)),
            _ => Big::new(self.negative, subtract_magnitudes(&self.magnitude, &other.magnitude)),
        }
    }

    pub fn minus(&self, other: &Big) -> Big {
        self.plus(&other.negate())
    }

    pub fn multiply(&self, other: &Big) -> Big {
        let mut product = vec![0u32; self.magnitude.len() + other.magnitude.len()];
        for (i, a) in self.magnitude.iter().enumerate() {
            let mut carry = 0u64;
            for (j, b) in other.magnitude.iter().enumerate() {
                let sum = product[i + j] as u64 + (*a as u64) * (*b as u64) + carry;
                product[i + j] = sum as u32;
                carry = sum >> 32;
            }
            product[i + other.magnitude.len()] = carry as u32;
        }
        Big::new(self.negative != other.negative, product)
    }

    /// The quotient rounded toward zero, and the remainder, which has the dividend's sign. The divisor is not zero.
    pub fn divide_truncated(&self, divisor: &Big) -> (Big, Big) {
        assert!(!divisor.is_zero(), "division by zero");
        let bits = self.magnitude.len() * 32;
        let mut quotient = vec![0u32; self.magnitude.len()];
        let mut remainder: Vec<u32> = Vec::new();
        for bit in (0..bits).rev() {
            remainder = shift_left_one(&remainder, (self.magnitude[bit / 32] >> (bit % 32)) & 1);
            if compare_magnitudes(&remainder, &divisor.magnitude) != Ordering::Less {
                remainder = subtract_magnitudes(&remainder, &divisor.magnitude);
                quotient[bit / 32] |= 1 << (bit % 32);
            }
        }
        (Big::new(self.negative != divisor.negative, quotient), Big::new(self.negative, remainder))
    }

    fn greatest_common_divisor(&self, other: &Big) -> Big {
        let (mut a, mut b) = (Big::new(false, self.magnitude.clone()), Big::new(false, other.magnitude.clone()));
        while !b.is_zero() {
            let (_, remainder) = a.divide_truncated(&b);
            a = b;
            b = remainder;
        }
        if a.is_zero() {
            Big::from_i128(1)
        } else {
            a
        }
    }

    fn magnitude_doubled(&self) -> Big {
        Big::new(false, shift_left_one(&self.magnitude, 0))
    }

    fn compare_magnitude(&self, other: &Big) -> Ordering {
        compare_magnitudes(&self.magnitude, &other.magnitude)
    }

    pub fn compare(&self, other: &Big) -> Ordering {
        match (self.negative, other.negative) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => self.compare_magnitude(other),
            (true, true) => other.compare_magnitude(self),
        }
    }

    fn magnitude_text(&self) -> String {
        if self.is_zero() {
            return "0".to_string();
        }
        let billion = Big::from_i128(1_000_000_000);
        let mut chunks = Vec::new();
        let mut rest = Big::new(false, self.magnitude.clone());
        while !rest.is_zero() {
            let (quotient, remainder) = rest.divide_truncated(&billion);
            chunks.push(remainder.to_i128().unwrap_or(0));
            rest = quotient;
        }
        let mut text = chunks.pop().map(|chunk| chunk.to_string()).unwrap_or_default();
        for chunk in chunks.iter().rev() {
            text.push_str(&format!("{chunk:09}"));
        }
        text
    }
}

impl Big {
    /// Y's `divide_rounded(b, mode)` on unbounded integers. The divisor is not zero: its precondition.
    pub fn divide_rounded(&self, divisor: &Big, mode: Rounding) -> Big {
        require(!divisor.is_zero(), "y_runtime::integer::divide#nonzero_divisor");
        divide_rounding(self, divisor, mode)
    }
}

macro_rules! big_op {
    ($trait:ident, $method:ident, $via:ident) => {
        impl std::ops::$trait for Big {
            type Output = Big;
            fn $method(self, other: Big) -> Big {
                Big::$via(&self, &other)
            }
        }
    };
}

big_op!(Add, add, plus);
big_op!(Sub, sub, minus);
big_op!(Mul, mul, multiply);

impl PartialOrd for Big {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Big {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
    }
}

impl fmt::Display for Big {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.negative { "-" } else { "" };
        write!(f, "{sign}{}", self.magnitude_text())
    }
}

fn compare_magnitudes(a: &[u32], b: &[u32]) -> Ordering {
    let a = &a[..a.iter().rposition(|limb| *limb != 0).map_or(0, |i| i + 1)];
    let b = &b[..b.iter().rposition(|limb| *limb != 0).map_or(0, |i| i + 1)];
    a.len().cmp(&b.len()).then_with(|| a.iter().rev().cmp(b.iter().rev()))
}

fn add_magnitudes(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut sum = Vec::with_capacity(a.len().max(b.len()) + 1);
    let mut carry = 0u64;
    for i in 0..a.len().max(b.len()) {
        let total = *a.get(i).unwrap_or(&0) as u64 + *b.get(i).unwrap_or(&0) as u64 + carry;
        sum.push(total as u32);
        carry = total >> 32;
    }
    sum.push(carry as u32);
    sum
}

/// `a - b`, where `a` is at least `b`.
fn subtract_magnitudes(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut difference = Vec::with_capacity(a.len());
    let mut borrow = 0i64;
    for (i, limb) in a.iter().enumerate() {
        let mut total = *limb as i64 - *b.get(i).unwrap_or(&0) as i64 - borrow;
        borrow = 0;
        if total < 0 {
            total += 1 << 32;
            borrow = 1;
        }
        difference.push(total as u32);
    }
    while difference.last() == Some(&0) {
        difference.pop();
    }
    difference
}

/// `a * 2 + bit`.
fn shift_left_one(a: &[u32], bit: u32) -> Vec<u32> {
    let mut shifted = Vec::with_capacity(a.len() + 1);
    let mut carry = bit;
    for limb in a {
        shifted.push((limb << 1) | carry);
        carry = limb >> 31;
    }
    if carry != 0 {
        shifted.push(carry);
    }
    shifted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d2(text: &str) -> Decimal64<2> {
        Decimal64::<2>::literal(text)
    }

    #[test]
    fn sizes_follow_clickhouse_and_rationals_are_two_decimals() {
        assert_eq!(std::mem::size_of::<Decimal16<2>>(), 2);
        assert_eq!(std::mem::size_of::<Decimal32<2>>(), 4);
        assert_eq!(std::mem::size_of::<Decimal64<2>>(), 8);
        assert_eq!(std::mem::size_of::<Decimal128<2>>(), 16);
        assert_eq!(std::mem::size_of::<Decimal256<2>>(), 32);
        assert_eq!(std::mem::size_of::<Rational128>() * 8, 256);
        assert_eq!(std::mem::size_of::<Rational256>() * 8, 512);
        assert_eq!(Decimal16::<0>::parse("9999").map(|d| d.to_string()), Some("9999".into()));
        assert_eq!(Decimal16::<0>::parse("10000"), None);
        assert_eq!(Decimal32::<4>::parse("99999.9999").map(|d| d.to_string()), Some("99999.9999".into()));
        assert_eq!(Decimal32::<4>::parse("100000"), None);
        assert_eq!(Decimal32::<4>::parse("1.23456"), None, "more digits after the point than the scale");
    }

    #[test]
    fn a_precision_below_its_size_is_checked_on_every_result() {
        type Money = Decimal<i64, 12, 2>;
        let largest = Money::literal("9999999999.99");
        assert_eq!(largest.to_string(), "9999999999.99");
        assert!(std::panic::catch_unwind(|| largest + Money::literal("0.01")).is_err());
    }

    #[test]
    fn every_rounding_mode_on_ties_and_non_ties() {
        use Rounding::*;
        let cases: [(&str, [&str; 9]); 6] = [
            ("2.5", ["2", "3", "2", "3", "2", "2", "3", "3", "2"]),
            ("-2.5", ["-2", "-3", "-2", "-2", "-3", "-2", "-3", "-2", "-3"]),
            ("3.5", ["4", "4", "3", "4", "3", "3", "4", "4", "3"]),
            ("2.4", ["2", "2", "2", "2", "2", "2", "3", "3", "2"]),
            ("-2.6", ["-3", "-3", "-3", "-3", "-3", "-2", "-3", "-2", "-3"]),
            ("7", ["7", "7", "7", "7", "7", "7", "7", "7", "7"]),
        ];
        let modes = [
            HalfEven,
            HalfAwayFromZero,
            HalfTowardZero,
            HalfTowardPositive,
            HalfTowardNegative,
            TowardZero,
            AwayFromZero,
            TowardPositive,
            TowardNegative,
        ];
        for (value, expected) in cases {
            for (mode, want) in modes.iter().zip(expected) {
                let got = Decimal64::<1>::literal(value).to_decimal_rounded::<i64, 18, 0>(*mode);
                assert_eq!(got.to_string(), want, "{value} rounded {mode:?}");
            }
        }
    }

    #[test]
    fn arithmetic_follows_clickhouse_scales() {
        // ClickHouse: toDecimal32(2, 4) / 3 is 0.6666, truncated.
        let two = Decimal32::<4>::literal("2");
        let three = Decimal32::<0>::literal("3");
        assert_eq!(two.divide_rounded(three, Rounding::TowardZero).to_string(), "0.6666");
        assert_eq!(two.divide_rounded(three, Rounding::HalfEven).to_string(), "0.6667");
        assert_eq!(two.divide_exact(three), Err(Inexact));
        assert_eq!(d2("1.50").divide_exact(Decimal64::<0>::literal("3")).map(|d| d.to_string()), Ok("0.50".into()));
        // The product's scale is the sum of the operands'.
        let product: Decimal64<6> = d2("12.34").multiply(Decimal64::<4>::literal("0.9500"));
        assert_eq!(product.to_string(), "11.723000");
        assert_eq!(d2("12.34").times(3).to_string(), "37.02");
        assert_eq!((d2("1.10") - d2("2.25")).to_string(), "-1.15");
        assert_eq!(d2("1.10").compare(Decimal64::<4>::literal("1.1000")), Ordering::Equal);
        assert_eq!(d2("0.05").to_decimal::<i128, 38, 4>().to_string(), "0.0500");
    }

    #[test]
    fn decimal256_holds_76_digits() {
        let nines = "9".repeat(76);
        let largest = Decimal256::<0>::literal(&nines);
        assert_eq!(largest.to_string(), nines);
        assert_eq!((-largest).to_string(), format!("-{nines}"));
        assert!(Decimal256::<0>::parse(&"1".repeat(77)).is_none());
        let half = Decimal256::<0>::literal(&"4".repeat(76)) + Decimal256::<0>::literal("5");
        assert_eq!(half.to_string(), format!("{}9", "4".repeat(75)));
        let big = Decimal256::<10>::literal("123456789012345678901234567890.0123456789");
        let tenth = big.divide_rounded(Decimal256::<0>::literal("10"), Rounding::HalfEven);
        assert_eq!(tenth.to_string(), "12345678901234567890123456789.0012345679");
        assert!(big > tenth);
    }

    #[test]
    fn rationals_stay_exact_and_in_lowest_terms() {
        let third = Rational64::new(1, 3);
        assert_eq!((third + third + third).to_string(), "1");
        assert_eq!(Rational64::new(2, -4).to_string(), "-1/2");
        assert_eq!(Rational64::new(0, -7), Rational64::new(0, 1));
        assert_eq!((Rational64::new(3, 4) / Rational64::new(3, 8)).to_string(), "2");
        assert!(Rational64::new(1, 3) < Rational64::new(1, 2));
        // A bill of 100.00 in thirds: each share rounded once, at the end.
        let share = d2("100.00").to_rational::<i64, 18>() * third;
        assert_eq!(share.to_decimal_rounded::<i64, 18, 2>(Rounding::HalfEven).to_string(), "33.33");
        assert_eq!(share.to_decimal_exact::<i64, 18, 2>(), Err(Inexact));
        assert_eq!(Rational64::new(1, 4).to_decimal_exact::<i64, 18, 2>().map(|d| d.to_string()), Ok("0.25".into()));
        assert!(std::panic::catch_unwind(|| Rational16::new(1, 9999) * Rational16::new(1, 2)).is_err());
    }

    #[test]
    fn a_decimal_or_rational_with_no_precision_has_no_limit() {
        type Exact = Decimal<Big, UNBOUNDED, 2>;
        let huge = Exact::literal(&format!("{}.99", "9".repeat(100)));
        let doubled = huge.clone() + huge;
        assert_eq!(doubled.to_string(), format!("1{}.98", "9".repeat(100)));
        type Fraction = Rational<Big, UNBOUNDED>;
        let tiny = Fraction::new(1, i128::MAX);
        let product = tiny.clone() * tiny;
        assert_eq!(product.denominator(), Big::from_i128(i128::MAX).multiply(&Big::from_i128(i128::MAX)));
        // Operands stored in different sizes meet in the size of the result.
        let small = Decimal16::<2>::literal("12.50");
        let large = Decimal128::<2>::literal("1000000000000.25");
        assert_eq!((small.to_decimal::<i128, 38, 2>() + large).to_string(), "1000000000012.75");
    }

    #[test]
    fn integers_are_checked_where_they_meet_their_constraints() {
        type Percent = Integer<i8, 0, 100>;
        let half = Percent::from_i128(50);
        let sum = half.value() + half.value();
        assert_eq!(Percent::new(&sum).to_i128(), 100);
        assert!(std::panic::catch_unwind(|| Percent::new(&(sum.clone() + Big::from_i128(1)))).is_err());
        let huge = Big::from_i128(i128::MAX) * Big::from_i128(i128::MAX);
        assert_eq!(huge.divide_rounded(&Big::from_i128(i128::MAX), Rounding::HalfEven), Big::from_i128(i128::MAX));
    }

    #[test]
    fn floats_enter_and_leave_by_name() {
        assert_eq!(d2("0.10").to_float64_nearest(), 0.1);
        // 0.125 is exact in binary, so it is a true tie.
        assert_eq!(Decimal64::<2>::from_float64_rounded(0.125, Rounding::HalfEven).to_string(), "0.12");
        assert_eq!(Decimal64::<2>::from_float64_rounded(0.125, Rounding::HalfAwayFromZero).to_string(), "0.13");
        // 2.675 is a little below it in binary, so half away from zero still gives 2.67.
        assert_eq!(Decimal64::<2>::from_float64_rounded(2.675, Rounding::HalfAwayFromZero).to_string(), "2.67");
        assert_eq!(d2("-2.50").to_integer_rounded(Rounding::HalfEven), -2);
    }
}
