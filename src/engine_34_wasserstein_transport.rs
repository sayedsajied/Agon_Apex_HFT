use nalgebra::Vector4;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): previous 4-bin normalized liquidity CDF [Bid2, Bid1, Ask1, Ask2]
static WASSERSTEIN_STATE: Mutex<[Vector4<f64>; 3]> = Mutex::new([
    Vector4::new(0.25, 0.50, 0.75, 1.0),
    Vector4::new(0.25, 0.50, 0.75, 1.0),
    Vector4::new(0.25, 0.50, 0.75, 1.0),
]);

// ENGINE 34: Tier-0 Wasserstein Earth-Mover's Distance (Optimal Transport Liquidity Tracker)
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
        Some(b) if b.len() >= 2 => b,
        _ => return,
    };
    let sells = match parsed.get("sell").and_then(|s| s.as_array()) {
        Some(s) if s.len() >= 2 => s,
        _ => return,
    };

    let b2 = parse_f64(buys[1].get("size"));
    let b1 = parse_f64(buys[0].get("size"));
    let a1 = parse_f64(sells[0].get("size"));
    let a2 = parse_f64(sells[1].get("size"));

    let total_mass = (b2 + b1 + a1 + a2).max(1e-6);
    if total_mass <= 0.001 {
        return;
    }

    // Current Empirical Cumulative Liquidity Mass Function F_t across [Bid2, Bid1, Ask1, Ask2]
    let c0 = b2 / total_mass;
    let c1 = (b2 + b1) / total_mass;
    let c2 = (b2 + b1 + a1) / total_mass;
    let curr_cdf = Vector4::new(c0, c1, c2, 1.0);

    if let Ok(mut state) = WASSERSTEIN_STATE.lock() {
        let prev_cdf = state[idx];

        // 1D Wasserstein-1 (Earth-Mover) Distance via nalgebra L1 Norm: W_1 = sum |F_t - F_{t-1}|
        let diff_cdf = curr_cdf - prev_cdf;
        let w1_cost = diff_cdf.lp_norm(1);

        // Net Signed Mass Transport Direction (Positive = Mass shifted toward Bids)
        let net_shift = diff_cdf[0] + diff_cdf[1] + diff_cdf[2];

        let transport_regime = if w1_cost > 0.22 && net_shift > 0.05 {
            "🚜 [EARTH_MOVER_BID_MASS_INFLOW]"
        } else if w1_cost > 0.22 && net_shift < -0.05 {
            "🚜 [EARTH_MOVER_ASK_MASS_WALL]"
        } else {
            "⚖️ [STATIC_TRANSPORT_EQUILIBRIUM]"
        };

        println!(
            "🚚 [E34:{}] W1_COST:{:.3} | MASS_SHIFT:{:+.3} -> {}",
            symbol, w1_cost, net_shift, transport_regime
        );

        state[idx] = (prev_cdf * 0.70) + (curr_cdf * 0.30);
    }
}

fn parse_f64(val: Option<&Value>) -> f64 {
    match val {
        Some(v) => v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0),
        None => 0.0,
    }
}
