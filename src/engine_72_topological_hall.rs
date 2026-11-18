use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, d_vec[dx_momentum, dy_book_flow, dz_mass_gap])
static TOPO_HALL_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.1, 0.1, 1.0)),
    (0.0, Vector3::new(0.1, 0.1, 1.0)),
    (0.0, Vector3::new(0.1, 0.1, 1.0)),
]);

// ENGINE 72: Tier-0 Topological Insulator, Berry Curvature & Quantum Hall Edge-State Engine
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

    if let Ok(mut state) = TOPO_HALL_STATE.lock() {
        let (last_p, prev_d) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_d);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let book_imb = (bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1);

        // Bloch Hamiltonian vector d(k) = [dx, dy, dz]: dz < 0 triggers Topological Band Inversion
        let dx = (prev_d[0] * 0.75) + (ret_bps * 0.25);
        let dy = (prev_d[1] * 0.75) + ((book_imb * 2.0) * 0.25);
        let inst_gap = 0.85 - (ret_bps.abs() * 0.55) - (book_imb.abs() * 0.65);
        let dz = ((prev_d[2] * 0.82) + (inst_gap * 0.18)).clamp(-2.5, 2.5);

        let next_d = Vector3::new(dx, dy, dz);
        let norm_d = next_d.norm().max(1e-4);

        // Berry Curvature Flux Omega_z = (d . (prev_d x next_d)) / |d|^3 & Quantized Hall Conductivity
        let cross_flow = prev_d.cross(&next_d);
        let berry_omega = next_d.dot(&cross_flow) / (norm_d * norm_d * norm_d);
        let chern_proxy = if dz < 0.0 { dx.signum() } else { 0.0 };
        let hall_sigma_xy = (chern_proxy * 0.70) + (berry_omega.clamp(-1.5, 1.5) * 0.30);

        let topo_regime = if dz < -0.10 && hall_sigma_xy > 0.45 {
            "⚡ [TOPO_CHIRAL_EDGE_STATE_BULL_SUPERCURRENT]"
        } else if dz < -0.10 && hall_sigma_xy < -0.45 {
            "🩸 [TOPO_CHIRAL_EDGE_STATE_BEAR_SUPERCURRENT]"
        } else if dz > 0.35 {
            "🧱 [TOPO_BULK_INSULATOR_GAPPED_CHOP]"
        } else {
            "🌀 [DIRAC_CONE_GAP_CLOSING_TRANSITION]"
        };

        println!(
            "🍩 [E72:{}] σ_XY_HALL:{:+.3} | Δ_GAP(dz):{:+.3} | Ω_BERRY:{:+.4} -> {}",
            symbol, hall_sigma_xy, next_d[2], berry_omega, topo_regime
        );

        state[idx] = (price, next_d);
    }
}
