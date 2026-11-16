use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): rolling smoothed basis in bps
static BASIS_STATE: Mutex<[f64; 3]> = Mutex::new([0.0; 3]);

// ENGINE 15: Tier-0 Funding Rate & Mark-to-Spot Basis Arbitrage Monitor
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

    let mark_price = parse_f64(parsed.get("mark_price"));
    let spot_price = parse_f64(parsed.get("spot_price"));
    let funding_rate = parse_f64(parsed.get("funding_rate"));

    if mark_price <= 0.0 || spot_price <= 0.0 {
        return;
    }

    // Mark-to-Spot Basis in Basis Points (bps)
    let raw_basis_bps = ((mark_price - spot_price) / spot_price) * 10000.0;

    if let Ok(mut state) = BASIS_STATE.lock() {
        let smooth_basis = (state[idx] * 0.80) + (raw_basis_bps * 0.20);
        state[idx] = smooth_basis;

        let arb_regime = if smooth_basis > 15.0 && funding_rate > 0.0 {
            "🔥 [OVERHEATED_LONG_BASIS_FLUSH_RISK]"
        } else if smooth_basis < -15.0 && funding_rate < 0.0 {
            "⚡ [DEEPLY_DISCOUNTED_SHORT_SQUEEZE]"
        } else {
            "🔗 [PARITY_LOCKED_BASIS]"
        };

        println!(
            "💎 [E15:{}] MARK:{:.2} | SPOT:{:.2} | BASIS:{:+.2}bps | FUND:{:+.4}% -> {}",
            symbol, mark_price, spot_price, smooth_basis, funding_rate, arb_regime
        );
    }
}

fn parse_f64(val: Option<&Value>) -> f64 {
    match val {
        Some(v) => v
            .as_f64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            .unwrap_or(0.0),
        None => 0.0,
    }
}
