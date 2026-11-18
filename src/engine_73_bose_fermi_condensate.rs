use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, q_stat_vec[temp_t, chem_mu, bec_fraction])
static BOSE_FERMI_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(1.0, -0.5, 0.0)),
    (0.0, Vector3::new(1.0, -0.5, 0.0)),
    (0.0, Vector3::new(1.0, -0.5, 0.0)),
]);

// ENGINE 73: Tier-0 Fermi-Dirac Pauli Exclusion & Bose-Einstein Liquidity Condensation Engine
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

    if let Ok(mut state) = BOSE_FERMI_STATE.lock() {
        let (last_p, prev_q) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_q);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let book_imb = (bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1);

        // Effective thermal dispersion T & Chemical Potential mu -> 0_minus (approaching ground state epsilon_0 = 0)
        let inst_t = (ret_bps.abs() * 0.6 + (1.0 - book_imb.abs()) * 0.8).clamp(0.10, 4.0);
        let temp_t = (prev_q[0] * 0.82) + (inst_t * 0.18);

        // High directional crowding drives chemical potential mu toward 0 from below (-0.02)
        let crowding = (ret_bps.abs() * 0.45) + (book_imb.abs() * 0.85);
        let inst_mu = (-1.20 / (1.0 + crowding * 2.5)).clamp(-1.50, -0.02);
        let chem_mu = (prev_q[1] * 0.78) + (inst_mu * 0.22);

        // Ground-state (epsilon_0 = 0) Bose-Einstein occupation N_0 vs Fermi-Dirac Pauli Pressure N_F
        let x_arg = (-chem_mu / temp_t).clamp(0.02, 10.0);
        let bose_n0 = 1.0 / (x_arg.exp() - 1.0).max(0.02);
        let fermi_nf = 1.0 / (x_arg.exp() + 1.0);

        let bec_ratio = (bose_n0 / (bose_n0 + fermi_nf + 2.0)).clamp(0.0, 0.99);
        let smooth_bec = (prev_q[2] * 0.72) + (bec_ratio * 0.28);
        let next_q = Vector3::new(temp_t, chem_mu, smooth_bec);

        let q_regime = if smooth_bec > 0.62 && (ret_bps + book_imb) > 0.25 {
            "⚡ [BOSE_EINSTEIN_CONDENSATE_BULL_SUPERFLUID]"
        } else if smooth_bec > 0.62 && (ret_bps + book_imb) < -0.25 {
            "🩸 [BOSE_EINSTEIN_CONDENSATE_BEAR_AVALANCHE]"
        } else if fermi_nf > 0.42 && smooth_bec < 0.35 {
            "🧱 [FERMI_DIRAC_PAULI_EXCLUSION_QUEUE_LOCK]"
        } else {
            "♨️ [MAXWELL_BOLTZMANN_CLASSICAL_GAS]"
        };

        println!(
            "🧊 [E73:{}] BEC_f:{:.1}% | μ_CHEM:{:+.3} | FERMI_P:{:.2} -> {}",
            symbol, next_q[2] * 100.0, next_q[1], fermi_nf, q_regime
        );

        state[idx] = (price, next_q);
    }
}
