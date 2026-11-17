use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, rolling_var, ac_vec[eta_impact, kappa_urgency, opt_rate])
static AC_STATE: Mutex<[(f64, f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, 1.0, Vector3::new(0.5, 1.0, 0.5)),
    (0.0, 1.0, Vector3::new(0.5, 1.0, 0.5)),
    (0.0, 1.0, Vector3::new(0.5, 1.0, 0.5)),
]);

// ENGINE 40: Tier-0 Almgren-Chriss Optimal Execution Trajectory & Slippage Minimizer
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
    let best_bid = quotes
        .and_then(|q| q.get("best_bid"))
        .and_then(|b| b.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(price);
    let best_ask = quotes
        .and_then(|q| q.get("best_ask"))
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(price);
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

    if let Ok(mut state) = AC_STATE.lock() {
        let (last_p, prev_var, prev_ac) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, 1.0, prev_ac);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let sigma_sq = ((prev_var * 0.88) + (ret_bps * ret_bps * 0.12)).max(0.05);

        // Temporary Market Impact Coefficient (eta) from live spread & top-of-book depth
        let spread_bps = (((best_ask - best_bid).max(0.0) / price) * 10000.0).max(0.1);
        let top_depth = (bid_sz + ask_sz).max(0.1);
        let eta_impact = (spread_bps / top_depth.sqrt()).clamp(0.05, 50.0);

        // Almgren-Chriss Urgency Parameter: kappa = sqrt( (lambda * sigma^2) / eta )
        let risk_aversion_lambda = 0.35;
        let kappa = ((risk_aversion_lambda * sigma_sq) / eta_impact).sqrt().clamp(0.1, 5.0);

        // Hyperbolic Execution Rate over normalized horizon T=1.0:1 - sinh(kappa * 0.5) / sinh(kappa)
        let exec_fraction = (1.0 - ((kappa * 0.5).sinh() / kappa.sinh())).clamp(0.0, 1.0);
        let inst_vec = Vector3::new(eta_impact, kappa, exec_fraction);
        let smooth_ac = (prev_ac * 0.75) + (inst_vec * 0.25);

        let exec_regime = if smooth_ac[1] > 1.8 {
            "⚡ [AC_HIGH_URGENCY_AGGRESSIVE_SWEEP]"
        } else if smooth_ac[0] > 3.5 {
            "🧊 [AC_HIGH_SLIPPAGE_PASSIVE_SLICE]"
        } else {
            "⚖️ [AC_BALANCED_TWAP_TRAJECTORY]"
        };

        println!(
            "🎯 [E40:{}] η_IMPACT:{:.2} | κ_URGENCY:{:.2} | SLICE_RATE:{:.1}% -> {}",
            symbol,
            smooth_ac[0],
            smooth_ac[1],
            smooth_ac[2] * 100.0,
            exec_regime
        );

        state[idx] = (price, sigma_sq, smooth_ac);
    }
}
