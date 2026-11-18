use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, cas_vec[spread_d_bps, casimir_force, plate_asymmetry])
static CASIMIR_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(1.0, 0.5, 0.0)),
    (0.0, Vector3::new(1.0, 0.5, 0.0)),
    (0.0, Vector3::new(1.0, 0.5, 0.0)),
]);

// ENGINE 74: Tier-0 Casimir Quantum Vacuum Fluctuation & Micro-Spread Zero-Point Force Engine
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
    let best_bid = quotes
        .and_then(|q| q.get("best_bid"))
        .and_then(|b| b.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(price);
    let best_ask = quotes
        .and_then(|q| q.get("best_ask"))
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(price);
    let bid_sz = quotes
        .and_then(|q| q.get("bid_size"))
        .and_then(|b| b.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0);
    let ask_sz = quotes
        .and_then(|q| q.get("ask_size"))
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0);

    if price <= 0.0 {
        return;
    }

    if let Ok(mut state) = CASIMIR_STATE.lock() {
        let (last_p, prev_c) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_c);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;

        // Bid-Ask Cavity Separation Distance d (in scaled basis points, clamped to prevent 1/d^4 singularity)
        let raw_spread_bps = (((best_ask - best_bid).max(0.0) / price) * 10000.0).max(0.25);
        let spread_d = ((prev_c[0] * 0.78) + (raw_spread_bps * 0.22)).clamp(0.35, 10.0);

        // Casimir Zero-Point Attractive Pressure: F_cas = (pi^2 * hbar_c) / (240 * d^4) * Plate_Conductivity
        let plate_mass = ((bid_sz * ask_sz).max(1.0).ln() * 0.45).clamp(0.30, 4.0);
        let d_quartic = spread_d.powi(4).max(0.05);
        let inst_casimir = ((1.20 * plate_mass) / d_quartic).clamp(0.01, 25.0);
        let casimir_f = (prev_c[1] * 0.72) + (inst_casimir * 0.28);

        // Dynamic Casimir Directional Impulse when one boundary plate dominates
        let plate_imb = (bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1);
        let asym_drive = (prev_c[2] * 0.75) + (((plate_imb * 0.6) + (ret_bps * 0.4)) * 0.25);

        let next_c = Vector3::new(spread_d, casimir_f, asym_drive);

        let casimir_regime = if casimir_f > 2.20 && asym_drive > 0.28 {
            "⚡ [CASIMIR_VACUUM_IMPLOSION_BULL_SNAP]"
        } else if casimir_f > 2.20 && asym_drive < -0.28 {
            "🩸 [CASIMIR_VACUUM_IMPLOSION_BEAR_SNAP]"
        } else if spread_d > 2.50 {
            "🌌 [CASIMIR_WIDE_CAVITY_LOW_PRESSURE]"
        } else {
            "⚖️ [ZERO_POINT_VACUUM_FLUCTUATION_BALANCE]"
        };

        println!(
            "🧲 [E74:{}] F_CASIMIR:{:.3} | d_GAP:{:.2}bps | DRIVE:{:+.2} -> {}",
            symbol, next_c[1], next_c[0], next_c[2], casimir_regime
        );

        state[idx] = (price, next_c);
    }
}
