//! API list prices of Claude models, to estimate what a conversation would
//! cost billed per token. Subscription plans do not bill per token, so the
//! app labels the figure as an estimate.

/// US dollars per million tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
}

impl Price {
    const fn new(input: f64, output: f64, cache_read: f64) -> Self {
        Self {
            input,
            output,
            cache_read,
        }
    }

    /// Five-minute cache writes cost 1.25 times regular input.
    pub fn cache_write(&self) -> f64 {
        self.input * 1.25
    }
}

/// Model id prefixes, most specific first: `claude-opus-5-5` must win over
/// `claude-opus-5`. Dated ids such as `claude-sonnet-4-5-20250929` match
/// their prefix.
const PRICES: &[(&str, Price)] = &[
    ("claude-fable-5-2", Price::new(8.0, 40.0, 0.20)),
    ("claude-mythos-5-2", Price::new(8.0, 40.0, 0.20)),
    ("claude-fable-5-1", Price::new(10.0, 50.0, 0.25)),
    ("claude-mythos-5-1", Price::new(10.0, 50.0, 0.25)),
    ("claude-fable-5", Price::new(10.0, 50.0, 1.0)),
    ("claude-mythos-5", Price::new(10.0, 50.0, 1.0)),
    ("claude-opus-5-5", Price::new(4.0, 20.0, 0.20)),
    ("claude-opus-5", Price::new(5.0, 25.0, 0.50)),
    ("claude-opus-4-8", Price::new(5.0, 25.0, 0.50)),
    ("claude-opus-4-7", Price::new(5.0, 25.0, 0.50)),
    ("claude-opus-4-6", Price::new(5.0, 25.0, 0.50)),
    ("claude-opus-4-5", Price::new(5.0, 25.0, 0.50)),
    ("claude-opus-4", Price::new(15.0, 75.0, 1.50)),
    ("claude-sonnet-5-5", Price::new(2.0, 10.0, 0.10)),
    ("claude-sonnet-5", Price::new(2.0, 10.0, 0.20)),
    ("claude-sonnet-4", Price::new(3.0, 15.0, 0.30)),
    ("claude-3-7-sonnet", Price::new(3.0, 15.0, 0.30)),
    ("claude-haiku-5-5", Price::new(0.10, 0.50, 0.01)),
    ("claude-haiku-4-5", Price::new(1.0, 5.0, 0.10)),
    ("claude-3-5-haiku", Price::new(0.80, 4.0, 0.08)),
];

/// The price of `model`, or `None` for a model this table does not know.
pub fn price(model: &str) -> Option<Price> {
    PRICES
        .iter()
        .find(|(prefix, _)| model.starts_with(prefix))
        .map(|(_, price)| *price)
}

/// Dollars for one request's tokens.
pub fn cost(price: Price, input: u64, output: u64, cache_read: u64, cache_creation: u64) -> f64 {
    const MILLION: f64 = 1_000_000.0;
    (input as f64 * price.input
        + output as f64 * price.output
        + cache_read as f64 * price.cache_read
        + cache_creation as f64 * price.cache_write())
        / MILLION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_most_specific_prefix_wins() {
        assert_eq!(price("claude-opus-5-5"), Some(Price::new(4.0, 20.0, 0.20)));
        assert_eq!(price("claude-opus-5"), Some(Price::new(5.0, 25.0, 0.50)));
        assert_eq!(
            price("claude-opus-4-1-20250805"),
            Some(Price::new(15.0, 75.0, 1.50))
        );
        assert_eq!(
            price("claude-sonnet-4-5-20250929"),
            Some(Price::new(3.0, 15.0, 0.30))
        );
        assert_eq!(price("gpt-5"), None);
    }

    #[test]
    fn cost_adds_every_kind_of_token() {
        let price = Price::new(4.0, 20.0, 0.20);
        let dollars = cost(price, 1_000_000, 100_000, 2_000_000, 400_000);
        // 4 + 2 + 0.4 + 2
        assert!((dollars - 8.4).abs() < 1e-9);
    }
}
