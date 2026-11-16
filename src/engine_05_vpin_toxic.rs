use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, buy_vol_bucket, sell_vol_bucket)
static VPIN_STATE: Mutex<[(f64, f64, f64); 3]> = Mutex::new([(0.0, 0.0, 0.0); 3]);

// ENGINE 05: Tier-0 VPIN (Volume-Synchronized Probability of Informed Trading)
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

    let price = parsed
        .get("mark_price")
        .and_then(|p| p.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let quotes = parsed.get("quotes");
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

    if price <= 0.0 || (bid_sz + ask_sz) <= 0.0 {
        return;
    }

    if let Ok(mut state) = VPIN_STATE.lock() {
        let (last_price, mut buy_bucket, mut sell_bucket) = state[idx];

        if last_price > 0.0 {
            // Exponential Rolling Decay (नए टिक को ज्यादा वजन)
            buy_bucket *= 0.85;
            sell_bucket *= 0.85;

            if price > last_price {
                buy_bucket += bid_sz + (ask_sz * 0.5);
            } else if price < last_price {
                sell_bucket += ask_sz + (bid_sz * 0.5);
            } else {
                buy_bucket += bid_sz * 0.5;
                sell_bucket += ask_sz * 0.5;
            }

            let total_vol = buy_bucket + sell_bucket;
            if total_vol > 0.0 {
                let vpin_toxicity = (buy_bucket - sell_bucket).abs() / total_vol;
                let flow_dir = if buy_bucket > sell_bucket { "BUY_TOXIC" } else { "SELL_TOXIC" };

                let alert = if vpin_toxicity > 0.55 {
                    "☣️ [SMART_MONEY_TOXIC_FLOW]"
                } else {
                    "🟢 [BENIGN_RETAIL_FLOW]"
                };

                println!(
                    "🧪 [E05:{}] VPIN:{:.3} | DIR:{} -> {}",
                    symbol, vpin_toxicity, flow_dir, alert
                );
            }
        }
        state[idx] = (price, buy_bucket, sell_bucket);
    }
}
