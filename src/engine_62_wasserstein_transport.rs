use nalgebra::Vector4;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, fast_mass_p, slow_mass_q)
static WASSERSTEIN_STATE: Mutex<[(f64, Vector4<f64>, Vector4<f64>); 3]> = Mutex::new([
    (0.0, Vector4::new(0.25, 0.25, 0.25, 0.25), Vector4::new(0.25, 0.25, 0.25, 0.25)),
    (0.0, Vector4::new(0.25, 0.25, 0.25, 0.25), Vector4::new(0.25, 0.25, 0.25, 0.25)),
    (0.0, Vector4::new(0.25, 0.25, 0.25, 0.25), Vector4::new(0.25, 0.25, 0.25, 0.25)),
]);

// ENGINE 62: Tier-0 Wasserstein Optimal Transport & Earth Mover's Distance (EMD) Engine
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

    if let Ok(mut state) = WASSERSTEIN_STATE.lock() {
        let (last_p, prev_p, prev_q) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_p, prev_q);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let book_skew = (bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1);
        let combined_impulse = ret_bps + (book_skew * 0.8);

        // Project impulse onto 4 metric space grid nodes [-2.0, -0.6, +0.6, +2.0]
        let grid = Vector4::new(-2.0, -0.6, 0.6, 2.0);
        let raw_w = grid.map(|g| (-0.45 * (combined_impulse - g).powi(2)).exp());
        let inst_mass = raw_w / raw_w.sum().max(1e-6);

        let mut p_fast = (prev_p * 0.58) + (inst_mass * 0.42);
        let mut q_slow = (prev_q * 0.93) + (inst_mass * 0.07);
        p_fast /= p_fast.sum().max(1e-6);
        q_slow /= q_slow.sum().max(1e-6);

        // Kantorovich-Rubinstein 1D Closed-Form Wasserstein Distance: W_1(P, Q) = sum |CDF_P(i) - CDF_Q(i)| * dx
        let mut cdf_p = 0.0;
        let mut cdf_q = 0.0;
        let mut w1_emd = 0.0;
        let mut signed_flux = 0.0;
        for i in 0..3 {
            cdf_p += p_fast[i];
            cdf_q += q_slow[i];
            let step_dx = grid[i + 1] - grid[i];
            let diff_cdf = cdf_q - cdf_p; // Positive when P shifts mass rightward (bullish) relative to Q
            w1_emd += diff_cdf.abs() * step_dx;
            signed_flux += diff_cdf * step_dx;
        }

        let ot_regime = if w1_emd > 0.22 && signed_flux > 0.15 {
            "🚜 [WASSERSTEIN_BULL_MASS_TRANSPORT]"
        } else if w1_emd > 0.22 && signed_flux < -0.15 {
            "🚜 [WASSERSTEIN_BEAR_MASS_TRANSPORT]"
        } else {
            "⚖️ [WASSERSTEIN_ZERO_WORK_EQUILIBRIUM]"
        };

        println!(
            "🚛 [E62:{}] W1_EMD:{:.4} | OT_FLUX:{:+.4} | BOOK_SKEW:{:+.2} -> {}",
            symbol, w1_emd, signed_flux, book_skew, ot_regime
        );

        state[idx] = (price, p_fast, q_slow);
    }
}
