use nalgebra::{Matrix2, Vector2};
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, metric_g_mat2, curvature_vec[scalar_r, flow_dir])
static RICCI_STATE: Mutex<[(f64, Matrix2<f64>, Vector2<f64>); 3]> = Mutex::new([
    (0.0, Matrix2::new(1.0, 0.0, 0.0, 1.0), Vector2::new(0.0, 0.0)),
    (0.0, Matrix2::new(1.0, 0.0, 0.0, 1.0), Vector2::new(0.0, 0.0)),
    (0.0, Matrix2::new(1.0, 0.0, 0.0, 1.0), Vector2::new(0.0, 0.0)),
]);

// ENGINE 76: Tier-0 Ricci Flow & Riemannian Manifold Scalar Curvature Singularity Engine
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

    if let Ok(mut state) = RICCI_STATE.lock() {
        let (last_p, prev_g, prev_c) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_g, prev_c);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let book_imb = (bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1);

        // Construct instantaneous Riemannian Metric Tensor perturbation from price-liquidity strain
        let g11_obs = (1.0 + ret_bps * ret_bps * 0.35).clamp(0.20, 10.0);
        let g22_obs = (1.0 + book_imb * book_imb * 1.50).clamp(0.20, 10.0);
        let g12_obs = (ret_bps * book_imb * 0.45).clamp(-1.5, 1.5);
        let obs_g = Matrix2::new(g11_obs, g12_obs, g12_obs, g22_obs);

        // Estimate Scalar Curvature R from metric determinant contraction & trace distortion
        let det_g = obs_g.determinant().max(0.05);
        let trace_g = obs_g.trace();
        let inst_r = ((trace_g - 2.0) / det_g) - (prev_g.determinant() - det_g);
        let scalar_r = ((prev_c[0] * 0.76) + (inst_r * 0.24)).clamp(-5.0, 10.0);

        // Hamilton-Perelman Normalized Ricci Flow step: dg_ij / dt = -2 * R_ij (where R_ij = 0.5 * R * g_ij in 2D)
        let dt = 0.12;
        let ricci_step = prev_g * (-scalar_r * dt);
        let mut next_g = (prev_g * 0.82) + (obs_g * 0.18) + ricci_step;
        next_g[(0, 0)] = next_g[(0, 0)].clamp(0.20, 10.0);
        next_g[(1, 1)] = next_g[(1, 1)].clamp(0.20, 10.0);

        let flow_dir = (prev_c[1] * 0.72) + (((ret_bps * 0.6) + (book_imb * 1.2)) * 0.28);
        let next_c = Vector2::new(scalar_r, flow_dir);

        let ricci_regime = if scalar_r > 1.35 && flow_dir > 0.30 {
            "⚡ [RICCI_NECK_PINCH_SINGULARITY_BULL_BURST]"
        } else if scalar_r > 1.35 && flow_dir < -0.30 {
            "🩸 [RICCI_NECK_PINCH_SINGULARITY_BEAR_BURST]"
        } else if scalar_r < -0.45 {
            "🌌 [RICCI_HYPERBOLIC_EXPANSION_COOLING]"
        } else {
            "⚖️ [RIEMANNIAN_FLAT_EUCLIDEAN_METRIC]"
        };

        println!(
            "🕳️ [E76:{}] R_SCALAR:{:+.3} | det(g):{:.3} | FLOW:{:+.2} -> {}",
            symbol, next_c[0], det_g, next_c[1], ricci_regime
        );

        state[idx] = (price, next_g, next_c);
    }
}
