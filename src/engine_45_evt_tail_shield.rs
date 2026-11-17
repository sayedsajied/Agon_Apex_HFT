use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, evt_vec[threshold_u, excess_mean, tail_es99])
static EVT_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(1.2, 0.5, 2.5)),
    (0.0, Vector3::new(1.2, 0.5, 2.5)),
    (0.0, Vector3::new(1.2, 0.5, 2.5)),
]);

// ENGINE 45: Tier-0 Extreme Value Theory (EVT) & Generalized Pareto Tail-Risk Shield
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

    if let Ok(mut state) = EVT_STATE.lock() {
        let (last_p, prev_evt) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_evt);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let abs_ret = ret_bps.abs();

        // Peaks-Over-Threshold (POT) dynamic quantile baseline u & tail exceedance y = max(|r| - u, 0)
        let thresh_u = ((prev_evt[0] * 0.92) + (abs_ret * 0.12)).clamp(0.4, 8.0);
        let excess_y = (abs_ret - thresh_u).max(0.0);
        let mean_excess = if excess_y > 0.0 {
            (prev_evt[1] * 0.80) + (excess_y * 0.20)
        } else {
            (prev_evt[1] * 0.96).max(0.15)
        };

        // Generalized Pareto Distribution (GPD) Heavy-Tail Shape Index xi in [0.05, 0.45]
        let xi_shape = ((mean_excess / (thresh_u + mean_excess)) * 0.65).clamp(0.05, 0.45);
        let beta_scale = (mean_excess * (1.0 - xi_shape)).max(0.1);

        // 99% Extreme Tail Expected Shortfall (CVaR / ES_99) in Basis Points
        let var_99 = thresh_u + (beta_scale / xi_shape) * ((10.0_f64).powf(xi_shape) - 1.0);
        let es_99 = (var_99 + beta_scale - (xi_shape * thresh_u)) / (1.0 - xi_shape);

        let evt_vec = Vector3::new(thresh_u, mean_excess, es_99);

        let tail_regime = if abs_ret > var_99 || xi_shape > 0.32 {
            "🦢 [EVT_BLACK_SWAN_FAT_TAIL_BREACH]"
        } else if excess_y > 0.0 {
            "⚠️ [EVT_POT_THRESHOLD_EXCEEDANCE]"
        } else {
            "🛡️ [EVT_BOUNDED_GAUSSIAN_SAFE]"
        };

        println!(
            "🛡️ [E45:{}] ξ_SHAPE:{:.3} | POT_U:{:.2}bps | ES99_RISK:{:.2}bps -> {}",
            symbol, xi_shape, evt_vec[0], evt_vec[2], tail_regime
        );

        state[idx] = (price, evt_vec);
    }
}
