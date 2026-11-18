use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, path_actions_s[bear_s, neutral_s, bull_s], coherence_amp)
static FEYNMAN_STATE: Mutex<[(f64, Vector3<f64>, f64); 3]> = Mutex::new([
    (0.0, Vector3::new(1.0, 1.0, 1.0), 0.5),
    (0.0, Vector3::new(1.0, 1.0, 1.0), 0.5),
    (0.0, Vector3::new(1.0, 1.0, 1.0), 0.5),
]);

// ENGINE 75: Tier-0 Feynman Path Integral & Least-Action Quantum Trajectory Optimizer
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

    if let Ok(mut state) = FEYNMAN_STATE.lock() {
        let (last_p, prev_s, prev_coh) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_s, prev_coh);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let book_bias = (bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1);

        // Evaluate Euclidean Action S_k = Integral (T_kinetic + V_potential) dt across 3 virtual trajectories [-1.5, 0.0, +1.5]
        let virtual_paths = Vector3::new(-1.5, 0.0, 1.5);
        let hbar_eff = 0.85;

        let inst_actions = virtual_paths.map(|v_path| {
            let kinetic_dev = 0.5 * (ret_bps - v_path).powi(2);
            let potential_barrier = (1.0 - (v_path.signum() * book_bias)).max(0.1) * 0.8;
            (kinetic_dev + potential_barrier).clamp(0.05, 20.0)
        });

        let action_s = (prev_s * 0.75) + (inst_actions * 0.25);

        // Wick-Rotated Feynman Propagator Weights: W_k = exp(-S_k / hbar) & Constructive Phase Interference
        let prop_weights = action_s.map(|s| (-s / hbar_eff).exp());
        let partition_k = prop_weights.sum().max(1e-6);
        let path_probs = prop_weights / partition_k;

        // Least-Action Trajectory Dominance & Quantum Phase Coherence
        let optimal_dir = path_probs[2] - path_probs[0];
        let max_prob = path_probs.max();
        let coherence = (prev_coh * 0.72) + (max_prob * 0.28);

        let feynman_regime = if coherence > 0.52 && optimal_dir > 0.25 {
            "🚀 [FEYNMAN_LEAST_ACTION_BULL_PROPAGATOR]"
        } else if coherence > 0.52 && optimal_dir < -0.25 {
            "🩸 [FEYNMAN_LEAST_ACTION_BEAR_PROPAGATOR]"
        } else if path_probs[1] > 0.50 {
            "⚖️ [FEYNMAN_STATIONARY_CLASSICAL_EXTREMUM]"
        } else {
            "🌀 [FEYNMAN_DESTRUCTIVE_PHASE_INTERFERENCE]"
        };

        println!(
            "🧬 [E75:{}] S_MIN:{:.2} | COH_K:{:.1}% | DIR_PATH:{:+.2} -> {}",
            symbol,
            action_s.min(),
            coherence * 100.0,
            optimal_dir,
            feynman_regime
        );

        state[idx] = (price, action_s, coherence);
    }
}
