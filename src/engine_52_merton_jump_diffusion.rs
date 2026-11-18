use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, diff_sigma, mjd_vec[lambda_intensity, mean_jump_k, comp_drift])
static MJD_STATE: Mutex<[(f64, f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, 0.8, Vector3::new(0.05, 0.0, 0.0)),
    (0.0, 0.8, Vector3::new(0.05, 0.0, 0.0)),
    (0.0, 0.8, Vector3::new(0.05, 0.0, 0.0)),
]);

// ENGINE 52: Tier-0 Merton Jump-Diffusion (Poisson Jump Intensity & Compensated Drift Pricer)
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

    if let Ok(mut state) = MJD_STATE.lock() {
        let (last_p, prev_sig, prev_mjd) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, 0.8, prev_mjd);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let abs_ret = ret_bps.abs();

        // Separate Poisson Jump Event dN_t = 1 if |ret_bps| > 2.2 * sigma_diffusion
        let is_jump = if abs_ret > (2.2 * prev_sig) && abs_ret > 0.6 { 1.0 } else { 0.0 };

        // Update continuous diffusion volatility (excluding raw jump outliers)
        let next_sig = if is_jump > 0.5 {
            (prev_sig * 0.96) + (abs_ret * 0.04)
        } else {
            ((prev_sig * 0.88) + (abs_ret * 0.12)).clamp(0.15, 10.0)
        };

        // Poisson Arrival Intensity lambda & Mean Jump Magnitude k = E[J - 1]
        let lambda = ((prev_mjd[0] * 0.90) + (is_jump * 0.10)).clamp(0.01, 0.99);
        let jump_k = if is_jump > 0.5 {
            (prev_mjd[1] * 0.65) + (ret_bps * 0.35)
        } else {
            prev_mjd[1] * 0.92
        };

        // Merton Compensated Drift Rate: mu_comp = r_t - lambda * k, and 1-tick Poisson Jump Prob = 1 - exp(-lambda)
        let comp_drift = (prev_mjd[2] * 0.80) + ((ret_bps - (lambda * jump_k)) * 0.20);
        let poisson_prob = (1.0 - (-lambda).exp()) * 100.0;
        let next_mjd = Vector3::new(lambda, jump_k, comp_drift);

        let mjd_regime = if lambda > 0.18 && jump_k > 0.50 {
            "🚀 [MERTON_POISSON_UP_JUMP_CLUSTER]"
        } else if lambda > 0.18 && jump_k < -0.50 {
            "🩸 [MERTON_POISSON_CRASH_JUMP_CASCADE]"
        } else {
            "🌊 [MERTON_PURE_GAUSSIAN_DIFFUSION]"
        };

        println!(
            "☄️ [E52:{}] λ_JUMP:{:.1}% | E[J]:{:+.2}bps | μ_COMP:{:+.2}bps -> {}",
            symbol, poisson_prob, next_mjd[1], next_mjd[2], mjd_regime
        );

        state[idx] = (price, next_sig, next_mjd);
    }
}
