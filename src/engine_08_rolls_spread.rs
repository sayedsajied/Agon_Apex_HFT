use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, prev_dp, rolling_autocov)
static ROLL_STATE: Mutex<[(f64, f64, f64); 3]> = Mutex::new([(0.0, 0.0, 0.0); 3]);

// ENGINE 08: Tier-0 Roll's Model (Effective Spread via Serial Covariance)
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

    if let Ok(mut state) = ROLL_STATE.lock() {
        let (last_price, prev_dp, prev_cov) = state[idx];

        if last_price > 0.0 {
            let dp = price - last_price;
            // Serial Covariance of consecutive price changes: Cov(ΔP_t, ΔP_{t-1})
            let instant_cov = dp * prev_dp;
            let smooth_cov = (prev_cov * 0.85) + (instant_cov * 0.15);

            // Roll's Effective Spread Estimator: S = 2 * sqrt(-Cov) when Cov < 0
            let effective_spread = if smooth_cov < 0.0 {
                2.0 * (-smooth_cov).sqrt()
            } else {
                0.0
            };

            let eff_bps = (effective_spread / price) * 10000.0;

            let bounce_regime = if eff_bps > 3.5 {
                "⚠️ [HIGH_HIDDEN_SPREAD_FRICTION]"
            } else if smooth_cov > 0.0 {
                "🚀 [DIRECTIONAL_MOMENTUM_DRIFT]"
            } else {
                "🎯 [TIGHT_SCALPABLE_SPREAD]"
            };

            println!(
                "📏 [E08:{}] ROLL_SPRD:{:.4} ({:.2}bps) | COV:{:+.5} -> {}",
                symbol, effective_spread, eff_bps, smooth_cov, bounce_regime
            );

            state[idx] = (price, dp, smooth_cov);
        } else {
            state[idx] = (price, 0.0, 0.0);
        }
    }
}
