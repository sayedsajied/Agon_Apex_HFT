use serde_json::Value;

// ENGINE 02: Tier-0 Micro-Price Spread & L1 Bid-Ask Imbalance Scalper
pub fn process_tick(parsed: &Value) {
    if parsed.get("type").and_then(|t| t.as_str()) != Some("v2/ticker") {
        return;
    }

    let symbol = match parsed.get("symbol").and_then(|s| s.as_str()) {
        Some(s) => s,
        None => return,
    };

    let quotes = parsed.get("quotes");

    let bid_str = quotes
        .and_then(|q| q.get("best_bid"))
        .and_then(|b| b.as_str())
        .or_else(|| parsed.get("best_bid").and_then(|b| b.as_str()));

    let ask_str = quotes
        .and_then(|q| q.get("best_ask"))
        .and_then(|a| a.as_str())
        .or_else(|| parsed.get("best_ask").and_then(|a| a.as_str()));

    let bid_sz_str = quotes
        .and_then(|q| q.get("bid_size"))
        .and_then(|bs| bs.as_str())
        .unwrap_or("0");

    let ask_sz_str = quotes
        .and_then(|q| q.get("ask_size"))
        .and_then(|as_str| as_str.as_str())
        .unwrap_or("0");

    if let (Some(b_s), Some(a_s)) = (bid_str, ask_str) {
        let bid: f64 = b_s.parse().unwrap_or(0.0);
        let ask: f64 = a_s.parse().unwrap_or(0.0);
        let bid_size: f64 = bid_sz_str.parse().unwrap_or(0.0);
        let ask_size: f64 = ask_sz_str.parse().unwrap_or(0.0);

        if bid > 0.0 && ask > 0.0 {
            let spread = ask - bid;
            let mid_price = (ask + bid) / 2.0;
            let total_qty = bid_size + ask_size;

            // Tier-0 Stoikov Micro-Price & L1 Orderbook Imbalance Ratio (-1.0 to +1.0)
            let micro_price = if total_qty > 0.0 {
                (bid * ask_size + ask * bid_size) / total_qty
            } else {
                mid_price
            };

            let imbalance = if total_qty > 0.0 {
                (bid_size - ask_size) / total_qty
            } else {
                0.0
            };

            let state = if imbalance > 0.45 {
                "🔥 [SCALP_LONG_BIAS]"
            } else if imbalance < -0.45 {
                "❄️ [SCALP_SHORT_BIAS]"
            } else {
                "⚖️ [EQUILIBRIUM]"
            };

            println!(
                "⚡ [E02:{}] MID:{:.2} | MICRO:{:.2} | SPRD:{:.2} | IMB:{:+.2} -> {}",
                symbol, mid_price, micro_price, spread, imbalance, state
            );
        }
    }
}
