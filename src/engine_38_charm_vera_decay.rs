use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (anchor_strike, rolling_sigma, tau_cycle, cv_vec[charm, vera, bleed_flow])
static CV_STATE: Mutex<[(f64, f64, f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, 0.015, 1.0, Vector3::new(0.0, 0.0, 0.0)),
    (0.0, 0.015, 1.0, Vector3::new(0.0, 0.0, 0.0)),
    (0.0, 0.015, 1.0, Vector3::new(0.0, 0.0, 0.0)),
]);

// ENGINE 38: Tier-0 Charm (Delta Bleed) & Vera (Vega Decay) Time-Decay Dealer Flow Detector
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

    if let Ok(mut state) = CV_STATE.lock() {
        let (mut strike_k, mut sigma, mut tau, prev_cv) = state[idx];

        if strike_k <= 0.0 {
            state[idx] = (price, 0.015, 1.0, prev_cv);
            return;
        }

        let ret_abs = ((price - strike_k) / strike_k).abs();
        sigma = ((sigma * 0.90) + (ret_abs * 0.10)).max(0.0005);
        strike_k = (strike_k * 0.93) + (price * 0.07);

        // Rolling micro-horizon expiration decay cycle tau in [0.15, 1.0]
        tau = if tau <= 0.18 { 1.0 } else { tau - 0.04 };
        let sqrt_tau = tau.sqrt();

        let d1 = ((price / strike_k).ln() + (0.5 * sigma * sigma * tau)) / (sigma * sqrt_tau);
        let d2 = d1 - (sigma * sqrt_tau);
        let pdf_d1 = (-0.5 * d1 * d1).exp() * 0.39894228;

        // Charm = -N'(d1) * [ -d2 / (2 * tau) ] & Vera = -S * N'(d1) * sqrt(tau) * [0.5 + d1*d2/(2*tau)]
        let syn_charm = (pdf_d1 * (d2 / (2.0 * tau))).clamp(-25.0, 25.0);
        let syn_vera = (-pdf_d1 * sqrt_tau * (0.5 + (d1 * d2) / (2.0 * tau))).clamp(-25.0, 25.0);
        let bleed_pressure = (syn_charm * 1.5) + (syn_vera * 0.5);

        let inst_vec = Vector3::new(syn_charm, syn_vera, bleed_pressure);
        let smooth_cv = (prev_cv * 0.75) + (inst_vec * 0.25);

        let decay_regime = if smooth_cv[2] > 0.85 {
            "⏳ [CHARM_POSITIVE_DELTA_BUY_BLEED]"
        } else if smooth_cv[2] < -0.85 {
            "⏳ [CHARM_NEGATIVE_DELTA_SELL_BLEED]"
        } else {
            "📌 [THETA_CHARM_STRIKE_PINNING]"
        };

        println!(
            "⌛ [E38:{}] CHARM:{:+.2} | VERA:{:+.2} | BLEED:{:+.2} -> {}",
            symbol, smooth_cv[0], smooth_cv[1], smooth_cv[2], decay_regime
        );

        state[idx] = (strike_k, sigma, tau, smooth_cv);
    }
}
