use nalgebra::Vector4;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, fast_posterior_p, slow_prior_q)
static KL_STATE: Mutex<[(f64, Vector4<f64>, Vector4<f64>); 3]> = Mutex::new([
    (0.0, Vector4::new(0.25, 0.25, 0.25, 0.25), Vector4::new(0.25, 0.25, 0.25, 0.25)),
    (0.0, Vector4::new(0.25, 0.25, 0.25, 0.25), Vector4::new(0.25, 0.25, 0.25, 0.25)),
    (0.0, Vector4::new(0.25, 0.25, 0.25, 0.25), Vector4::new(0.25, 0.25, 0.25, 0.25)),
]);

// ENGINE 61: Tier-0 Kullback-Leibler (KL) Divergence & Jeffreys Relative Entropy Shift Detector
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

    if let Ok(mut state) = KL_STATE.lock() {
        let (last_p, prev_p, prev_q) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_p, prev_q);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;

        // Soft-map micro-return across 4 canonical regime bins [-2.0, -0.5, +0.5, +2.0]
        let centers = Vector4::new(-2.0, -0.5, 0.5, 2.0);
        let raw_w = centers.map(|c| (-0.50 * (ret_bps - c).powi(2)).exp());
        let inst_dist = raw_w / raw_w.sum().max(1e-6);

        // Fast Posterior P (reacts to immediate shock) vs Slow Prior Q (baseline memory)
        let mut p_fast = (prev_p * 0.60) + (inst_dist * 0.40);
        let mut q_slow = (prev_q * 0.94) + (inst_dist * 0.06);
        p_fast /= p_fast.sum().max(1e-6);
        q_slow /= q_slow.sum().max(1e-6);

        // Forward KL: D_KL(P || Q) = sum( P_i * ln(P_i / Q_i) ) & Reverse KL: D_KL(Q || P)
        let mut d_kl_pq = 0.0;
        let mut d_kl_qp = 0.0;
        for i in 0..4 {
            let pi = p_fast[i].max(1e-6);
            let qi = q_slow[i].max(1e-6);
            d_kl_pq += pi * (pi / qi).ln();
            d_kl_qp += qi * (qi / pi).ln();
        }

        // Jeffreys Symmetrized Divergence J_KL = D_KL(P || Q) + D_KL(Q || P)
        let jeffreys_div = (d_kl_pq + d_kl_qp).max(0.0);
        let info_dir = ((p_fast[2] + p_fast[3]) - (q_slow[2] + q_slow[3])) * 2.0;

        let kl_regime = if jeffreys_div > 0.12 && info_dir > 0.10 {
            "🚀 [KL_INFO_SHOCK_BULL_REGIME_SHIFT]"
        } else if jeffreys_div > 0.12 && info_dir < -0.10 {
            "🩸 [KL_INFO_SHOCK_BEAR_REGIME_SHIFT]"
        } else {
            "⚖️ [KL_PRIOR_POSTERIOR_EQUILIBRIUM]"
        };

        println!(
            "📡 [E61:{}] D_KL(P||Q):{:.4} | J_SYM:{:.4} | ΔINFO:{:+.3} -> {}",
            symbol, d_kl_pq, jeffreys_div, info_dir, kl_regime
        );

        state[idx] = (price, p_fast, q_slow);
    }
}
