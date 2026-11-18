use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, nig_vec[alpha_tail, beta_asym, delta_scale])
static NIG_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(1.5, 0.0, 0.8)),
    (0.0, Vector3::new(1.5, 0.0, 0.8)),
    (0.0, Vector3::new(1.5, 0.0, 0.8)),
]);

// ENGINE 55: Tier-0 Normal Inverse Gaussian (NIG) Barndorff-Nielsen Hyperbolic Heavy-Tail Levy Engine
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

    if let Ok(mut state) = NIG_STATE.lock() {
        let (last_p, prev_nig) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_nig);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let abs_ret = ret_bps.abs();

        // Update Scale parameter delta & Asymmetry parameter beta via nalgebra Vector3
        let delta = ((prev_nig[2] * 0.86) + (abs_ret * 0.14)).clamp(0.15, 12.0);
        let inst_asym = (ret_bps / delta).clamp(-2.5, 2.5);
        let beta = (prev_nig[1] * 0.84) + (inst_asym * 0.16);

        // NIG Tail Heaviness alpha must strictly satisfy alpha > |beta|
        let tail_shock = (abs_ret / delta).clamp(0.2, 4.0);
        let raw_alpha = (prev_nig[0] * 0.85) + ((1.8 / tail_shock.sqrt()) * 0.15);
        let alpha = raw_alpha.max(beta.abs() + 0.15).clamp(0.25, 5.0);

        // Barndorff-Nielsen Subordinator Drift gamma = sqrt(alpha^2 - beta^2)
        let gamma_sub = ((alpha * alpha) - (beta * beta)).max(0.01).sqrt();

        // NIG Implied First Moment (Mean Drift): E[X] = delta * (beta / gamma)
        let nig_drift_bps = delta * (beta / gamma_sub);
        let next_nig = Vector3::new(alpha, beta, delta);

        let nig_regime = if nig_drift_bps > 0.55 && alpha < 1.35 {
            "🚀 [NIG_HYPERBOLIC_UPPER_TAIL_BURST]"
        } else if nig_drift_bps < -0.55 && alpha < 1.35 {
            "🩸 [NIG_HYPERBOLIC_LOWER_TAIL_CRASH]"
        } else {
            "⚖️ [NIG_INVERSE_GAUSSIAN_EQUILIBRIUM]"
        };

        println!(
            "🌀 [E55:{}] α_TAIL:{:.2} | β_ASYM:{:+.2} | NIG_E[X]:{:+.2}bps -> {}",
            symbol, next_nig[0], next_nig[1], nig_drift_bps, nig_regime
        );

        state[idx] = (price, next_nig);
    }
}
