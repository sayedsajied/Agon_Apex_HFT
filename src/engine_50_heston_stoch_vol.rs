use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, heston_vec[inst_var_v, long_theta, spot_vol_rho])
static HESTON_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(1.0, 1.0, 0.0)),
    (0.0, Vector3::new(1.0, 1.0, 0.0)),
    (0.0, Vector3::new(1.0, 1.0, 0.0)),
]);

// ENGINE 50: Tier-0 Heston-CIR Stochastic Volatility, Feller Ratio & Spot-Vol Leverage Engine
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

    if let Ok(mut state) = HESTON_STATE.lock() {
        let (last_p, prev_h) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_h);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let inst_var = (ret_bps * ret_bps).max(0.02);

        // CIR Stochastic Variance State v_t & Long-Run Mean Variance Theta
        let v_prev = prev_h[0].max(0.02);
        let theta = ((prev_h[1] * 0.96) + (inst_var * 0.04)).max(0.10);
        let dv = inst_var - v_prev;

        // Heston Mean-Reversion Speed kappa = 0.25 & Vol-of-Vol xi estimation
        let kappa = 0.25;
        let xi_vol_of_vol = (dv.abs() / v_prev.sqrt()).clamp(0.10, 5.0);
        let v_next = (v_prev + kappa * (theta - v_prev) + (0.20 * dv)).max(0.02);

        // Spot-Volatility Leverage Correlation rho = Corr(dS, dv)
        let cov_sign = (ret_bps * dv).clamp(-10.0, 10.0) / 10.0;
        let rho = ((prev_h[2] * 0.85) + (cov_sign * 0.15)).clamp(-0.98, 0.98);

        // Cox-Ingersoll-Ross Feller Condition Ratio: 2 * kappa * theta / xi^2 (>= 1.0 means strictly positive var)
        let feller_ratio = (2.0 * kappa * theta) / (xi_vol_of_vol * xi_vol_of_vol);
        let next_h = Vector3::new(v_next, theta, rho);

        let heston_regime = if v_next > theta * 1.8 && rho > 0.25 {
            "🚀 [HESTON_INVERSE_LEVERAGE_BULL_VOL_EXPLOSION]"
        } else if v_next > theta * 1.8 && rho < -0.25 {
            "🩸 [HESTON_PANIC_LEVERAGE_CRASH_SPIKE]"
        } else if feller_ratio < 0.50 {
            "⚡ [CIR_FELLER_BREACH_EXTREME_VOL_OF_VOL]"
        } else {
            "⚖️ [HESTON_MEAN_REVERTING_VARIANCE]"
        };

        println!(
            "🌊 [E50:{}] V_t:{:.2} | θ_LONG:{:.2} | ρ_LEV:{:+.2} | FELLER:{:.2} -> {}",
            symbol, next_h[0], next_h[1], next_h[2], feller_ratio, heston_regime
        );

        state[idx] = (price, next_h);
    }
}
