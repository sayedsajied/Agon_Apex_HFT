use nalgebra::Vector2;
use serde_json::Value;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, last_time, mu_drift, sigma_sq_var)
static ITO_STATE: Mutex<[(f64, f64, f64, f64); 3]> = Mutex::new([(0.0, 0.0, 0.0, 0.0); 3]);

// ENGINE 17: Tier-0 Ito's Lemma Stochastic Drift & Diffusion Calculator
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

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();

    if let Ok(mut state) = ITO_STATE.lock() {
        let (last_p, last_t, prev_mu, prev_var) = state[idx];

        if last_p > 0.0 && now > last_t {
            let dt = (now - last_t).max(1e-4);
            // Log return d(ln S) and simple return dS/S in Basis Points
            let simple_ret_bps = ((price - last_p) / last_p) * 10000.0;
            let inst_var = (simple_ret_bps * simple_ret_bps) / dt;

            // nalgebra Vector2 state update for [Drift (mu), Variance (sigma^2)]
            let prev_vec = Vector2::new(prev_mu, prev_var);
            let inst_vec = Vector2::new(simple_ret_bps / dt, inst_var);
            let smooth_vec = (prev_vec * 0.85) + (inst_vec * 0.15);

            // Ito's Lemma Exact Geometric Drift: d(ln S) = (mu - 0.5 * sigma^2) * dt
            let ito_net_drift = smooth_vec[0] - (0.5 * smooth_vec[1] * 0.01);
            let sigma = smooth_vec[1].sqrt();

            let drift_regime = if ito_net_drift > 2.5 {
                "🚀 [ITO_POSITIVE_STOCHASTIC_DRIFT]"
            } else if ito_net_drift < -2.5 {
                "🩸 [ITO_NEGATIVE_STOCHASTIC_DRIFT]"
            } else {
                "🎲 [BROWNIAN_DIFFUSION_NOISE]"
            };

            println!(
                "∫ [E17:{}] ITO_DRIFT:{:+.2}bps/s | DIFFUSION_σ:{:.2} -> {}",
                symbol, ito_net_drift, sigma, drift_regime
            );

            state[idx] = (price, now, smooth_vec[0], smooth_vec[1]);
        } else {
            state[idx] = (price, now, 0.0, 0.0);
        }
    }
}
