use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (gravity_center_p, gr_vec[r_schwarzschild, time_dilation, escape_ratio])
static RELATIVITY_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(0.5, 1.0, 0.5)),
    (0.0, Vector3::new(0.5, 1.0, 0.5)),
    (0.0, Vector3::new(0.5, 1.0, 0.5)),
]);

// ENGINE 81: Tier-0 General Relativity, Einstein Field Curvature & Schwarzschild Event Horizon Engine
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

    if let Ok(mut state) = RELATIVITY_STATE.lock() {
        let (mut grav_center, prev_gr) = state[idx];

        if grav_center <= 0.0 {
            state[idx] = (price, prev_gr);
            return;
        }

        // Radial coordinate r (distance from liquidity mass center in bps) & Stress-Energy Mass M
        let signed_r = ((price - grav_center) / grav_center) * 10000.0;
        let radial_r = signed_r.abs().max(0.15);
        grav_center = (grav_center * 0.93) + (price * 0.07);

        let stress_mass_m = ((bid_sz + ask_sz).max(1.0).ln() * 0.45).clamp(0.20, 6.0);
        let c_speed = 2.20;

        // Schwarzschild Event Horizon Radius: r_s = 2 * G * M / c^2
        let inst_rs = ((2.0 * stress_mass_m) / (c_speed * c_speed)).clamp(0.10, 4.5);
        let r_s = (prev_gr[0] * 0.80) + (inst_rs * 0.20);

        // Gravitational Time Dilation factor: gamma_g = sqrt(max(1 - r_s / r, 0.01))
        let inst_dilation = (1.0 - (r_s / radial_r)).clamp(0.01, 1.0).sqrt();
        let time_dil = (prev_gr[1] * 0.75) + (inst_dilation * 0.25);

        // Escape Velocity v_esc = sqrt(2GM / r) & Kinetic Escape Ratio |v| / v_esc
        let v_esc = ((2.0 * stress_mass_m) / radial_r).sqrt().max(0.10);
        let book_thrust = (bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1);
        let kinetic_v = (signed_r.abs() * 0.65) + (book_thrust.abs() * 1.15);
        let escape_ratio = (prev_gr[2] * 0.72) + ((kinetic_v / v_esc).clamp(0.05, 5.0) * 0.28);

        let next_gr = Vector3::new(r_s, time_dil, escape_ratio);

        let gr_regime = if escape_ratio > 1.35 && signed_r > 0.30 {
            "🚀 [RELATIVISTIC_ESCAPE_VELOCITY_BULL_BREAKOUT]"
        } else if escape_ratio > 1.35 && signed_r < -0.30 {
            "🩸 [RELATIVISTIC_ESCAPE_VELOCITY_BEAR_BREAKOUT]"
        } else if radial_r <= r_s * 1.15 || time_dil < 0.35 {
            "🕳️ [SCHWARZSCHILD_EVENT_HORIZON_GRAVITY_TRAP]"
        } else {
            "🌌 [EINSTEIN_GEODESIC_ORBIT_EQUILIBRIUM]"
        };

        println!(
            "🕳️ [E81:{}] r_s:{:.2} | γ_TIME:{:.2} | v/v_ESC:{:.2} -> {}",
            symbol, next_gr[0], next_gr[1], next_gr[2], gr_regime
        );

        state[idx] = (grav_center, next_gr);
    }
}
