use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (bid_p, bid_q, bid_refills, ask_p, ask_q, ask_refills)
static ICEBERG_STATE: Mutex<[(f64, f64, f64, f64, f64, f64); 3]> =
    Mutex::new([(0.0, 0.0, 0.0, 0.0, 0.0, 0.0); 3]);

// ENGINE 10: Tier-0 Iceberg Order & Hidden Liquidity Absorption Sniffer
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

    let bid_p = parse_f64(buys[0].get("limit_price"));
    let bid_q = parse_f64(buys[0].get("size"));
    let ask_p = parse_f64(sells[0].get("limit_price"));
    let ask_q = parse_f64(sells[0].get("size"));

    if bid_p <= 0.0 || ask_p <= 0.0 {
        return;
    }

    if let Ok(mut state) = ICEBERG_STATE.lock() {
        let (pb_p, pb_q, mut b_refill, pa_p, pa_q, mut a_refill) = state[idx];

        // Bid Iceberg Detection: Price pinned at exact level while queue refills/absorbs
        if (bid_p - pb_p).abs() < 1e-5 {
            if bid_q != pb_q {
                b_refill = (b_refill * 0.85) + 1.0;
            }
        } else {
            b_refill = 0.0;
        }

        // Ask Iceberg Detection: Price pinned at exact level while queue refills/absorbs
        if (ask_p - pa_p).abs() < 1e-5 {
            if ask_q != pa_q {
                a_refill = (a_refill * 0.85) + 1.0;
            }
        } else {
            a_refill = 0.0;
        }

        let iceberg_signal = if b_refill > 3.2 && b_refill > a_refill {
            "🧊 [HIDDEN_BID_ICEBERG_ABSORPTION]"
        } else if a_refill > 3.2 && a_refill > b_refill {
            "🧊 [HIDDEN_ASK_ICEBERG_CEILING]"
        } else {
            "👁️ [VISIBLE_LIQUIDITY_ONLY]"
        };

        println!(
            "🕵️ [E10:{}] BID_REFILL:{:.2}@{:.1} | ASK_REFILL:{:.2}@{:.1} -> {}",
            symbol, b_refill, bid_p, a_refill, ask_p, iceberg_signal
        );

        state[idx] = (bid_p, bid_q, b_refill, ask_p, ask_q, a_refill);
    }
}

fn parse_f64(val: Option<&Value>) -> f64 {
    match val {
        Some(v) => v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0),
        None => 0.0,
    }
}
