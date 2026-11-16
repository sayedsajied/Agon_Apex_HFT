use nalgebra::{Matrix2, Vector2};
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (pos, vel, p00, p01, p10, p11, initialized)
static KALMAN_STATE: Mutex<[(f64, f64, f64, f64, f64, f64, bool); 3]> =
    Mutex::new([(0.0, 0.0, 1.0, 0.0, 0.0, 1.0, false); 3]);

// ENGINE 16: Tier-0 nalgebra Matrix State-Space & Kalman Filter Price Smoother
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

    let obs_price = parsed
        .get("mark_price")
        .and_then(|p| p.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    if obs_price <= 0.0 {
        return;
    }

    if let Ok(mut state) = KALMAN_STATE.lock() {
        let (pos, vel, p00, p01, p10, p11, init) = state[idx];

        if !init {
            state[idx] = (obs_price, 0.0, 1.0, 0.0, 0.0, 1.0, true);
            return;
        }

        // State Transition Matrix (F) and Process Noise Covariance (Q) via nalgebra
        let f_mat = Matrix2::new(1.0, 1.0, 0.0, 1.0);
        let q_mat = Matrix2::new(0.01, 0.0, 0.0, 0.005);
        let r_noise = 0.25; // Measurement noise variance

        let x_prev = Vector2::new(pos, vel);
        let p_prev = Matrix2::new(p00, p01, p10, p11);

        // 1. Matrix Prediction Step: x_pred = F * x, P_pred = F * P * F^T + Q
        let x_pred = f_mat * x_prev;
        let p_pred = (f_mat * p_prev * f_mat.transpose()) + q_mat;

        // 2. Innovation & Kalman Gain Update Step
        let innovation = obs_price - x_pred[0];
        let s_cov = p_pred[(0, 0)] + r_noise;
        let k_gain = Vector2::new(p_pred[(0, 0)] / s_cov, p_pred[(1, 0)] / s_cov);

        let x_new = x_pred + (k_gain * innovation);
        let i_kh = Matrix2::new(1.0 - k_gain[0], 0.0, -k_gain[1], 1.0);
        let p_new = i_kh * p_pred;

        let err_bps = (innovation / obs_price) * 10000.0;
        let regime = if err_bps > 1.5 {
            "🟢 [PRICE_ABOVE_KALMAN_MEAN]"
        } else if err_bps < -1.5 {
            "🔴 [PRICE_BELOW_KALMAN_MEAN]"
        } else {
            "🎯 [KALMAN_LOCKED]"
        };

        println!(
            "🧮 [E16:{}] KALMAN_P:{:.2} | K_VEL:{:+.3} | NOISE:{:+.2}bps -> {}",
            symbol, x_new[0], x_new[1], err_bps, regime
        );

        state[idx] = (x_new[0], x_new[1], p_new[(0, 0)], p_new[(0, 1)], p_new[(1, 0)], p_new[(1, 1)], true);
    }
}
