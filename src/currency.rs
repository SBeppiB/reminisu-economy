use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub enum Currency {
    Ufc,
    Rimund,
    Yeria,
    Akenako,
    Komunry,
    Cuneragen,
    FrontierDollar,
    MarilciaStert,
}

impl Currency {
    pub fn code(self) -> &'static str {
        match self {
            Currency::Ufc => "UFC",
            Currency::Rimund => "RIM",
            Currency::Yeria => "YER",
            Currency::Akenako => "AKN",
            Currency::Komunry => "KOM",
            Currency::Cuneragen => "CUN",
            Currency::FrontierDollar => "FD",
            Currency::MarilciaStert => "MST",
        }
    }

    pub fn all() -> &'static [Currency] {
        &[
            Currency::Ufc,
            Currency::Rimund,
            Currency::Yeria,
            Currency::Akenako,
            Currency::Komunry,
            Currency::Cuneragen,
            Currency::FrontierDollar,
            Currency::MarilciaStert,
        ]
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RationalRate {
    numerator: i128,
    denominator: i128,
}

impl RationalRate {
    pub fn new(numerator: i128, denominator: i128) -> Result<Self, CurrencyError> {
        if numerator <= 0 || denominator <= 0 {
            return Err(CurrencyError::InvalidExchangeRate);
        }

        let divisor = gcd(numerator, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    pub fn convert(self, amount: Money, to_currency: Currency) -> Result<Money, CurrencyError> {
        let value = checked_div_round(
            amount
                .minor_units
                .checked_mul(self.numerator)
                .ok_or(CurrencyError::Overflow)?,
            self.denominator,
        )?;
        Ok(Money::from_minor(to_currency, value))
    }
}

/// Money is stored in integer minor units. The MVP treats one displayed unit as
/// 100 minor units for every currency, avoiding long-run floating point drift.
#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(deny_unknown_fields)]
pub struct Money {
    currency: Currency,
    #[serde(deserialize_with = "deserialize_minor")]
    minor_units: i128,
}

impl Money {
    pub const MINOR_PER_UNIT: i128 = 100;

    pub fn zero(currency: Currency) -> Self {
        Self {
            currency,
            minor_units: 0,
        }
    }

    pub fn from_units(currency: Currency, units: i128) -> Self {
        Self::checked_from_units(currency, units).expect("money units overflow")
    }

    pub fn checked_from_units(currency: Currency, units: i128) -> Result<Self, CurrencyError> {
        Ok(Self::from_minor(
            currency,
            units
                .checked_mul(Self::MINOR_PER_UNIT)
                .ok_or(CurrencyError::Overflow)?,
        ))
    }

    pub fn ratio(self, numerator: i128, denominator: i128) -> Result<Self, CurrencyError> {
        if denominator <= 0 {
            return Err(CurrencyError::InvalidExchangeRate);
        }
        // Split the quotient to avoid overflowing when the final result fits.
        let whole = (self.minor_units / denominator)
            .checked_mul(numerator)
            .ok_or(CurrencyError::Overflow)?;
        let remainder = (self.minor_units % denominator)
            .checked_mul(numerator)
            .ok_or(CurrencyError::Overflow)?
            / denominator;
        Ok(Self::from_minor(
            self.currency,
            whole
                .checked_add(remainder)
                .ok_or(CurrencyError::Overflow)?,
        ))
    }

    pub fn from_minor(currency: Currency, minor_units: i128) -> Self {
        Self {
            currency,
            minor_units,
        }
    }

    pub fn currency(self) -> Currency {
        self.currency
    }

    pub fn minor_units(self) -> i128 {
        self.minor_units
    }

    pub fn checked_mul_quantity(self, quantity: u64) -> Result<Self, CurrencyError> {
        let minor_units = self
            .minor_units
            .checked_mul(i128::from(quantity))
            .ok_or(CurrencyError::Overflow)?;
        Ok(Self::from_minor(self.currency, minor_units))
    }

    pub fn checked_add(self, other: Self) -> Result<Self, CurrencyError> {
        self.ensure_same_currency(other)?;
        Ok(Self::from_minor(
            self.currency,
            self.minor_units
                .checked_add(other.minor_units)
                .ok_or(CurrencyError::Overflow)?,
        ))
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, CurrencyError> {
        self.ensure_same_currency(other)?;
        Ok(Self::from_minor(
            self.currency,
            self.minor_units
                .checked_sub(other.minor_units)
                .ok_or(CurrencyError::Overflow)?,
        ))
    }

    fn ensure_same_currency(self, other: Self) -> Result<(), CurrencyError> {
        if self.currency == other.currency {
            Ok(())
        } else {
            Err(CurrencyError::CurrencyMismatch {
                left: self.currency,
                right: other.currency,
            })
        }
    }
}

impl Add for Money {
    type Output = Money;

    fn add(self, rhs: Self) -> Self::Output {
        self.checked_add(rhs)
            .expect("money currency mismatch or overflow")
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Money {
    type Output = Money;

    fn sub(self, rhs: Self) -> Self::Output {
        self.checked_sub(rhs)
            .expect("money currency mismatch or overflow")
    }
}

impl SubAssign for Money {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl fmt::Display for Money {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.minor_units < 0 { "-" } else { "" };
        let absolute = self.minor_units.unsigned_abs();
        let units = absolute / Money::MINOR_PER_UNIT as u128;
        let minor = absolute % Money::MINOR_PER_UNIT as u128;
        write!(
            formatter,
            "{sign}{}.{:02} {}",
            format_integer(units),
            minor,
            self.currency.code()
        )
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ExchangeTable {
    reference_currency: Currency,
    rates_to_reference_minor: Vec<(Currency, RationalRate)>,
}

impl ExchangeTable {
    pub fn initial() -> Self {
        let mut rates = Vec::new();
        for currency in Currency::all() {
            let rate = match currency {
                Currency::Ufc => RationalRate::new(1, 1),
                Currency::Rimund => RationalRate::new(1, 200),
                Currency::Yeria => RationalRate::new(1, 500),
                Currency::Akenako => RationalRate::new(1, 15),
                Currency::Komunry => RationalRate::new(1, 70),
                Currency::Cuneragen => RationalRate::new(1, 4500),
                Currency::FrontierDollar => RationalRate::new(100, 365),
                Currency::MarilciaStert => RationalRate::new(565, 100),
            }
            .expect("hard-coded exchange rates must be valid");
            rates.push((*currency, rate));
        }

        Self {
            reference_currency: Currency::Ufc,
            rates_to_reference_minor: rates,
        }
    }

    pub fn reference_currency(&self) -> Currency {
        self.reference_currency
    }

    pub fn convert(&self, amount: Money, to_currency: Currency) -> Result<Money, CurrencyError> {
        let reference = self.to_reference(amount)?;
        self.convert_reference_to(reference, to_currency)
    }

    pub fn to_reference(&self, amount: Money) -> Result<Money, CurrencyError> {
        let rate = self.rate_for(amount.currency())?;
        rate.convert(amount, self.reference_currency)
    }

    pub fn convert_reference_to(
        &self,
        reference_amount: Money,
        to_currency: Currency,
    ) -> Result<Money, CurrencyError> {
        if reference_amount.currency() != self.reference_currency {
            return Err(CurrencyError::CurrencyMismatch {
                left: reference_amount.currency(),
                right: self.reference_currency,
            });
        }
        let rate = self.rate_for(to_currency)?;
        let inverse = RationalRate::new(rate.denominator, rate.numerator)?;
        inverse.convert(reference_amount, to_currency)
    }

    fn rate_for(&self, currency: Currency) -> Result<RationalRate, CurrencyError> {
        self.rates_to_reference_minor
            .iter()
            .find_map(|(candidate, rate)| (*candidate == currency).then_some(*rate))
            .ok_or(CurrencyError::MissingExchangeRate(currency))
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum CurrencyError {
    CurrencyMismatch { left: Currency, right: Currency },
    InvalidExchangeRate,
    MissingExchangeRate(Currency),
    Overflow,
}

impl fmt::Display for CurrencyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CurrencyError::CurrencyMismatch { left, right } => {
                write!(
                    formatter,
                    "currency mismatch: {} vs {}",
                    left.code(),
                    right.code()
                )
            }
            CurrencyError::InvalidExchangeRate => write!(formatter, "invalid exchange rate"),
            CurrencyError::MissingExchangeRate(currency) => {
                write!(formatter, "missing exchange rate for {}", currency.code())
            }
            CurrencyError::Overflow => write!(formatter, "currency arithmetic overflow"),
        }
    }
}

impl std::error::Error for CurrencyError {}

fn checked_div_round(numerator: i128, denominator: i128) -> Result<i128, CurrencyError> {
    if denominator <= 0 {
        return Err(CurrencyError::InvalidExchangeRate);
    }
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    let threshold = (denominator / 2 + denominator % 2) as u128;
    if remainder.unsigned_abs() >= threshold {
        quotient
            .checked_add(if numerator < 0 { -1 } else { 1 })
            .ok_or(CurrencyError::Overflow)
    } else {
        Ok(quotient)
    }
}
fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

fn format_integer(value: u128) -> String {
    let digits = value.to_string();
    let mut formatted = String::new();
    for (index, character) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(character);
    }
    formatted.chars().rev().collect()
}

// TOML integers are i64; decimal strings allow larger exact configuration values.
fn deserialize_minor<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<i128, D::Error> {
    let value = <serde_json::Value as serde::Deserialize>::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(number) => {
            number.to_string().parse().map_err(serde::de::Error::custom)
        }
        serde_json::Value::String(text) => text.parse().map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom(
            "expected integer minor units or decimal integer string",
        )),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_known_rates_to_reference() {
        let table = ExchangeTable::initial();

        assert_eq!(
            table
                .to_reference(Money::from_units(Currency::Rimund, 200))
                .unwrap(),
            Money::from_units(Currency::Ufc, 1)
        );
        assert_eq!(
            table
                .to_reference(Money::from_units(Currency::Yeria, 500))
                .unwrap(),
            Money::from_units(Currency::Ufc, 1)
        );
        assert_eq!(
            table
                .to_reference(Money::from_units(Currency::MarilciaStert, 1))
                .unwrap(),
            Money::from_minor(Currency::Ufc, 565)
        );
    }

    #[test]
    fn converts_frontier_dollar_exact_minor_units() {
        let table = ExchangeTable::initial();

        assert_eq!(
            table
                .to_reference(Money::from_minor(Currency::FrontierDollar, 365))
                .unwrap(),
            Money::from_minor(Currency::Ufc, 100)
        );
    }

    #[test]
    fn money_arithmetic_preserves_currency() {
        let left = Money::from_units(Currency::Ufc, 5);
        let right = Money::from_units(Currency::Ufc, 3);

        assert_eq!(left + right, Money::from_units(Currency::Ufc, 8));
        assert_eq!(left - right, Money::from_units(Currency::Ufc, 2));
    }

    #[test]
    fn rejects_cross_currency_arithmetic() {
        let result =
            Money::from_units(Currency::Ufc, 1).checked_add(Money::from_units(Currency::Rimund, 1));

        assert!(matches!(
            result,
            Err(CurrencyError::CurrencyMismatch { .. })
        ));
    }
}
