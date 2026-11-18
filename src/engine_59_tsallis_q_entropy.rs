use nalgebra::Vector4;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, rolling_var, q_index, prob_vec4)
static TSALLIS_STATE: Mutex<[(f64, f64, f64, Vector4<f64>); 3]> = Mutex::new([
    (0.0, 1.0, 1.35, Vector4::new(0.25, 0.25, 0.25, 0.25)),
    (0.0, 1.0, 1.35, Vector4::new(0.25, 0.25, 0.25, 0.25)),
    (0.0, 1.0, 1.35, Vector4::new(0.25, 0.25, 0.25, 0.25)),
]);

// ENGINE 59: Tier-0 Tsallis Non-Extensive q-Entropy & Power-Law q-Gaussian Cascade Engine
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

    if let Ok(mut state) = TSALLIS_STATE.lock() {
        let (last_p, prev_var, prev_q, prev_p) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, 1.0, 1.35, prev_p);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let sq_ret = ret_bps * ret_bps;
        let sigma_sq = ((prev_var * 0.88) + (sq_ret * 0.12)).max(0.05);

        // Estimate Tsallis Entropic Index q from excess kurtosis bursts (q -> 1 is Gaussian, q > 1.5 is heavy power-law)
        let tail_ratio = (sq_ret / sigma_sq).clamp(0.1, 12.0);
        let inst_q = (1.10 + (0.12 * tail_ratio)).clamp(1.05, 2.40);
        let q_idx = (prev_q * 0.85) + (inst_q * 0.15);

        // q-Gaussian Weights across 4 state attractors [-2.0, -0.6, +0.6, +2.0] via Tsallis q-exponential
        let states = Vector4::new(-2.0, -0.6, 0.6, 2.0);
        let beta = 1.0 / (2.0 * sigma_sq);
        let q_weights = states.map(|s| {
            let u = beta * (s - ret_bps).powi(2);
            let base = (1.0 - (1.0 - q_idx) * u).max(1e-4);
            base.powf(1.0 / (1.0 - q_idx))
        });

        let norm_p = q_weights / q_weights.sum().max(1e-6);
        let smooth_p = (prev_p * 0.72) + (norm_p * 0.28);

        // Tsallis Non-Extensive Entropy: S_q = (1 - sum(P_i^q)) / (q - 1)
        let sum_p_q = smooth_p.map(|p| p.powf(q_idx)).sum();
        let tsallis_s_q = (1.0 - sum_p_q) / (q_idx - 1.0);
        let tail_skew = (smooth_p[2] + smooth_p[3]) - (smooth_p[0] + smooth_p[1]);

        let tsallis_regime = if q_idx > 1.52 && tail_skew > 0.28 {
            "⚡ [TSALLIS_Q_GAUSSIAN_BULL_AVALANCHE]"
        } else if q_idx > 1.52 && tail_skew < -0.28 {
            "🩸 [TSALLIS_Q_GAUSSIAN_BEAR_CASCADE]"
        } else {
            "⚖️ [TSALLIS_EXTENSIVE_GAUSSIAN_EQUILIBRIUM]"
        };

        println!(
            "🌪️ [E59:{}] q_INDEX:{:.3} | S_q_ENTROPY:{:.3} | q_SKEW:{:+.2} -> {}",
            symbol, q_idx, tsallis_s_q, tail_skew, tsallis_regime
        );

        state[idx] = (price, sigma_sq, q_idx, smooth_p);
    }
}
