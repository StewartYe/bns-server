/// Calculate discount percentage: (1.26 * previous_price - price) / (1.26 * previous_price)
/// Returns 0.0 if price >= 1.26 * previous_price
/// Result is rounded to 3 decimal places
pub fn calculate_discount(price: u64, previous_price: u64) -> f64 {
    if previous_price == 0 {
        return 1.0;
    }

    let previous_price_f64 = previous_price as f64;
    let benchmark_price = previous_price_f64 * 1.26;
    let price_f64 = price as f64;

    if price_f64 >= benchmark_price {
        return 0.0;
    }

    let discount = (benchmark_price - price_f64) / benchmark_price;
    (discount * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_discount() {
        // Example from requirements: (1.26*100-50)/(1.26*100) = 0.603
        assert_eq!(calculate_discount(50, 100), 0.603);

        // No discount case
        assert_eq!(calculate_discount(126, 100), 0.0);
        assert_eq!(calculate_discount(130, 100), 0.0);

        // Full discount (price 0)
        assert_eq!(calculate_discount(0, 100), 1.0);

        // Invalid previous price
        assert_eq!(calculate_discount(100, 0), 1.0);
    }
}
