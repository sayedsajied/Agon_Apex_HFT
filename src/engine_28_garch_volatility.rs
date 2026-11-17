use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, prev_shock_sq, prev_cond_var)
static GARCH_STATE: Mutex<[(f64, f64, f64); 3]> = Mutex::new([(0.0, 0.5, 1.0); 3]);

// ENGINE 28: Tier-0 GARCH(1,1) Conditional Volatility Clustering Forecaster
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

    if let Ok(mut state) = GARCH_STATE.lock() {
        let (last_p, prev_eps2, prev_sigma2) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, 0.5, 1.0);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let eps_sq = ret_bps * ret_bps;

        // GARCH(1,1) Weights via nalgebra Vector3: [omega (baseline), alpha (shock), beta (persistence)]
        let params = Vector3::new(0.05, 0.18, 0.78);
        let state_vec = Vector3::new(1.0, prev_eps2, prev_sigma2);

        // Forecasted Conditional Variance: sigma_t^2 = omega + alpha * eps_{t-1}^2 + beta * sigma_{t-1}^2
        let cond_var = params.dot(&state_vec).max(0.01);
        let cond_vol = cond_var.sqrt();
        let shock_ratio = ret_bps.abs() / cond_vol;

        let vol_regime = if cond_vol > 3.0 || shock_ratio > 2.2 {
            "🌋 [GARCH_VOLATILITY_EXPANSION_CLUSTER]"
        } else if cond_vol < 0.8 {
            "🧊 [GARCH_VOLATILITY_COMPRESSION_COIL]"
        } else {
            "🌊 [GARCH_STEADY_VARIANCE]"
        };

        println!(
            "📈 [E28:{}] GARCH_σ:{:.2}bps | SHOCK_RATIO:{:.2}x -> {}",
            symbol, cond_vol, shock_ratio, vol_regime
        );

        state[idx] = (price, eps_sq, cond_var);
    }
}
