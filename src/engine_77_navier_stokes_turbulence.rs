use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, ns_vec[vel_u, reynolds_re, vorticity_omega])
static NAVIER_STOKES_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.0, 100.0, 0.0)),
    (0.0, Vector3::new(0.0, 100.0, 0.0)),
    (0.0, Vector3::new(0.0, 100.0, 0.0)),
]);

// ENGINE 77: Tier-0 Navier-Stokes Fluid Turbulence, Reynolds Number & Vortex Shedding Engine
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

    if let Ok(mut state) = NAVIER_STOKES_STATE.lock() {
        let (last_p, prev_ns) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_ns);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let rho_density = (bid_sz + ask_sz).max(1.0).ln().clamp(0.5, 10.0);

        // Fluid Pressure Gradient -grad(P) from bid/ask wall imbalance & Dynamic Viscosity mu
        let neg_grad_p = ((bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1)) * 1.5;
        let mu_viscosity = (1.8 / (ret_bps.abs() + 0.5)).clamp(0.10, 3.5);

        // Navier-Stokes 1D Advection-Diffusion step: du/dt = -u*(du/dx) - (1/rho)*grad(P) + nu*laplacian(u)
        let u_prev = prev_ns[0];
        let du_advect = -0.15 * u_prev * (ret_bps - u_prev).abs();
        let viscous_laplacian = (mu_viscosity / rho_density) * (ret_bps - u_prev);
        let u_next = ((u_prev * 0.65) + ((ret_bps + (neg_grad_p / rho_density) + du_advect + viscous_laplacian) * 0.35)).clamp(-15.0, 15.0);

        // Reynolds Number Re = (rho * |u| * L) / mu & Shear Vorticity Curl omega = du_y / dx
        let inst_re = ((rho_density * u_next.abs().max(0.05) * 120.0) / mu_viscosity).clamp(10.0, 5000.0);
        let re_num = (prev_ns[1] * 0.78) + (inst_re * 0.22);
        let vorticity = (prev_ns[2] * 0.72) + (((u_next - u_prev) * neg_grad_p) * 0.28);

        let next_ns = Vector3::new(u_next, re_num, vorticity);

        let ns_regime = if re_num > 450.0 && u_next > 0.45 {
            "🌪️ [NAVIER_STOKES_TURBULENT_VORTEX_BULL_JET]"
        } else if re_num > 450.0 && u_next < -0.45 {
            "🌪️ [NAVIER_STOKES_TURBULENT_VORTEX_BEAR_JET]"
        } else if re_num < 160.0 {
            "🌊 [NAVIER_STOKES_LAMINAR_VISCOUS_FLOW]"
        } else {
            "🌀 [KARMAN_VORTEX_STREET_TRANSITION]"
        };

        println!(
            "🌊 [E77:{}] Re_NUM:{:.0} | u_VEL:{:+.2} | ω_VORTEX:{:+.3} -> {}",
            symbol, next_ns[1], next_ns[0], next_ns[2], ns_regime
        );

        state[idx] = (price, next_ns);
    }
}
