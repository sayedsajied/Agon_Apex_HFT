use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (prev_bid_p, prev_bid_q, prev_ask_p, prev_ask_q, rolling_ofi)
static OFI_STATE: Mutex<[(f64, f64, f64, f64, f64); 3]> = Mutex::new([(0.0, 0.0, 0.0, 0.0, 0.0); 3]);

// ENGINE 09: Tier-0 Order Flow Imbalance (Cont-Kukanov-Stoikov OFI Dynamics)
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
    // Multi-level top 3 aggregated queue size for institutional depth
    let bid_q: f64 = buys.iter().take(3).map(|l| parse_f64(l.get("size"))).sum();

    let ask_p = parse_f64(sells[0].get("limit_price"));
    let ask_q: f64 = sells.iter().take(3).map(|l| parse_f64(l.get("size"))).sum();

    if bid_p <= 0.0 || ask_p <= 0.0 {
        return;
    }

    if let Ok(mut state) = OFI_STATE.lock() {
        let (pb_p, pb_q, pa_p, pa_q, prev_ofi) = state[idx];

        if pb_p > 0.0 && pa_p > 0.0 {
            // Bid-side flow contribution (e_b)
            let e_bid = if bid_p > pb_p {
                bid_q
            } else if bid_p < pb_p {
                -pb_q
            } else {
                bid_q - pb_q
            };

            // Ask-side flow contribution (e_a)
            let e_ask = if ask_p < pa_p {
                ask_q
            } else if ask_p > pa_p {
                -pa_q
            } else {
                ask_q - pa_q
            };

            let instant_ofi = e_bid - e_ask;
            let smooth_ofi = (prev_ofi * 0.80) + (instant_ofi * 0.20);

            let ofi_signal = if smooth_ofi > 150.0 {
                "🟢 [AGGRESSIVE_BID_INFLOW]"
            } else if smooth_ofi < -150.0 {
                "🔴 [AGGRESSIVE_ASK_SUPPLY]"
            } else {
                "⚖️ [NEUTRAL_OFI]"
            };

            println!(
                "🌊 [E09:{}] INST_OFI:{:+.1} | ROLLING_OFI:{:+.1} -> {}",
                symbol, instant_ofi, smooth_ofi, ofi_signal
            );

            state[idx] = (bid_p, bid_q, ask_p, ask_q, smooth_ofi);
        } else {
            state[idx] = (bid_p, bid_q, ask_p, ask_q, 0.0);
        }
    }
}

fn parse_f64(val: Option<&Value>) -> f64 {
    match val {
        Some(v) => v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0),
        None => 0.0,
    }
}
