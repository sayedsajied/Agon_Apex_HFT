use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): rolling smoothed micro-price deviation (bps)
static DEV_STATE: Mutex<[f64; 3]> = Mutex::new([0.0; 3]);

// ENGINE 13: Tier-0 Micro-Price Deviation & Fair-Value Skew Calculator
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

    let total_sz = bid_sz + ask_sz;
    if bid <= 0.0 || ask <= 0.0 || total_sz <= 0.0 {
        return;
    }

    let mid_price = (bid + ask) / 2.0;
    let spread = (ask - bid).max(1e-8);
    let micro_price = ((bid * ask_sz) + (ask * bid_sz)) / total_sz;

    // Deviation normalized by half-spread (-1.0 to +1.0) and in Basis Points (bps)
    let raw_dev = micro_price - mid_price;
    let dev_bps = (raw_dev / mid_price) * 10000.0;
    let skew_pct = (raw_dev / (spread / 2.0)) * 100.0;

    if let Ok(mut state) = DEV_STATE.lock() {
        let smooth_bps = (state[idx] * 0.75) + (dev_bps * 0.25);
        state[idx] = smooth_bps;

        let lead_signal = if skew_pct > 45.0 {
            "🎯 [MICRO_PULL_UPWARD]"
        } else if skew_pct < -45.0 {
            "🎯 [MICRO_PULL_DOWNWARD]"
        } else {
            "⚖️ [CENTERED_FAIR_VALUE]"
        };

        println!(
            "🧭 [E13:{}] DEV:{:+.4} ({:+.2}bps) | SKEW:{:+.1}% -> {}",
            symbol, raw_dev, smooth_bps, skew_pct, lead_signal
        );
    }
}
