use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State: (log_prices[3], spread_mean[3], spread_var[3])
static COINT_STATE: Mutex<(Vector3<f64>, Vector3<f64>, Vector3<f64>)> = Mutex::new((
    Vector3::new(0.0, 0.0, 0.0),
    Vector3::new(0.0, 0.0, 0.0),
    Vector3::new(1.0, 1.0, 1.0),
));

// ENGINE 27: Tier-0 Cointegration Spread Z-Score & Statistical Arbitrage Engine
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

    if let Ok(mut state) = COINT_STATE.lock() {
        let (mut log_p, mut sp_mean, mut sp_var) = *state;
        log_p[idx] = price.ln();

        // Require anchor asset BTC (idx 0) and current asset to be initialized
        if log_p[0] <= 0.0 || log_p[idx] <= 0.0 {
            *state = (log_p, sp_mean, sp_var);
            return;
        }

        // Dynamic Cointegration Hedge Ratio (Beta_0) & Synthetic Spread in bps
        let beta = if sp_mean[idx] == 0.0 {
            log_p[idx] / log_p[0]
        } else {
            (log_p[idx] - (sp_mean[idx] * 0.0001)) / log_p[0]
        };

        let spread_bps = (log_p[idx] - (beta * log_p[0])) * 10000.0;

        if sp_mean[idx] == 0.0 {
            sp_mean[idx] = spread_bps;
            *state = (log_p, sp_mean, sp_var);
            return;
        }

        let diff = spread_bps - sp_mean[idx];
        sp_mean[idx] = (sp_mean[idx] * 0.90) + (spread_bps * 0.10);
        sp_var[idx] = ((sp_var[idx] * 0.90) + (diff * diff * 0.10)).max(1e-4);

        let z_score = if idx == 0 { 0.0 } else { diff / sp_var[idx].sqrt() };

        let arb_signal = if z_score > 2.1 {
            "🔻 [STAT_ARB_RICH_SHORT_SPREAD]"
        } else if z_score < -2.1 {
            "🔺 [STAT_ARB_CHEAP_LONG_SPREAD]"
        } else {
            "🔗 [COINTEGRATED_PARITY]"
        };

        println!(
            "⚖️ [E27:{}] BETA:{:.4} | SPREAD_DIFF:{:+.2}bps | Z:{:+.2}σ -> {}",
            symbol, beta, diff, z_score, arb_signal
        );

        *state = (log_p, sp_mean, sp_var);
    }
}
