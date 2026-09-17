use crate::currency::{CurrencyError, Money};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct InventoryLine {
    pub product_name: String,
    pub quantity: u64,
    pub unit_value: Money,
}

impl InventoryLine {
    pub fn total_value(&self) -> Result<Money, CurrencyError> {
        self.unit_value.checked_mul_quantity(self.quantity)
    }
}
