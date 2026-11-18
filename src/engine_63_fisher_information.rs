use nalgebra::{Matrix2, Vector2};
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, theta_vec[mean_mu, var_sigma2])
static FISHER_STATE: Mutex<[(f64, Vector2<f64>); 3]> = Mutex::new([
    (0.0, Vector2::new(0.0, 1.0)),
    (0.0, Vector2::new(0.0, 1.0)),
    (0.0, Vector2::new(0.0, 1.0)),
]);

// ENGINE 63: Tier-0 Fisher Information Metric & Cramer-Rao Riemannian Manifold Engine
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

    if let Ok(mut state) = FISHER_STATE.lock() {
        let (last_p, prev_theta) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_theta);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;

        // Parameter manifold coordinates theta = [mu, sigma^2]
        let mu = (prev_theta[0] * 0.84) + (ret_bps * 0.16);
        let diff = ret_bps - mu;
        let sigma_sq = ((prev_theta[1] * 0.85) + (diff * diff * 0.15)).clamp(0.05, 25.0);

        let next_theta = Vector2::new(mu, sigma_sq);
        let d_theta = next_theta - prev_theta;

        // Fisher Information Metric Tensor G(theta) = diag(1 / sigma^2, 1 / (2 * sigma^4))
        let g11 = 1.0 / sigma_sq;
        let g22 = 1.0 / (2.0 * sigma_sq * sigma_sq);
        let fisher_g = Matrix2::new(g11, 0.0, 0.0, g22);

        // Squared Riemannian Geodesic Distance ds^2 = d_theta^T * G * d_theta
        let ds_sq = d_theta.dot(&(fisher_g * d_theta));
        let fisher_speed = ds_sq.sqrt();

        // Cramer-Rao Lower Bound Trace: Tr(G^-1) = sigma^2 + 2 * sigma^4
        let crlb_bound = sigma_sq + (2.0 * sigma_sq * sigma_sq);

        let fisher_regime = if fisher_speed > 0.35 && mu > 0.25 {
            "📐 [FISHER_GEODESIC_WARP_BULL_BREAKOUT]"
        } else if fisher_speed > 0.35 && mu < -0.25 {
            "📐 [FISHER_GEODESIC_WARP_BEAR_BREAKOUT]"
        } else if crlb_bound < 0.80 {
            "🔬 [CRAMER_RAO_HYPER_PRECISION_LOCK]"
        } else {
            "⚖️ [FISHER_MANIFOLD_FLAT_EQUILIBRIUM]"
        };

        println!(
            "🧭 [E63:{}] ds_FISHER:{:.4} | CRLB_VAR:{:.2} | μ_MANIFOLD:{:+.2} -> {}",
            symbol, fisher_speed, crlb_bound, next_theta[0], fisher_regime
        );

        state[idx] = (price, next_theta);
    }
}
