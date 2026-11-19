use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (anchor_p, qcd_vec[gluon_a, alpha_s, ym_action_f2])
static QCD_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.0, 0.5, 0.1)),
    (0.0, Vector3::new(0.0, 0.5, 0.1)),
    (0.0, Vector3::new(0.0, 0.5, 0.1)),
]);

// ENGINE 80: Tier-0 Quantum Chromodynamics (QCD), Asymptotic Freedom & Yang-Mills Gauge Field Engine
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

    if let Ok(mut state) = QCD_STATE.lock() {
        let (mut anchor_p, prev_q) = state[idx];

        if anchor_p <= 0.0 {
            state[idx] = (price, prev_q);
            return;
        }

        let disp_bps = ((price - anchor_p) / anchor_p) * 10000.0;
        anchor_p = (anchor_p * 0.92) + (price * 0.08);
        let color_imb = (bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1);

        // Running Strong Coupling alpha_s(Q^2) = 1 / (b0 * ln(Q^2 / Lambda^2)): drops at high momentum (Asymptotic Freedom)
        let q_sq = (disp_bps * disp_bps + color_imb * color_imb + 1.5).max(1.5);
        let inst_alpha_s = (1.15 / q_sq.ln().max(0.35)).clamp(0.08, 2.50);
        let alpha_s = (prev_q[1] * 0.80) + (inst_alpha_s * 0.20);

        // Non-Abelian Gluon Gauge Potential A_mu & Self-Interaction Field Strength F_mu_nu = dA + g * (A x A)
        let inst_a = (disp_bps * 0.45) + (color_imb * 1.40);
        let gluon_a = (prev_q[0] * 0.72) + (inst_a * 0.28);
        let da_dt = gluon_a - prev_q[0];

        // Yang-Mills Lagrangian Action Density: L_YM = 0.25 * F^2 (includes non-linear g * A^2 self-coupling)
        let f_tensor = da_dt + (alpha_s * gluon_a * color_imb);
        let inst_f2 = (0.25 * f_tensor * f_tensor).clamp(0.01, 30.0);
        let ym_action = (prev_q[2] * 0.75) + (inst_f2 * 0.25);

        let next_q = Vector3::new(gluon_a, alpha_s, ym_action);

        let qcd_regime = if ym_action > 1.25 && alpha_s < 0.45 && gluon_a > 0.35 {
            "⚡ [QCD_QUARK_GLUON_PLASMA_BULL_DECONFINEMENT]"
        } else if ym_action > 1.25 && alpha_s < 0.45 && gluon_a < -0.35 {
            "🩸 [QCD_QUARK_GLUON_PLASMA_BEAR_DECONFINEMENT]"
        } else if alpha_s > 0.95 {
            "🔗 [QCD_COLOR_CONFINEMENT_FLUX_TUBE_LOCK]"
        } else {
            "⚛️ [YANG_MILLS_GAUGE_VACUUM_EQUILIBRIUM]"
        };

        println!(
            "⚛️ [E80:{}] α_s:{:.3} | L_YM:{:.3} | A_GLUON:{:+.2} -> {}",
            symbol, next_q[1], next_q[2], next_q[0], qcd_regime
        );

        state[idx] = (anchor_p, next_q);
    }
}
