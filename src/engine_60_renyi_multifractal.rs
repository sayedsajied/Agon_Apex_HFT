use nalgebra::Vector4;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, prob_vec4[deep_dn, mild_dn, mild_up, deep_up])
static RENYI_STATE: Mutex<[(f64, Vector4<f64>); 3]> = Mutex::new([
    (0.0, Vector4::new(0.25, 0.25, 0.25, 0.25)),
    (0.0, Vector4::new(0.25, 0.25, 0.25, 0.25)),
    (0.0, Vector4::new(0.25, 0.25, 0.25, 0.25)),
]);

// ENGINE 60: Tier-0 Renyi Generalized Multi-Fractal Entropy Spectrum & Collision Engine
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

    if let Ok(mut state) = RENYI_STATE.lock() {
        let (last_p, prev_p) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_p);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;

        // Soft-assign micro-return across 4 fractal velocity states [-2.0, -0.5, +0.5, +2.0]
        let centers = Vector4::new(-2.0, -0.5, 0.5, 2.0);
        let raw_w = centers.map(|c| (-0.45 * (ret_bps - c).powi(2)).exp());
        let inst_p = raw_w / raw_w.sum().max(1e-6);

        let mut p_vec = (prev_p * 0.78) + (inst_p * 0.22);
        p_vec /= p_vec.sum().max(1e-6);

        // Renyi Entropy of order alpha: H_alpha = (1 / (1 - alpha)) * ln( sum(P_i^alpha) )
        // Order alpha = 0.5 (Rare Tail-Support Spectrum)
        let sum_a05 = p_vec.map(|p| p.max(1e-6).sqrt()).sum();
        let h_05 = (1.0 / (1.0 - 0.5)) * sum_a05.ln();

        // Order alpha = 2.0 (Quadratic Collision Entropy: H_2 = -ln sum(P_i^2))
        let sum_a2 = p_vec.dot(&p_vec).max(1e-6);
        let h_2 = -sum_a2.ln();

        // Order alpha = 4.0 (High-Concentration Singularity Spectrum)
        let sum_a4 = p_vec.map(|p| p.powi(4)).sum().max(1e-9);
        let h_4 = (1.0 / (1.0 - 4.0)) * sum_a4.ln();

        // Multi-Fractal Spectrum Width: Delta_H = H_0.5 - H_4 & Directional Singularity Bias
        let spectrum_width = (h_05 - h_4).max(0.0);
        let dir_bias = (p_vec[2] + p_vec[3]) - (p_vec[0] + p_vec[1]);

        let renyi_regime = if spectrum_width > 0.35 && dir_bias > 0.30 {
            "💎 [RENYI_MULTIFRACTAL_BULL_SINGULARITY]"
        } else if spectrum_width > 0.35 && dir_bias < -0.30 {
            "💎 [RENYI_MULTIFRACTAL_BEAR_SINGULARITY]"
        } else {
            "🌀 [RENYI_MONOFRACTAL_DISPERSION]"
        };

        println!(
            "🔬 [E60:{}] H2_COLL:{:.3} | ΔH_SPEC:{:.3} | BIAS:{:+.2} -> {}",
            symbol, h_2, spectrum_width, dir_bias, renyi_regime
        );

        state[idx] = (price, p_vec);
    }
}
