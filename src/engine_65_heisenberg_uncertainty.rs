use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, h_vec[delta_x_pos, delta_p_mom, uncertainty_product])
static HEISENBERG_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(1.0, 1.0, 1.0)),
    (0.0, Vector3::new(1.0, 1.0, 1.0)),
    (0.0, Vector3::new(1.0, 1.0, 1.0)),
]);

// ENGINE 65: Tier-0 Heisenberg Uncertainty Principle & Phase-Space Minimum Wavepacket Bound Engine
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

    if price <= 0.0 {
        return;
    }

    if let Ok(mut state) = HEISENBERG_STATE.lock() {
        let (last_p, prev_h) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_h);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let abs_ret = ret_bps.abs();

        // Position dispersion delta_x (Price volatility envelope) & Momentum dispersion delta_p (Velocity variance)
        let delta_x = ((prev_h[0] * 0.84) + (abs_ret * 0.16)).clamp(0.05, 50.0);
        let velocity_v = ret_bps;
        let delta_p = ((prev_h[1] * 0.84) + (velocity_v.abs() * 0.16)).clamp(0.05, 50.0);

        // Heisenberg Phase-Space Action Uncertainty Product: Delta_x * Delta_p >= hbar_effective / 2
        let hbar_effective = 0.50; // Canonical scaled quantum action constant for trading phase-space
        let inst_product = delta_x * delta_p;
        let uncertainty_prod = (prev_h[2] * 0.75) + (inst_product * 0.25);

        // Coherence compression ratio relative to the quantum minimum bound
        let bound_ratio = uncertainty_prod / (hbar_effective * 0.5);
        let next_h = Vector3::new(delta_x, delta_p, uncertainty_prod);

        let heisenberg_regime = if bound_ratio < 1.25 && ret_bps > 0.30 {
            "⚛️ [HEISENBERG_MIN_UNCERTAINTY_BULL_SQUEEZE]"
        } else if bound_ratio < 1.25 && ret_bps < -0.30 {
            "⚛️ [HEISENBERG_MIN_UNCERTAINTY_BEAR_CRASH]"
        } else if bound_ratio > 4.50 {
            "🌫️ [HEISENBERG_HIGH_ENTROPY_THERMAL_NOISE]"
        } else {
            "⚖️ [HEISENBERG_COHERENT_WAVEPACKET_DRIFT]"
        };

        println!(
            "📐 [E65:{}] ΔX:{:.2} | ΔP:{:.2} | PROD:{:.3} (Ratio:{:.2}) -> {}",
            symbol, next_h[0], next_h[1], next_h[2], bound_ratio, heisenberg_regime
        );

        state[idx] = (price, next_h);
    }
}
