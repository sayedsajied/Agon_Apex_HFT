use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (p_t2, p_t1, taylor_vec[vel, gamma, proj_bps])
static TAYLOR_STATE: Mutex<[(f64, f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, 0.0, Vector3::new(0.0, 0.0, 0.0)),
    (0.0, 0.0, Vector3::new(0.0, 0.0, 0.0)),
    (0.0, 0.0, Vector3::new(0.0, 0.0, 0.0)),
]);

// ENGINE 31: Tier-0 Taylor Series Second-Order Convexity & Gamma Price Accelerator
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

    if let Ok(mut state) = TAYLOR_STATE.lock() {
        let (p_t2, p_t1, prev_vec) = state[idx];

        if p_t1 <= 0.0 || p_t2 <= 0.0 {
            state[idx] = (p_t1, price, prev_vec);
            return;
        }

        // First-order Delta Velocity (dP) and Second-order Gamma Convexity (d^2P) in bps
        let vel_bps = ((price - p_t1) / p_t1) * 10000.0;
        let prev_vel_bps = ((p_t1 - p_t2) / p_t2) * 10000.0;
        let gamma_bps = vel_bps - prev_vel_bps;

        // 2nd-Order Taylor Expansion Step Projection: dP_next = v*dt + 0.5*Gamma*dt^2
        let taylor_step = vel_bps + (0.5 * gamma_bps);
        let inst_vec = Vector3::new(vel_bps, gamma_bps, taylor_step);
        let smooth_vec = (prev_vec * 0.75) + (inst_vec * 0.25);

        let convexity_regime = if smooth_vec[0] > 0.8 && smooth_vec[1] > 0.5 {
            "🚀 [POSITIVE_GAMMA_CONVEX_THRUST]"
        } else if smooth_vec[0] < -0.8 && smooth_vec[1] < -0.5 {
            "🩸 [NEGATIVE_GAMMA_CONCAVE_FLUSH]"
        } else {
            "📐 [LINEAR_FIRST_ORDER_DRIFT]"
        };

        println!(
            "🎢 [E31:{}] Δ_VEL:{:+.2}bps | Γ_ACCEL:{:+.2}bps | TAYLOR_PROJ:{:+.2}bps -> {}",
            symbol, smooth_vec[0], smooth_vec[1], smooth_vec[2], convexity_regime
        );

        state[idx] = (p_t1, price, smooth_vec);
    }
}
