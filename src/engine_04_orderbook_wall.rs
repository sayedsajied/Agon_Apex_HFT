use serde_json::Value;

// ENGINE 04: Tier-0 L2 Orderbook Depth & Liquidity Wall Scanner
pub fn process_tick(parsed: &Value) {
    if parsed.get("type").and_then(|t| t.as_str()) != Some("l2_orderbook") {
        return;
    }

    let symbol = match parsed.get("symbol").and_then(|s| s.as_str()) {
        Some(s) => s,
        None => return,
    };

    let buy_book = parsed.get("buy").and_then(|b| b.as_array());
    let sell_book = parsed.get("sell").and_then(|s| s.as_array());

    if let (Some(buys), Some(sells)) = (buy_book, sell_book) {
        let mut bid_depth_vol = 0.0;
        let mut ask_depth_vol = 0.0;
        let mut max_bid_wall = (0.0, 0.0); // (Price, Size)
        let mut max_ask_wall = (0.0, 0.0); // (Price, Size)

        // टॉप 5 Buy Levels की गहराई और सबसे बड़ी दीवार (Wall) स्कैन करना
        for level in buys.iter().take(5) {
            let price = parse_val(level.get("limit_price"));
            let size = parse_val(level.get("size"));
            bid_depth_vol += size;
            if size > max_bid_wall.1 {
                max_bid_wall = (price, size);
            }
        }

        // टॉप 5 Sell Levels की गहराई और सबसे बड़ी दीवार (Wall) स्कैन करना
        for level in sells.iter().take(5) {
            let price = parse_val(level.get("limit_price"));
            let size = parse_val(level.get("size"));
            ask_depth_vol += size;
            if size > max_ask_wall.1 {
                max_ask_wall = (price, size);
            }
        }

        let total_depth = bid_depth_vol + ask_depth_vol;
        if total_depth > 0.0 {
            let depth_ratio = (bid_depth_vol - ask_depth_vol) / total_depth;

            let wall_alert = if depth_ratio > 0.50 {
                "🧱 [MASSIVE_BID_WALL_SUPPORT]"
            } else if depth_ratio < -0.50 {
                "🚧 [MASSIVE_ASK_WALL_RESISTANCE]"
            } else {
                "⚖️ [BALANCED_BOOK]"
            };

            println!(
                "🛡️ [E04:{}] L2_IMB:{:+.2} | MAX_BID:{}@{:.1} | MAX_ASK:{}@{:.1} -> {}",
                symbol, depth_ratio, max_bid_wall.1, max_bid_wall.0, max_ask_wall.1, max_ask_wall.0, wall_alert
            );
        }
    }
}

// JSON से नंबर और स्ट्रिंग दोनों को सुपरफास्ट तरीके से f64 में बदलने का हेल्पर
fn parse_val(val: Option<&Value>) -> f64 {
    match val {
        Some(v) => v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0),
        None => 0.0,
    }
}
