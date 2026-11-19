use nalgebra::Vector3;
use serde_json::Value;
use std::sync::Mutex;

// State per asset (BTC=0, ETH=1, SOL=2): (last_price, hawk_vec[bh_mass_m, temp_th, evap_flux])
static HAWKING_STATE: Mutex<[(f64, Vector3<f64>); 3]> = Mutex::new([
    (0.0, Vector3::new(1.5, 0.4, 0.1)),
    (0.0, Vector3::new(1.5, 0.4, 0.1)),
    (0.0, Vector3::new(1.5, 0.4, 0.1)),
]);

// ENGINE 82: Tier-0 Hawking Radiation, Bekenstein Entropy & Black Hole Liquidity Evaporation Engine
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

    if let Ok(mut state) = HAWKING_STATE.lock() {
        let (last_p, prev_h) = state[idx];

        if last_p <= 0.0 {
            state[idx] = (price, prev_h);
            return;
        }

        let ret_bps = ((price - last_p) / last_p) * 10000.0;
        let book_imb = (bid_sz - ask_sz) / (bid_sz + ask_sz).max(0.1);

        // Effective Black Hole Liquidity Mass M from passive orderbook depth
        let inst_mass = ((bid_sz + ask_sz).max(1.0).ln() * 0.42).clamp(0.22, 6.0);
        let mass_m = (prev_h[0] * 0.78) + (inst_mass * 0.22);
        let dm_dt = mass_m - prev_h[0]; // Negative when liquidity wall is evaporating/pulled

        // Hawking Temperature T_H ~ 1 / M & Evaporation Luminosity L_H ~ 1 / M^2
        let inst_th = (0.85 / mass_m).clamp(0.05, 5.0);
        let temp_th = (prev_h[1] * 0.75) + (inst_th * 0.25);

        let shrink_boost = (-dm_dt).max(0.0) * 3.5;
        let inst_evap = ((0.75 / (mass_m * mass_m)) + shrink_boost).clamp(0.02, 20.0);
        let evap_flux = (prev_h[2] * 0.72) + (inst_evap * 0.28);

        // Bekenstein-Hawking Area Entropy S_BH ~ M^2
        let bh_entropy = 0.50 * mass_m * mass_m;
        let dir_drive = (ret_bps * 0.65) + (book_imb * 1.15);
        let next_h = Vector3::new(mass_m, temp_th, evap_flux);

        let hawk_regime = if evap_flux > 1.65 && dm_dt < -0.02 && dir_drive > 0.25 {
            "💥 [HAWKING_EVAPORATION_FLASH_BULL_ERUPTION]"
        } else if evap_flux > 1.65 && dm_dt < -0.02 && dir_drive < -0.25 {
            "🩸 [HAWKING_EVAPORATION_FLASH_BEAR_COLLAPSE]"
        } else if bh_entropy > 2.40 && temp_th < 0.45 {
            "🧊 [BEKENSTEIN_COLD_HORIZON_WALL_ABSORPTION]"
        } else {
            "☢️ [HAWKING_QUANTUM_VACUUM_RADIATION_DRIFT]"
        };

        println!(
            "☢️ [E82:{}] T_HAWK:{:.3} | L_EVAP:{:.3} | S_BH:{:.2} -> {}",
            symbol, next_h[1], next_h[2], bh_entropy, hawk_regime
        );

        state[idx] = (price, next_h);
    }
}
