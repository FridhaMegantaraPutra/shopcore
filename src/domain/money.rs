//! Money value objects. Amounts are integer minor units (cents), never floats.

use super::DomainError;

/// A strictly positive amount of money in cents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Amount(i64);

impl Amount {
    pub fn new(cents: i64) -> Result<Self, DomainError> {
        if cents <= 0 {
            return Err(DomainError::Validation("amount_cents must be a positive integer".into()));
        }
        Ok(Self(cents))
    }

    pub fn cents(self) -> i64 {
        self.0
    }
}

/// ISO 4217 currency code, e.g. `IDR`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Currency(String);

impl Currency {
    pub const DEFAULT: &'static str = "IDR";

    pub fn parse(code: Option<&str>) -> Result<Self, DomainError> {
        let c = code.unwrap_or(Self::DEFAULT).trim().to_ascii_uppercase();
        if c.len() != 3 || !c.chars().all(|ch| ch.is_ascii_alphabetic()) {
            return Err(DomainError::Validation("currency must be a 3-letter ISO code".into()));
        }
        Ok(Self(c))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_must_be_positive() {
        assert_eq!(Amount::new(1).unwrap().cents(), 1);
        assert!(Amount::new(0).is_err());
        assert!(Amount::new(-5).is_err());
    }

    #[test]
    fn currency_is_normalised_and_validated() {
        assert_eq!(Currency::parse(None).unwrap().as_str(), "IDR");
        assert_eq!(Currency::parse(Some(" usd ")).unwrap().as_str(), "USD");
        assert!(Currency::parse(Some("US")).is_err());
        assert!(Currency::parse(Some("U5D")).is_err());
    }
}
