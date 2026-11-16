use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (prev_best_bid, prev_bid_vol, prev_best_ask, prev_ask_vol)
static SWEEP_STATE: Mutex<[(f64, f64, f64, f64); 3]> = Mutex::new([(0.0, 0.0, 0.0, 0.0); 3]);

// ENGINE 11: Tier-0 Whale Book Sweep & Aggressive Market Order Detector
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

    let best_bid = parse_f64(buys[0].get("limit_price"));
    let best_ask = parse_f64(sells[0].get("limit_price"));
    let top3_bid_vol: f64 = buys.iter().take(3).map(|l| parse_f64(l.get("size"))).sum();
    let top3_ask_vol: f64 = sells.iter().take(3).map(|l| parse_f64(l.get("size"))).sum();

    if best_bid <= 0.0 || best_ask <= 0.0 {
        return;
    }

    if let Ok(mut state) = SWEEP_STATE.lock() {
        let (p_bid, p_bid_v, p_ask, p_ask_v) = state[idx];

        if p_bid > 0.0 && p_ask > 0.0 {
            let ask_jump_bps = ((best_ask - p_ask) / p_ask) * 10000.0;
            let bid_drop_bps = ((p_bid - best_bid) / p_bid) * 10000.0;

            // Liquidity consumed during multi-level book sweep
            let ask_swept_vol = if best_ask > p_ask { p_ask_v } else { (p_ask_v - top3_ask_vol).max(0.0) };
            let bid_swept_vol = if best_bid < p_bid { p_bid_v } else { (p_bid_v - top3_bid_vol).max(0.0) };

            let sweep_alert = if ask_jump_bps > 1.2 && ask_swept_vol > 50.0 {
                "🐋 [WHALE_BUY_SWEEP_BURST]"
            } else if bid_drop_bps > 1.2 && bid_swept_vol > 50.0 {
                "🐋 [WHALE_SELL_DUMP_SWEEP]"
            } else {
                "🐟 [NO_BOOK_SWEEP]"
            };

            println!(
                "🌊 [E11:{}] ASK_SWEPT:{:.0} (+{:.2}bps) | BID_SWEPT:{:.0} (-{:.2}bps) -> {}",
                symbol, ask_swept_vol, ask_jump_bps.max(0.0), bid_swept_vol, bid_drop_bps.max(0.0), sweep_alert
            );
        }

        state[idx] = (best_bid, top3_bid_vol, best_ask, top3_ask_vol);
    }
}

fn parse_f64(val: Option<&Value>) -> f64 {
    match val {
        Some(v) => v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0),
        None => 0.0,
    }
}
