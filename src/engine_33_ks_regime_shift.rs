use nalgebra::Vector4;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, fast_ecdf_vec4, slow_ecdf_vec4)
static KS_STATE: Mutex<[(f64, Vector4<f64>, Vector4<f64>); 3]> = Mutex::new([
    (0.0, Vector4::new(0.25, 0.50, 0.75, 1.0), Vector4::new(0.25, 0.50, 0.75, 1.0)),
    (0.0, Vector4::new(0.25, 0.50, 0.75, 1.0), Vector4::new(0.25, 0.50, 0.75, 1.0)),
    (0.0, Vector4::new(0.25, 0.50, 0.75, 1.0), Vector4::new(0.25, 0.50, 0.75, 1.0)),
]);

// ENGINE 33: Tier-0 Kolmogorov-Smirnov (K-S) Empirical Distribution Shift & Regime Break Detector
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

    if let Ok(mut state) = KS_STATE.lock() {
        let (last_p, prev_fast, prev_slow) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_fast, prev_slow);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;

        // Empirical Cumulative Step Vector I(R <= threshold_k) for thresholds [-1.2, 0.0, +1.2, +inf]
        let step_vec = Vector4::new(
            if ret_bps <= -1.2 { 1.0 } else { 0.0 },
            if ret_bps <= 0.0 { 1.0 } else { 0.0 },
            if ret_bps <= 1.2 { 1.0 } else { 0.0 },
            1.0,
        );

        // Update Fast ECDF (alpha = 0.22) vs Slow Baseline ECDF (alpha = 0.03) via nalgebra
        let fast_ecdf = (prev_fast * 0.78) + (step_vec * 0.22);
        let slow_ecdf = (prev_slow * 0.97) + (step_vec * 0.03);

        // Kolmogorov-Smirnov Supremum Distance: D_KS = max |F_fast(x) - F_slow(x)|
        let diff_vec = fast_ecdf - slow_ecdf;
        let ks_stat = diff_vec.amax();
        let directional_bias = slow_ecdf[1] - fast_ecdf[1]; // Positive if fast distribution shifted upward

        let ks_regime = if ks_stat > 0.28 && directional_bias > 0.0 {
            "🚨 [KS_BULLISH_DISTRIBUTION_BREAK]"
        } else if ks_stat > 0.28 && directional_bias < 0.0 {
            "🚨 [KS_BEARISH_DISTRIBUTION_BREAK]"
        } else {
            "🟢 [STATIONARY_DISTRIBUTION]"
        };

        println!(
            "📊 [E33:{}] KS_D:{:.3} | SHIFT_BIAS:{:+.3} -> {}",
            symbol, ks_stat, directional_bias, ks_regime
        );

        state[idx] = (price, fast_ecdf, slow_ecdf);
    }
}
