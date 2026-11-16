use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): rolling time-decayed pressure score
static DECAY_STATE: Mutex<[f64; 3]> = Mutex::new([0.0; 3]);

// ENGINE 14: Tier-0 Time-Weighted Orderbook Pressure Decay
pub fn process_tick(parsed: &Value) {
    if parsed.get("type").and_then(|t| t.as_str()) != Some("l2_orderbook") {
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

    let buys = match parsed.get("buy").and_then(|b| b.as_array()) {
        Some(b) if !b.is_empty() => b,
        _ => return,
    };
    let sells = match parsed.get("sell").and_then(|s| s.as_array()) {
        Some(s) if !s.is_empty() => s,
        _ => return,
    };

    let top_bid_sz = parse_f64(buys[0].get("size"));
    let top_ask_sz = parse_f64(sells[0].get("size"));
    let total_sz = top_bid_sz + top_ask_sz;

    if total_sz <= 0.0 {
        return;
    }

    let instant_pressure = (top_bid_sz - top_ask_sz) / total_sz;

    if let Ok(mut state) = DECAY_STATE.lock() {
        // Half-life decay weighting: old pressure fades exponentially
        let decay_factor = 0.80;
        let weighted_pressure = (state[idx] * decay_factor) + (instant_pressure * (1.0 - decay_factor));
        state[idx] = weighted_pressure;

        let pressure_zone = if weighted_pressure > 0.30 {
            "📈 [DECAY_WEIGHTED_BULL_BIAS]"
        } else if weighted_pressure < -0.30 {
            "📉 [DECAY_WEIGHTED_BEAR_BIAS]"
        } else {
            "⚖️ [DECAY_NEUTRAL_ZONE]"
        };

        println!(
            "⏳ [E14:{}] INST_PR:{:+.2} | TIME_DECAY_PR:{:+.3} -> {}",
            symbol, instant_pressure, weighted_pressure, pressure_zone
        );
    }
}

fn parse_f64(val: Option<&Value>) -> f64 {
    match val {
        Some(v) => v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0),
        None => 0.0,
    }
}
