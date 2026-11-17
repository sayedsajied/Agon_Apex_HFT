use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (anchor_strike, rolling_sigma, sz_vec[speed, zomma, squeeze_pressure])
static SZ_STATE: Mutex<[(f64, f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, 0.015, Vector3::new(0.0, 0.0, 0.0)),
    (0.0, 0.015, Vector3::new(0.0, 0.0, 0.0)),
    (0.0, 0.015, Vector3::new(0.0, 0.0, 0.0)),
]);

// ENGINE 39: Tier-0 Speed & Zomma Third-Order Gamma-Squeeze & Convexity Accelerator
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

    if let Ok(mut state) = SZ_STATE.lock() {
        let (mut strike_k, mut sigma, prev_sz) = state[idx];

        if strike_k <= 0.0 {
            state[idx] = (price, 0.015, prev_sz);
            return;
        }

        let ret_signed = (price - strike_k) / strike_k;
        sigma = ((sigma * 0.88) + (ret_signed.abs() * 0.12)).max(0.0005);
        strike_k = (strike_k * 0.92) + (price * 0.08);

        let d1 = ((price / strike_k).ln() + (0.5 * sigma * sigma)) / sigma;
        let d2 = d1 - sigma;
        let pdf_d1 = (-0.5 * d1 * d1).exp() * 0.39894228;

        // Base Gamma = N'(d1) / (S * sigma)
        let gamma = pdf_d1 / (price * sigma);

        // Speed = -Gamma / S * (d1 / sigma + 1) & Zomma = Gamma * (d1 * d2 - 1) / sigma
        let syn_speed = (-gamma / price * (d1 / sigma + 1.0) * 100000.0).clamp(-100.0, 100.0);
        let syn_zomma = (gamma * (d1 * d2 - 1.0) / sigma).clamp(-50.0, 50.0);
        let squeeze_score = (syn_speed * 0.6) + (syn_zomma * 0.4) + (ret_signed * 500.0);

        let inst_vec = Vector3::new(syn_speed, syn_zomma, squeeze_score);
        let smooth_sz = (prev_sz * 0.70) + (inst_vec * 0.30);

        let sz_regime = if smooth_sz[2] > 2.5 {
            "🚀 [THIRD_ORDER_EXPLOSIVE_GAMMA_SQUEEZE]"
        } else if smooth_sz[2] < -2.5 {
            "🩸 [THIRD_ORDER_DEALER_UNWIND_FLUSH]"
        } else {
            "⚖️ [THIRD_ORDER_NEUTRAL_CONVEXITY]"
        };

        println!(
            "⚡ [E39:{}] SPEED:{:+.2} | ZOMMA:{:+.2} | SQUEEZE:{:+.2} -> {}",
            symbol, smooth_sz[0], smooth_sz[1], smooth_sz[2], sz_regime
        );

        state[idx] = (strike_k, sigma, smooth_sz);
    }
}
