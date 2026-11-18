use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (anchor_price, qho_vec[disp_x, mom_p, quantum_level_n])
static QHO_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.0, 0.0, 0.0)),
    (0.0, Vector3::new(0.0, 0.0, 0.0)),
    (0.0, Vector3::new(0.0, 0.0, 0.0)),
]);

// ENGINE 66: Tier-0 Quantum Harmonic Oscillator, Dirac Ladder Operators & Ground-State Well Engine
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

    if let Ok(mut state) = QHO_STATE.lock() {
        let (mut anchor_p, prev_qho) = state[idx];

        if anchor_p <= 0.0 {
            state[idx] = (price, prev_qho);
            return;
        }

        // Displacement x from dynamic harmonic well center & canonical momentum p
        let disp_x = ((price - anchor_p) / anchor_p) * 10000.0;
        let mom_p = disp_x - prev_qho[0];
        anchor_p = (anchor_p * 0.92) + (price * 0.08);

        // Harmonic well stiffness omega derived from orderbook restoring depth
        let book_stiffness = ((bid_sz + ask_sz).max(1.0).ln() * 0.35).clamp(0.40, 3.0);
        let omega = book_stiffness;
        let hbar = 0.50;

        // Dirac Ladder Operator occupation number: n = (m*omega*x^2 + p^2/(m*omega)) / (2*hbar) - 0.5
        let hamiltonian_ratio = ((omega * disp_x * disp_x) + (mom_p * mom_p / omega)) / (2.0 * hbar);
        let inst_n = (hamiltonian_ratio - 0.5).clamp(0.0, 15.0);
        let smooth_n = (prev_qho[2] * 0.72) + (inst_n * 0.28);

        // Quantized Eigen-Energy E_n = hbar * omega * (n + 0.5)
        let energy_en = hbar * omega * (smooth_n + 0.5);
        let next_qho = Vector3::new(disp_x, mom_p, smooth_n);

        let qho_regime = if smooth_n > 2.4 && disp_x > 0.35 {
            "🚀 [QHO_LADDER_EXCITATION_BULL_ESCAPE]"
        } else if smooth_n > 2.4 && disp_x < -0.35 {
            "🩸 [QHO_LADDER_EXCITATION_BEAR_ESCAPE]"
        } else if smooth_n < 0.45 {
            "🧊 [QHO_ZERO_POINT_GROUND_STATE_COIL]"
        } else {
            "〰️ [QHO_BOUND_HARMONIC_OSCILLATION]"
        };

        println!(
            "⚛️ [E66:{}] LEVEL_n:{:.2} | E_n:{:.2} | X_WELL:{:+.2}bps -> {}",
            symbol, next_qho[2], energy_en, next_qho[0], qho_regime
        );

        state[idx] = (anchor_p, next_qho);
    }
}
