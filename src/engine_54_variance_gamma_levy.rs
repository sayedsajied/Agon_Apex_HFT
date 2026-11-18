use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, vg_vec[theta_skew, sigma_sq, nu_kurtosis])
static VG_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.0, 1.0, 0.25)),
    (0.0, Vector3::new(0.0, 1.0, 0.25)),
    (0.0, Vector3::new(0.0, 1.0, 0.25)),
]);

// ENGINE 54: Tier-0 Variance Gamma (Madan-Carr-Chang Pure Jump Levy Process & Gamma Subordinator)
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

    if let Ok(mut state) = VG_STATE.lock() {
        let (last_p, prev_vg) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_vg);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;

        // Decompose Pure-Jump Levy Process into Up-Jump (Gamma_p) and Down-Jump (Gamma_n) increments
        let up_jump = ret_bps.max(0.0);
        let dn_jump = (-ret_bps).max(0.0);

        // Levy Skewness parameter theta = E[G_p - G_n] & Variance sigma^2
        let theta = (prev_vg[0] * 0.85) + ((up_jump - dn_jump) * 0.15);
        let centered_sq = (ret_bps - theta) * (ret_bps - theta);
        let sigma_sq = ((prev_vg[1] * 0.86) + (centered_sq * 0.14)).clamp(0.05, 30.0);

        // Gamma Subordinator Variance Rate nu (controls excess kurtosis: K_excess = 3 * nu)
        let norm_fourth = (centered_sq * centered_sq) / (sigma_sq * sigma_sq).max(0.01);
        let inst_nu = ((norm_fourth - 3.0).max(0.0) / 3.0).clamp(0.05, 2.5);
        let nu = (prev_vg[2] * 0.88) + (inst_nu * 0.12);

        // Madan-Carr-Chang Exponential Martingale Correction omega = (1/nu) * ln(1 - theta*nu - 0.5*sigma^2*nu)
        let inner_term = (1.0 - (theta * nu * 0.05) - (0.5 * sigma_sq * nu * 0.01)).clamp(0.10, 2.5);
        let levy_omega = (inner_term.ln() / nu) * 10.0;

        let next_vg = Vector3::new(theta, sigma_sq, nu);

        let vg_regime = if next_vg[0] > 0.45 && next_vg[2] > 0.50 {
            "⚡ [VG_LEVY_PURE_UP_JUMP_SUBORDINATOR]"
        } else if next_vg[0] < -0.45 && next_vg[2] > 0.50 {
            "🩸 [VG_LEVY_PURE_DOWN_JUMP_SUBORDINATOR]"
        } else {
            "⚖️ [VG_BALANCED_GAMMA_CLOCK]"
        };

        println!(
            "⏱️ [E54:{}] θ_SKEW:{:+.2} | ν_KURT:{:.2} | ω_LEVY:{:+.2} -> {}",
            symbol, next_vg[0], next_vg[2], levy_omega, vg_regime
        );

        state[idx] = (price, next_vg);
    }
}
