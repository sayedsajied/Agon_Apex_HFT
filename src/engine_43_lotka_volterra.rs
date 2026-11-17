use nalgebra::Vector2;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, pop_vec[prey_depth_x, predator_vel_y])
static LV_STATE: Mutex<[(f64, Vector2<f64>); 3]> = Mutex::new([
    (0.0, Vector2::new(1.0, 0.5)),
    (0.0, Vector2::new(1.0, 0.5)),
    (0.0, Vector2::new(1.0, 0.5)),
]);

// ENGINE 43: Tier-0 Lotka-Volterra Predator-Prey Coupled Differential Liquidity Dynamics
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

    if let Ok(mut state) = LV_STATE.lock() {
        let (last_p, prev_pop) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_pop);
            return;
        }

        let ret_bps = ((price - last_p) / last_p).abs() * 10000.0;
        let book_mass = ((bid_sz + ask_sz).max(0.1)).ln().max(0.2);

        // Prey (x = normalized passive depth) & Predator (y = aggressive price velocity shock)
        let x = (prev_pop[0] * 0.80) + (book_mass * 0.20);
        let y = (prev_pop[1] * 0.75) + (ret_bps.max(0.1) * 0.25);

        // Lotka-Volterra Parameters: alpha(replenish)=0.4, beta(predation)=0.3, delta(growth)=0.25, gamma(decay)=0.35
        let dx_dt = (0.40 * x) - (0.30 * x * y);
        let dy_dt = (0.25 * x * y) - (0.35 * y);

        // Euler Forward Step in 2D Phase Space via nalgebra Vector2
        let d_pop = Vector2::new(dx_dt, dy_dt);
        let next_pop = (Vector2::new(x, y) + (d_pop * 0.15)).map(|v| v.clamp(0.05, 15.0));
        let predation_ratio = next_pop[1] / next_pop[0];

        let eco_regime = if predation_ratio > 1.6 && dy_dt > 0.0 {
            "🦖 [LV_PREDATOR_SWARM_LIQUIDITY_DEVOUR]"
        } else if predation_ratio < 0.45 && dx_dt > 0.0 {
            "🌿 [LV_PREY_DEPTH_OVERGROWTH_CALM]"
        } else {
            "🔄 [LV_CYCLIC_ORBIT_BALANCE]"
        };

        println!(
            "🐾 [E43:{}] PREY_X:{:.2} | PRED_Y:{:.2} | RATIO:{:.2} -> {}",
            symbol, next_pop[0], next_pop[1], predation_ratio, eco_regime
        );

        state[idx] = (price, next_pop);
    }
}
