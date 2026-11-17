use nalgebra::{Matrix3, Vector3};
use serde_json::Value;
use std::sync::Mutex;

// State: (last_prices[3], latest_returns[3], 3x3 covariance_matrix)
static EIGEN_STATE: Mutex<(Vector3<f64>, Vector3<f64>, Matrix3<f64>)> = Mutex::new((
    Vector3::new(0.0, 0.0, 0.0),
    Vector3::new(0.0, 0.0, 0.0),
    Matrix3::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
));

// ENGINE 25: Tier-0 Eigenvalue Portfolio Covariance (BTC-ETH-SOL Trifecta Matrix)
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

    if let Ok(mut state) = EIGEN_STATE.lock() {
        let (mut last_p, mut rets, cov_m) = *state;

        if last_p[idx] <= 0.0 {
            last_p[idx] = price;
            *state = (last_p, rets, cov_m);
            return;
        }

        // Update return in Basis Points (bps) for the active leg of the Trifecta
        rets[idx] = ((price - last_p[idx]) / last_p[idx]) * 10000.0;
        last_p[idx] = price;

        // Update 3x3 Cross-Asset Covariance Matrix: Sigma_t = 0.90 * Sigma_{t-1} + 0.10 * (r * r^T)
        let outer = rets * rets.transpose();
        let new_cov = (cov_m * 0.90) + (outer * 0.10);

        // Compute Real Symmetric Eigenvalues via nalgebra
        let eig = new_cov.symmetric_eigenvalues();
        let max_eig = eig.max();
        let trace_sum = (eig[0].abs() + eig[1].abs() + eig[2].abs()).max(1e-6);
        let absorption_ratio = (max_eig / trace_sum).clamp(0.0, 1.0);

        let sync_regime = if absorption_ratio > 0.78 {
            "⚡ [SYSTEMIC_TRIFECTA_SYNC_BURST]"
        } else if absorption_ratio < 0.45 {
            "🔀 [DECOUPLED_ROTATION_REGIME]"
        } else {
            "🔗 [NORMAL_COVARIANCE_LINK]"
        };

        println!(
            "🌐 [E25:{}] λ_MAX:{:.3} | SYNC_RATIO:{:.1}% -> {}",
            symbol, max_eig, absorption_ratio * 100.0, sync_regime
        );

        *state = (last_p, rets, new_cov);
    }
}
