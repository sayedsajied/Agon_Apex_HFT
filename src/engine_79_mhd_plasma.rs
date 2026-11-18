use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, mhd_vec[magnetic_b, alfven_va, reconnection_e])
static MHD_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.0, 1.0, 0.0)),
    (0.0, Vector3::new(0.0, 1.0, 0.0)),
    (0.0, Vector3::new(0.0, 1.0, 0.0)),
]);

// ENGINE 79: Tier-0 Magnetohydrodynamics (MHD), Alfvén Plasma Waves & Magnetic Reconnection Engine
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

    if let Ok(mut state) = MHD_STATE.lock() {
        let (last_p, prev_m) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_m);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let plasma_rho = (bid_sz + ask_sz).max(1.0).ln().clamp(0.25, 10.0);

        // Magnetic Field B (Orderbook Imbalance flux density) & Induction rate dB/dt
        let inst_b = ((bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1)) * 2.5;
        let magnetic_b = (prev_m[0] * 0.75) + (inst_b * 0.25);
        let db_dt = (magnetic_b - prev_m[0]).abs();

        // Alfvén Speed v_A = |B| / sqrt(mu_0 * rho)
        let inst_va = (magnetic_b.abs() / plasma_rho.sqrt()).clamp(0.05, 50.0);
        let alfven_va = (prev_m[1] * 0.82) + (inst_va * 0.18);

        // Sweet-Parker Magnetic Reconnection Electric Field E = eta*J + v x B (Sudden field line snap)
        let plasma_eta = (1.2 / (plasma_rho + 0.5)).clamp(0.1, 2.0);
        let inst_rec_e = (plasma_eta * db_dt) + (ret_bps.abs() * alfven_va * 0.05);
        let recon_e = (prev_m[2] * 0.70) + (inst_rec_e * 0.30);

        let next_m = Vector3::new(magnetic_b, alfven_va, recon_e);

        let mhd_regime = if recon_e > 1.80 && magnetic_b > 0.40 {
            "⚡ [MHD_MAGNETIC_RECONNECTION_BULL_JET]"
        } else if recon_e > 1.80 && magnetic_b < -0.40 {
            "🩸 [MHD_MAGNETIC_RECONNECTION_BEAR_JET]"
        } else if alfven_va > 3.50 {
            "🌌 [ALFVEN_SUPER_WAVE_PROPAGATION]"
        } else {
            "⚖️ [Mhd_MAGNETO_STATIC_EQUILIBRIUM]"
        };

        println!(
            "☀️ [E79:{}] B_FLUX:{:+.2} | v_A:{:.2} | E_RECON:{:.3} -> {}",
            symbol, next_m[0], next_m[1], next_m[2], mhd_regime
        );

        state[idx] = (price, next_m);
    }
}
