use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (anchor_strike, rolling_vol, greeks_vec[delta, vega, impulse])
static BSM_STATE: Mutex<[(f64, f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, 0.015, Vector3::new(0.5, 0.0, 0.0)),
    (0.0, 0.015, Vector3::new(0.5, 0.0, 0.0)),
    (0.0, 0.015, Vector3::new(0.5, 0.0, 0.0)),
]);

// ENGINE 36: Tier-0 Black-Scholes-Merton (BSM) Synthetic Delta & Vega Dealer Hedge Impulse
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

    if let Ok(mut state) = BSM_STATE.lock() {
        let (mut strike_k, mut sigma, prev_g) = state[idx];

        if strike_k <= 0.0 {
            state[idx] = (price, 0.015, prev_g);
            return;
        }

        // Update synthetic micro-horizon ATM strike & local realized volatility
        let ret_abs = ((price - strike_k) / strike_k).abs();
        sigma = ((sigma * 0.90) + (ret_abs * 0.10)).max(0.0005);
        strike_k = (strike_k * 0.92) + (price * 0.08);

        // BSM d1 parameter over normalized micro-horizon tau = 1.0
        let d1 = ((price / strike_k).ln() + (0.5 * sigma * sigma)) / sigma;

        // Fast Abramowitz-Stegun logistic approximation for Normal CDF N(d1) and PDF N'(d1)
        let syn_delta = 1.0 / (1.0 + (-1.702 * d1).exp());
        let pdf_d1 = (-0.5 * d1 * d1).exp() * 0.39894228;
        let syn_vega = pdf_d1 * sigma * 10000.0;

        // Dealer Delta-Hedging Impulse: change in Delta * Vega pressure
        let delta_shift = syn_delta - prev_g[0];
        let hedge_impulse = (delta_shift * 100.0) + (syn_vega * d1.signum() * 0.1);
        let g_vec = Vector3::new(syn_delta, syn_vega, hedge_impulse);

        let bsm_regime = if g_vec[0] > 0.72 && g_vec[2] > 1.5 {
            "🚀 [BSM_DEALER_LONG_DELTA_SQUEEZE]"
        } else if g_vec[0] < 0.28 && g_vec[2] < -1.5 {
            "🩸 [BSM_DEALER_SHORT_DELTA_FLUSH]"
        } else {
            "⚖️ [BSM_DELTA_NEUTRAL_PIN]"
        };

        println!(
            "🛡️ [E36:{}] BSM_Δ:{:.3} | VEGA_ν:{:.2} | IMPULSE:{:+.2} -> {}",
            symbol, g_vec[0], g_vec[1], g_vec[2], bsm_regime
        );

        state[idx] = (strike_k, sigma, g_vec);
    }
}
