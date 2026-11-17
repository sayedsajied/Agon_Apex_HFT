use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, phase_vec[r_t2, r_t1, r_t], prev_dist, smooth_lambda)
static LYAPUNOV_STATE: Mutex<[(f64, Vector3<f64>, f64, f64); 3]> = Mutex::new([
    (0.0, Vector3::new(0.0, 0.0, 0.0), 0.1, -0.05),
    (0.0, Vector3::new(0.0, 0.0, 0.0), 0.1, -0.05),
    (0.0, Vector3::new(0.0, 0.0, 0.0), 0.1, -0.05),
]);

// ENGINE 32: Tier-0 Largest Lyapunov Exponent (Phase-Space Chaos & Divergence Detector)
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

    if let Ok(mut state) = LYAPUNOV_STATE.lock() {
        let (last_p, prev_phase, prev_d, prev_lam) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_phase, 0.1, -0.05);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;

        // Takens' Delay Embedding 3D Phase-Space Vector: Z(t) = [r(t-2), r(t-1), r(t)]
        let curr_phase = Vector3::new(prev_phase[1], prev_phase[2], ret_bps);
        let delta_vec = curr_phase - prev_phase;
        let curr_d = delta_vec.norm().max(0.02);

        // Local Lyapunov Exponent: lambda = ln( ||dZ(t)|| / ||dZ(t-1)|| )
        let inst_lambda = (curr_d / prev_d.max(0.02)).ln().clamp(-2.0, 2.0);
        let smooth_lambda = (prev_lam * 0.85) + (inst_lambda * 0.15);

        let chaos_regime = if smooth_lambda > 0.18 {
            "🌪️ [LYAPUNOV_CHAOTIC_BIFURCATION]"
        } else if smooth_lambda < -0.08 {
            "🧲 [STABLE_ATTRACTOR_ORBIT]"
        } else {
            "⚖️ [EDGE_OF_CHAOS_TRANSITION]"
        };

        println!(
            "🦋 [E32:{}] PHASE_D:{:.2} | LYAPUNOV_λ:{:+.3} -> {}",
            symbol, curr_d, smooth_lambda, chaos_regime
        );

        state[idx] = (price, curr_phase, curr_d, smooth_lambda);
    }
}
