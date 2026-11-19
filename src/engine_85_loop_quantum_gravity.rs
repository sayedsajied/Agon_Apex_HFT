use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, lqg_vec[spin_j, area_a, rho_ratio])
static LQG_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.5, 0.5, 0.2)),
    (0.0, Vector3::new(0.5, 0.5, 0.2)),
    (0.0, Vector3::new(0.5, 0.5, 0.2)),
]);

// ENGINE 85: Tier-0 Loop Quantum Gravity (LQG), Spin Network Area & Planck-Scale Bounce Engine
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
        .unwrap_or(1.0);
    let ask_sz = quotes
        .and_then(|q| q.get("ask_size"))
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0);

    if price <= 0.0 {
        return;
    }

    if let Ok(mut state) = LQG_STATE.lock() {
        let (last_p, prev_lqg) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_lqg);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let book_imb = (bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1);
        let depth_log = (bid_sz + ask_sz).max(1.0).ln().clamp(0.4, 8.0);

        // Quantize Spin Network Edge Label j into discrete half-integers: j = 0.5, 1.0, 1.5, ...
        let raw_j = ((ret_bps.abs() * 0.45) + (book_imb.abs() * 1.60)).clamp(0.5, 6.0);
        let spin_j = ((raw_j * 2.0).round() * 0.5).max(0.5);

        // Rovelli-Smolin Quantized Area Operator: A = 8 * pi * gamma * l_p^2 * sqrt(j * (j + 1))
        let immirzi_gamma = 0.2375;
        let inst_area = (2.0 * immirzi_gamma * (spin_j * (spin_j + 1.0)).sqrt()).clamp(0.10, 10.0);
        let area_a = (prev_lqg[1] * 0.75) + (inst_area * 0.25);

        // LQC Energy Density Ratio rho / rho_c -> triggers repulsive Quantum Bounce near 1.0
        let inst_rho = ((ret_bps.abs() * 0.28 + book_imb.abs() * 0.55) * (depth_log / 4.0)).clamp(0.05, 0.99);
        let rho_ratio = (prev_lqg[2] * 0.72) + (inst_rho * 0.28);
        let bounce_drive = (book_imb * 1.25) - (ret_bps * 0.35);

        let next_lqg = Vector3::new(spin_j, area_a, rho_ratio);

        let lqg_regime = if rho_ratio > 0.78 && bounce_drive > 0.28 {
            "🚀 [LQG_PLANCK_QUANTUM_BOUNCE_BULL_REVERSAL]"
        } else if rho_ratio > 0.78 && bounce_drive < -0.28 {
            "🩸 [LQG_PLANCK_QUANTUM_BOUNCE_BEAR_REVERSAL]"
        } else if area_a > 1.45 && rho_ratio <= 0.78 {
            "🕸️ [SPIN_NETWORK_DISCRETE_AREA_EXPANSION]"
        } else {
            "⚛️ [PLANCK_SCALE_SPIN_FOAM_EQUILIBRIUM]"
        };

        println!(
            "🕸️ [E85:{}] j_SPIN:{:.1} | A_LQG:{:.3} | ρ/ρ_c:{:.2} -> {}",
            symbol, next_lqg[0], next_lqg[1], next_lqg[2], lqg_regime
        );

        state[idx] = (price, next_lqg);
    }
}
