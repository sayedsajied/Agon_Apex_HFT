use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, lg_vec[order_phi, temp_r, free_energy_f])
static LANDAU_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.0, 0.5, 0.0)),
    (0.0, Vector3::new(0.0, 0.5, 0.0)),
    (0.0, Vector3::new(0.0, 0.5, 0.0)),
]);

// ENGINE 70: Tier-0 Landau-Ginzburg Spontaneous Symmetry Breaking & Order-Parameter Field Engine
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

    if let Ok(mut state) = LANDAU_STATE.lock() {
        let (last_p, prev_lg) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_lg);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let h_field = (bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1);

        // Order parameter phi (normalized directional alignment) & gradient d_phi
        let inst_phi = ((ret_bps * 0.8) + (h_field * 1.2)).tanh();
        let phi = (prev_lg[0] * 0.72) + (inst_phi * 0.28);
        let grad_phi = phi - prev_lg[0];

        // Landau Reduced Temperature r = alpha * (T - T_c): r < 0 triggers Spontaneous Symmetry Breaking
        let directional_coherence = phi.abs();
        let inst_r = 0.65 - (1.80 * directional_coherence);
        let r_param = ((prev_lg[1] * 0.82) + (inst_r * 0.18)).clamp(-1.50, 1.50);

        // Landau-Ginzburg Free Energy Density: F = 0.5*r*phi^2 + 0.25*beta*phi^4 + 0.5*kappa*(grad_phi)^2 - h*phi
        let beta = 1.20;
        let kappa = 0.80;
        let free_f = (0.5 * r_param * phi * phi)
            + (0.25 * beta * phi.powi(4))
            + (0.5 * kappa * grad_phi * grad_phi)
            - (0.40 * h_field * phi);

        let next_lg = Vector3::new(phi, r_param, free_f);

        let lg_regime = if r_param < -0.15 && phi > 0.45 {
            "⚡ [LANDAU_SYMMETRY_BROKEN_BULL_CONDENSATE]"
        } else if r_param < -0.15 && phi < -0.45 {
            "🩸 [LANDAU_SYMMETRY_BROKEN_BEAR_CONDENSATE]"
        } else if r_param.abs() <= 0.15 {
            "🌋 [GINZBURG_CRITICAL_OPALESCENCE_POINT]"
        } else {
            "⚖️ [LANDAU_SYMMETRIC_DISORDERED_PHASE]"
        };

        println!(
            "👑 [E70:{}] φ_ORDER:{:+.3} | r_LANDAU:{:+.3} | F_LG:{:+.3} -> {}",
            symbol, next_lg[0], next_lg[1], next_lg[2], lg_regime
        );

        state[idx] = (price, next_lg);
    }
}
