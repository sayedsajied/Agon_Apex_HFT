use nalgebra::{Matrix3, Vector3};
use serde_json::Value;
use std::sync::Mutex;

// State: (last_prices[3], prev_returns[3], curr_returns[3], lead_lag_matrix_3x3)
static LEAD_LAG_STATE: Mutex<(Vector3<f64>, Vector3<f64>, Vector3<f64>, Matrix3<f64>)> = Mutex::new((
    Vector3::new(0.0, 0.0, 0.0),
    Vector3::new(0.0, 0.0, 0.0),
    Vector3::new(0.0, 0.0, 0.0),
    Matrix3::zeros(),
));

// ENGINE 26: Tier-0 Cross-Asset Lead-Lag Matrix (BTC Leading ETH/SOL Arbitrage)
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

    if let Ok(mut state) = LEAD_LAG_STATE.lock() {
        let (mut last_p, mut prev_r, mut curr_r, mut ll_mat) = *state;

        if last_p[idx] <= 0.0 {
            last_p[idx] = price;
            *state = (last_p, prev_r, curr_r, ll_mat);
            return;
        }

        // Shift current return to previous lag slot and compute fresh return in bps
        prev_r[idx] = curr_r[idx];
        curr_r[idx] = ((price - last_p[idx]) / last_p[idx]) * 10000.0;
        last_p[idx] = price;

        // Cross-Autocovariance Matrix via nalgebra: L = 0.88 * L + 0.12 * (r_t * r_{t-1}^T)
        let cross_outer = curr_r * prev_r.transpose();
        ll_mat = (ll_mat * 0.88) + (cross_outer * 0.12);

        // BTC (idx 0) impulse leading ETH (idx 1) and SOL (idx 2)
        let btc_impulse = curr_r[0];
        let lag_divergence = btc_impulse - curr_r[idx];
        let btc_lead_beta = ll_mat[(idx, 0)];

        let lead_signal = if idx != 0 && btc_impulse > 1.8 && lag_divergence > 1.2 {
            "🚀 [BTC_LEADING_UP_CATCHUP_LONG]"
        } else if idx != 0 && btc_impulse < -1.8 && lag_divergence < -1.2 {
            "🩸 [BTC_LEADING_DOWN_CATCHUP_SHORT]"
        } else {
            "🔗 [SYNCHRONIZED_NO_LAG]"
        };

        println!(
            "⏱️ [E26:{}] BTC_IMP:{:+.2}bps | LAG_GAP:{:+.2}bps | BETA:{:+.3} -> {}",
            symbol, btc_impulse, lag_divergence, btc_lead_beta, lead_signal
        );

        *state = (last_p, prev_r, curr_r, ll_mat);
    }
}
