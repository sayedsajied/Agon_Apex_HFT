use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (anchor_strike, rolling_sigma, vv_vec[vanna, volga, flow_pressure])
static VV_STATE: Mutex<[(f64, f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, 0.015, Vector3::new(0.0, 0.0, 0.0)),
    (0.0, 0.015, Vector3::new(0.0, 0.0, 0.0)),
    (0.0, 0.015, Vector3::new(0.0, 0.0, 0.0)),
]);

// ENGINE 37: Tier-0 Vanna & Volga Second-Order Cross-Greek Convexity Flow
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

    if let Ok(mut state) = VV_STATE.lock() {
        let (mut strike_k, prev_sigma, prev_vv) = state[idx];

        if strike_k <= 0.0 {
            state[idx] = (price, 0.015, prev_vv);
            return;
        }

        let ret_signed = (price - strike_k) / strike_k;
        let sigma = ((prev_sigma * 0.88) + (ret_signed.abs() * 0.12)).max(0.0005);
        let d_sigma = sigma - prev_sigma;
        strike_k = (strike_k * 0.90) + (price * 0.10);

        // BSM d1 and d2 parameters over normalized micro-horizon
        let d1 = ((price / strike_k).ln() + (0.5 * sigma * sigma)) / sigma;
        let d2 = d1 - sigma;
        let pdf_d1 = (-0.5 * d1 * d1).exp() * 0.39894228;

        // Vanna = -N'(d1) * (d2 / sigma) & Volga (Vomma) = Vega * (d1 * d2 / sigma)
        let syn_vanna = (-pdf_d1 * (d2 / sigma)).clamp(-50.0, 50.0);
        let syn_volga = (pdf_d1 * d1 * d2).clamp(-50.0, 50.0);

        // Cross-Greek Dealer Re-hedging Pressure triggered by volatility shift
        let vanna_flow = (syn_vanna * d_sigma * 10000.0) + (ret_signed * 1000.0);
        let inst_vec = Vector3::new(syn_vanna, syn_volga, vanna_flow);
        let smooth_vv = (prev_vv * 0.75) + (inst_vec * 0.25);

        let vv_regime = if smooth_vv[2] > 1.2 {
            "🔥 [VANNA_VOLGA_BULLISH_GAMMA_RAMP]"
        } else if smooth_vv[2] < -1.2 {
            "🩸 [VANNA_VOLGA_BEARISH_DEALER_PUKE]"
        } else {
            "⚖️ [SECOND_ORDER_GREEK_BALANCE]"
        };

        println!(
            "🧬 [E37:{}] VANNA:{:+.2} | VOLGA:{:+.2} | VV_FLOW:{:+.2} -> {}",
            symbol, smooth_vv[0], smooth_vv[1], smooth_vv[2], vv_regime
        );

        state[idx] = (strike_k, sigma, smooth_vv);
    }
}
