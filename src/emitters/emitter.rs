use crate::ast::ast::Quantity;

#[cfg(feature = "runtime")]
use async_trait::async_trait;

pub trait CanEmit<T> {
    fn emit(&self, item: &T) -> String;
}

#[cfg(feature = "runtime")]
#[async_trait]
pub trait CanEmitAI<T>: Send + Sync {
    async fn emit_ai(&self, item: &T) -> Result<T, String>;
}

pub fn quoted_string(output: &mut String, s: &str) {
    output.push('"');
    output.push_str(s);
    output.push('"');
}

pub fn format_quantity(quantity: &Quantity) -> String {
    match &quantity.unit {
        Some(unit) => format!("{}{}", quantity.amount, unit),
        None => format!("{}", quantity.amount),
    }
}
