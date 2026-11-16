use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_mid_price, rolling_lambda)
static KYLE_STATE: Mutex<[(f64, f64); 3]> = Mutex::new([(0.0, 0.0); 3]);

// ENGINE 07: Tier-0 Kyle's Lambda (Market Impact & Liquidity Fragility)
pub fn process_tick(parsed: &Value) {
    if parsed.get("type").and_then(|t| t.as_str()) != Some("v2/ticker") {
        return;
    }

    let symbol = match parsed.get("symbol").and_then(|s| s.as_str()) {
        Some(s) => s,
        None => return,
    };

    let idx = if symbol.contains("BTC") {
        0
    } else if symbol.contains("ETH") {
        1
    } else if symbol.contains("SOL") {
        2
    } else {
        return;
    };

    let quotes = parsed.get("quotes");
    let bid = quotes
        .and_then(|q| q.get("best_bid"))
        .and_then(|b| b.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let ask = quotes
        .and_then(|q| q.get("best_ask"))
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let bid_sz = quotes
        .and_then(|q| q.get("bid_size"))
        .and_then(|b| b.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let ask_sz = quotes
        .and_then(|q| q.get("ask_size"))
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    if bid <= 0.0 || ask <= 0.0 {
        return;
    }

    let mid = (bid + ask) / 2.0;
    let net_order_flow = (bid_sz - ask_sz).abs().max(1.0);

    if let Ok(mut state) = KYLE_STATE.lock() {
        let (last_mid, prev_lambda) = state[idx];

        if last_mid > 0.0 {
            let price_impact_bps = ((mid - last_mid).abs() / last_mid) * 10000.0;
            // Kyle's Lambda = Price Change (BPS) per unit of Order Imbalance
            let raw_lambda = (price_impact_bps / net_order_flow) * 100.0;
            let smooth_lambda = (prev_lambda * 0.80) + (raw_lambda * 0.20);

            let impact_state = if smooth_lambda > 0.50 {
                "⚠️ [THIN_BOOK_HIGH_SLIPPAGE]"
            } else if smooth_lambda > 0.15 {
                "⚡ [FRAGILE_LIQUIDITY]"
            } else {
                "🛡️ [DEEP_ABSORPTION]"
            };

            println!(
                "📐 [E07:{}] KYLE_λ:{:.4} | IMPACT:{:.2}bps -> {}",
                symbol, smooth_lambda, price_impact_bps, impact_state
            );

            state[idx] = (mid, smooth_lambda);
        } else {
            state[idx] = (mid, 0.0);
        }
    }
}
