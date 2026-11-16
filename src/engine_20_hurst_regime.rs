use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, cum_dev, max_dev, min_dev, rolling_std, smooth_h)
static HURST_STATE: Mutex<[(f64, f64, f64, f64, f64, f64); 3]> =
    Mutex::new([(0.0, 0.0, 0.0, 0.0, 1.0, 0.5); 3]);

// ENGINE 20: Tier-0 Hurst Exponent (R/S Fractal Dimension & Market Regime Identifier)
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

    if let Ok(mut state) = HURST_STATE.lock() {
        let (last_p, prev_cum, prev_max, prev_min, prev_std, prev_h) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, 0.0, 0.0, 0.0, 0.5, 0.5);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let cum_dev = (prev_cum * 0.85) + ret_bps;
        let max_dev = (prev_max * 0.90).max(cum_dev);
        let min_dev = (prev_min * 0.90).min(cum_dev);

        // Rescaled Range (R/S) Calculation via nalgebra Vector3: [Range (R), StdDev (S), Hurst (H)]
        let range_r = (max_dev - min_dev).max(0.05);
        let std_s = ((prev_std * 0.88) + (ret_bps.abs() * 0.12)).max(0.05);
        let rs_ratio = (range_r / std_s).max(1.05);

        // H = ln(R/S) / ln(N_effective), clamped to [0.1, 0.9]
        let raw_h = (rs_ratio.ln() / (20.0_f64).ln()).clamp(0.10, 0.90);
        let smooth_h = (prev_h * 0.80) + (raw_h * 0.20);
        let hurst_vec = Vector3::new(range_r, std_s, smooth_h);

        let regime = if hurst_vec[2] > 0.56 {
            "🔥 [PERSISTENT_MOMENTUM_TREND]"
        } else if hurst_vec[2] < 0.44 {
            "🪃 [ANTI_PERSISTENT_MEAN_REVERT]"
        } else {
            "🎲 [BROWNIAN_RANDOM_WALK]"
        };

        println!(
            "🧬 [E20:{}] R/S:{:.2} | FRACTAL_H:{:.3} -> {}",
            symbol, hurst_vec[0] / hurst_vec[1], hurst_vec[2], regime
        );

        state[idx] = (price, cum_dev, max_dev, min_dev, std_s, smooth_h);
    }
}
