use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (prev_bid_vol, prev_ask_vol, bid_churn, ask_churn)
static SPOOF_STATE: Mutex<[(f64, f64, f64, f64); 3]> = Mutex::new([(0.0, 0.0, 0.0, 0.0); 3]);

// ENGINE 12: Tier-0 Trade-to-Order Cancellation & Spoofing Filter
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

    let bid_vol: f64 = buys.iter().take(5).map(|l| parse_f64(l.get("size"))).sum();
    let ask_vol: f64 = sells.iter().take(5).map(|l| parse_f64(l.get("size"))).sum();

    if bid_vol <= 0.0 || ask_vol <= 0.0 {
        return;
    }

    if let Ok(mut state) = SPOOF_STATE.lock() {
        let (p_bid_v, p_ask_v, mut b_churn, mut a_churn) = state[idx];

        if p_bid_v > 0.0 && p_ask_v > 0.0 {
            // Sudden vanished liquidity ratio (Order Pulling / Cancellation)
            let bid_pulled = (p_bid_v - bid_vol).max(0.0) / p_bid_v;
            let ask_pulled = (p_ask_v - ask_vol).max(0.0) / p_ask_v;

            b_churn = (b_churn * 0.75) + (bid_pulled * 0.25);
            a_churn = (a_churn * 0.75) + (ask_pulled * 0.25);

            let spoof_alert = if b_churn > 0.35 && b_churn > a_churn {
                "🎭 [FAKE_BID_SPOOF_PULLED]"
            } else if a_churn > 0.35 && a_churn > b_churn {
                "🎭 [FAKE_ASK_SPOOF_PULLED]"
            } else {
                "✅ [GENUINE_FIRM_ORDERS]"
            };

            println!(
                "👻 [E12:{}] BID_CHURN:{:.1}% | ASK_CHURN:{:.1}% -> {}",
                symbol, b_churn * 100.0, a_churn * 100.0, spoof_alert
            );
        }

        state[idx] = (bid_vol, ask_vol, b_churn, a_churn);
    }
}

fn parse_f64(val: Option<&Value>) -> f64 {
    match val {
        Some(v) => v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0),
        None => 0.0,
    }
}
