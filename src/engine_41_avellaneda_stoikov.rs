use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, rolling_var, as_vec[res_skew_bps, opt_spread_bps, inv_q])
static AS_STATE: Mutex<[(f64, f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, 1.0, Vector3::new(0.0, 2.0, 0.0)),
    (0.0, 1.0, Vector3::new(0.0, 2.0, 0.0)),
    (0.0, 1.0, Vector3::new(0.0, 2.0, 0.0)),
]);

// ENGINE 41: Tier-0 Avellaneda-Stoikov Reservation Price & Optimal Market-Making Spread Engine
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
        .unwrap_or(1.0);
    let ask_sz = quotes
        .and_then(|q| q.get("ask_size"))
        .and_then(|a| a.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0);

    if price <= 0.0 {
        return;
    }

    if let Ok(mut state) = AS_STATE.lock() {
        let (last_p, prev_var, prev_as) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, 1.0, prev_as);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let sigma_sq = ((prev_var * 0.88) + (ret_bps * ret_bps * 0.12)).max(0.05);

        // Synthetic Dealer Inventory Pressure q in [-1.0, 1.0] & Order Book Liquidity Intensity k
        let total_depth = (bid_sz + ask_sz).max(0.1);
        let inv_q = (ask_sz - bid_sz) / total_depth; // Positive q when dealers absorb heavy sell pressure
        let kappa_liq = (total_depth.ln().max(0.5)) * 1.5;

        // Avellaneda-Stoikov Risk Aversion gamma = 0.25 over normalized horizon tau = 1.0
        let gamma = 0.25;
        // Reservation Price Skew in bps: (r - s) = -q * gamma * sigma^2
        let res_skew_bps = -inv_q * gamma * sigma_sq;

        // Optimal Spread: delta* = gamma * sigma^2 + (2 / gamma) * ln(1 + gamma / kappa)
        let opt_spread_bps = (gamma * sigma_sq) + ((2.0 / gamma) * (1.0 + (gamma / kappa_liq)).ln());

        let inst_vec = Vector3::new(res_skew_bps, opt_spread_bps, inv_q);
        let smooth_as = (prev_as * 0.75) + (inst_vec * 0.25);

        let mm_regime = if smooth_as[0] > 0.45 {
            "🟢 [AS_RESERVATION_SKEW_UP_BID_CHASE]"
        } else if smooth_as[0] < -0.45 {
            "🔴 [AS_RESERVATION_SKEW_DOWN_ASK_DUMP]"
        } else {
            "⚖️ [AS_SYMMETRIC_MARKET_MAKING]"
        };

        println!(
            "🏦 [E41:{}] RES_SKEW:{:+.2}bps | OPT_SPREAD:{:.2}bps | INV_Q:{:+.2} -> {}",
            symbol, smooth_as[0], smooth_as[1], smooth_as[2], mm_regime
        );

        state[idx] = (price, sigma_sq, smooth_as);
    }
}
