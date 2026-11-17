use nalgebra::{Matrix2, Vector2};
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, mean_vec, cov_mat)
static MAHAL_STATE: Mutex<[(f64, Vector2<f64>, Matrix2<f64>); 3]> = Mutex::new([
    (0.0, Vector2::new(0.0, 0.0), Matrix2::new(1.0, 0.0, 0.0, 0.1)),
    (0.0, Vector2::new(0.0, 0.0), Matrix2::new(1.0, 0.0, 0.0, 0.1)),
    (0.0, Vector2::new(0.0, 0.0), Matrix2::new(1.0, 0.0, 0.0, 0.1)),
]);

// ENGINE 24: Tier-0 Mahalanobis Distance Multivariate Anomaly Sniffer
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
        .unwrap_or(0.0);
    let ask_sz = quotes
        .and_then(|q| q.get("ask_size"))
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let total_sz = bid_sz + ask_sz;
    if price <= 0.0 || total_sz <= 0.0 {
        return;
    }

    if let Ok(mut state) = MAHAL_STATE.lock() {
        let (last_p, mean_v, cov_m) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, mean_v, cov_m);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let imb = (bid_sz - ask_sz) / total_sz;
        let x_vec = Vector2::new(ret_bps, imb);

        let diff = x_vec - mean_v;
        // Regularized Covariance Matrix Inverse via nalgebra
        let reg_cov = cov_m + Matrix2::new(0.05, 0.0, 0.0, 0.01);
        let inv_cov = reg_cov.try_inverse().unwrap_or_else(Matrix2::identity);

        // Mahalanobis Distance: D_M = sqrt( diff^T * Sigma^-1 * diff )
        let d_sq = diff.dot(&(inv_cov * diff)).max(0.0);
        let mahal_d = d_sq.sqrt();

        // Exponentially update Mean Vector and 2x2 Covariance Matrix
        let new_mean = (mean_v * 0.90) + (x_vec * 0.10);
        let outer = diff * diff.transpose();
        let new_cov = (cov_m * 0.90) + (outer * 0.10);

        let anomaly_sig = if mahal_d > 3.0 {
            "🚨 [3D_MULTIVARIATE_ANOMALY_BURST]"
        } else if mahal_d > 2.0 {
            "⚡ [COVARIANCE_REGIME_STRETCH]"
        } else {
            "🟢 [ELLIPTICAL_NORMAL_FLOW]"
        };

        println!(
            "📡 [E24:{}] MAHAL_D:{:.2}σ | VEC:[{:+.2}bps, {:+.2}imb] -> {}",
            symbol, mahal_d, ret_bps, imb, anomaly_sig
        );

        state[idx] = (price, new_mean, new_cov);
    }
}
