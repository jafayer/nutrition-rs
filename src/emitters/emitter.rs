use crate::ast::ast::Quantity;

pub trait CanEmit<T> {
    fn emit(&self, item: &T) -> String;
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